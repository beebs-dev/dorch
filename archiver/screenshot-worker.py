#!/usr/bin/env python3

from __future__ import annotations

import argparse
import asyncio
import contextlib
import json
import os
import re
import signal
import sys
import tempfile
import time
from urllib.parse import urlparse
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
import uuid

import boto3
import requests
from nats.errors import TimeoutError as NatsTimeoutError

_PROM_AVAILABLE = False
try:
	from prometheus_client import Counter, Gauge, Histogram, start_http_server  # type: ignore

	_PROM_AVAILABLE = True
except Exception:
	# Prometheus metrics are optional at runtime; dependency is declared in requirements.
	_PROM_AVAILABLE = False

import meta
from natsutil import connect_nats, ensure_stream, nats_flush_timeout_seconds
from screenshots import NoMapsError, RenderConfig, render_screenshots


STREAM_NAME = os.getenv("DORCH_IMAGES_STREAM", "DORCH_IMAGES")
SUBJECT_PREFIX = os.getenv("DORCH_IMAGES_SUBJECT_PREFIX", "dorch.wad")
SUBJECT_SUFFIX = os.getenv("DORCH_IMAGES_SUBJECT_SUFFIX", "img")

WADINFO_BASE_URL = os.getenv("WADINFO_BASE_URL", "http://localhost:8000")


def _spaces_public_base_url(*, bucket: str, endpoint: str) -> str:
	"""Return public base URL like https://{bucket}.{region}.digitaloceanspaces.com

	We intentionally use virtual-hosted-style URLs because that's the canonical
	public form for DigitalOcean Spaces.
	"""
	bucket = (bucket or "").strip()
	if not bucket:
		raise ValueError("bucket must be non-empty")
	parsed = urlparse((endpoint or "").strip())
	host = (parsed.netloc or parsed.path or "").strip()
	if not host:
		raise ValueError(f"invalid endpoint (missing host): {endpoint!r}")
	# Always use https for public URLs.
	return f"https://{bucket}.{host.rstrip('/')}"


def _collect_map_image_payloads(
	*,
	sha1: str,
	output_root: str,
	public_base_url: str,
) -> Dict[str, List[Dict[str, Any]]]:
	"""Build {map_name: [wadimage-json]} from local render output.

	Expected directory structure under output_root:
	- {map}/images/{n}.webp
	- {map}/pano/pano_{n}.webp

	Returned items intentionally omit `id`.
	- regular screenshots: {"url": ...}
	- panos: {"url": ..., "type": "pano"}
	"""
	sha1 = (sha1 or "").strip().lower()
	if not _valid_sha1(sha1):
		raise ValueError("sha1 must be 40 hex chars")
	root = Path(output_root).expanduser().resolve()
	if not root.exists() or not root.is_dir():
		raise FileNotFoundError(f"output_root is not a directory: {root}")
	public_base_url = public_base_url.rstrip("/")

	out: Dict[str, List[Dict[str, Any]]] = {}
	for p in sorted(root.rglob("*")):
		if not p.is_file():
			continue
		rel = p.relative_to(root).as_posix()
		parts = rel.split("/")
		# Require at least: MAP01/images/0.webp
		if len(parts) < 3:
			continue
		map_name, kind_dir = parts[0], parts[1]
		if kind_dir not in {"images", "pano"}:
			continue
		key = f"{sha1}/{rel}"
		item: Dict[str, Any] = {"url": f"{public_base_url}/{key}"}
		if kind_dir == "pano":
			item["type"] = "pano"
		out.setdefault(map_name, []).append(item)

	return out


def put_wad_map_images_to_wadinfo(
	*,
	wadinfo_base_url: str,
	wad_id: str,
	map_name: str,
	items: List[Dict[str, Any]],
) -> None:
	if not _valid_uuid(wad_id):
		raise ValueError(f"invalid wad_id uuid: {wad_id}")
	map_name = (map_name or "").strip()
	if not map_name:
		raise ValueError("map_name must be non-empty")
	url = f"{wadinfo_base_url.rstrip('/')}/wad/{wad_id}/maps/{map_name}/images"
	r = requests.put(url, json=items, timeout=_wadinfo_timeout_seconds())
	r.raise_for_status()


def _maybe_start_prometheus_http_server(*, worker: str) -> None:
	"""Best-effort Prometheus /metrics server.

	Env:
	- DORCH_METRICS_ENABLED: true/false (default true)
	- DORCH_METRICS_ADDR: bind address (default 0.0.0.0)
	- DORCH_METRICS_PORT: port (default 2112)
	"""
	if not _PROM_AVAILABLE:
		return
	if not _env_bool("DORCH_METRICS_ENABLED", True):
		return
	addr = _env_str("DORCH_METRICS_ADDR", "0.0.0.0")
	port = _env_int("DORCH_METRICS_PORT", 2112)
	try:
		start_http_server(port, addr=addr)
		meta.eprint(f"{worker}: prometheus metrics listening on http://{addr}:{port}/metrics")
	except Exception as ex:
		meta.eprint(f"{worker}: prometheus metrics disabled (failed to start): {type(ex).__name__}: {ex}")


if _PROM_AVAILABLE:
	_SCREENSHOT_JOBS_TOTAL = Counter(
		"dorch_screenshot_jobs_total",
		"Screenshot jobs processed",
		["result"],
	)
	_SCREENSHOT_JOB_DURATION_SECONDS = Histogram(
		"dorch_screenshot_job_duration_seconds",
		"Screenshot job duration in seconds",
	)
	_SCREENSHOT_IN_PROGRESS = Gauge(
		"dorch_screenshot_in_progress",
		"Screenshot jobs currently being processed",
	)
	_SCREENSHOT_EXCEPTIONS_TOTAL = Counter(
		"dorch_screenshot_exceptions_total",
		"Exceptions while processing screenshot jobs",
		["exception"],
	)
	_SCREENSHOT_NO_MAPS_TOTAL = Counter(
		"dorch_screenshot_no_maps_total",
		"Screenshot jobs that had no renderable maps",
	)


def subject_for_wad_id(wad_id: str) -> str:
	wad_id = (wad_id or "").strip().lower()
	return f"{SUBJECT_PREFIX}.{wad_id}.{SUBJECT_SUFFIX}"


def wad_id_from_subject(subject: str) -> Optional[str]:
	# Expected: dorch.wad.{wad_id}.img (prefix length may vary)
	parts = (subject or "").split(".")
	if len(parts) < 4:
		return None
	if parts[-1] != SUBJECT_SUFFIX:
		return None
	wad_id = parts[-2].strip().lower()
	if not _valid_uuid(wad_id):
		return None
	return wad_id


def _env_bool(name: str, default: bool) -> bool:
	v = os.getenv(name)
	if v is None:
		return default
	v = v.strip().lower()
	return v in {"1", "true", "yes", "y", "on"}


def _env_int(name: str, default: int) -> int:
	v = os.getenv(name)
	if v is None or not v.strip():
		return default
	try:
		return int(v)
	except ValueError:
		return default


def _env_str(name: str, default: str) -> str:
	v = os.getenv(name)
	return (v.strip() if v is not None else "") or default


def _valid_sha1(s: str) -> bool:
	return bool(re.fullmatch(r"[0-9a-f]{40}", (s or "").lower()))


def _valid_uuid(s: str) -> bool:
	try:
		uuid.UUID(str(s))
		return True
	except Exception:
		return False


def _wadinfo_timeout_seconds() -> float:
	# Keep this independent from NATS timeouts.
	# Values like 5-30s are typical depending on DB latency.
	v = os.getenv("DORCH_WADINFO_TIMEOUT")
	if v is None or not v.strip():
		return 10.0
	try:
		return float(v)
	except ValueError:
		return 10.0


def _render_timeout_seconds() -> float:
	v = os.getenv("DORCH_SCREENSHOT_RENDER_TIMEOUT")
	if v is None or not v.strip():
		return 900.0
	try:
		return float(v)
	except ValueError:
		return 900.0


def _max_deliveries() -> int:
	# Cap retries for render crashes / deterministic failures.
	return _env_int("DORCH_SCREENSHOT_MAX_DELIVERIES", 3)


async def _run_renderer_subprocess(*, wad_id: str) -> Dict[str, Any]:
	print(f"running renderer for wad_id={wad_id}", file=sys.stderr)
	proc = await asyncio.create_subprocess_exec(
		sys.executable,
		"/app/screenshot-renderer.py",
		"--wad-id",
		wad_id,
		stdout=asyncio.subprocess.PIPE,
		stderr=asyncio.subprocess.PIPE,
	)

	async def _stream_and_capture_tail(
		stream: asyncio.StreamReader,
		*,
		max_chars: int = 4000,
	) -> str:
		tail = ""
		while True:
			chunk = await stream.read(4096)
			if not chunk:
				break
			text = chunk.decode("utf-8", errors="replace")
			sys.stderr.write(text)
			sys.stderr.flush()
			tail += text
			if len(tail) > max_chars:
				tail = tail[-max_chars:]
		return tail

	stdout_task = asyncio.create_task(proc.stdout.read() if proc.stdout is not None else asyncio.sleep(0, result=b""))
	stderr_task = asyncio.create_task(_stream_and_capture_tail(proc.stderr) if proc.stderr is not None else asyncio.sleep(0, result=""))
	wait_task = asyncio.create_task(proc.wait())
	try:
		stdout_b, stderr_tail, _ = await asyncio.wait_for(
			asyncio.gather(stdout_task, stderr_task, wait_task),
			timeout=_render_timeout_seconds(),
		)
	except asyncio.TimeoutError:
		with contextlib.suppress(ProcessLookupError):
			proc.kill()
		# Drain streams after kill so we can return useful output.
		with contextlib.suppress(Exception):
			await proc.wait()
		stdout_b, stderr_tail = await asyncio.gather(stdout_task, stderr_task, return_exceptions=False)
		return {
			"ok": False,
			"retry": True,
			"kind": "Timeout",
			"message": f"renderer timed out after {_render_timeout_seconds()}s",
			"stderr": (stderr_tail or "")[-4000:],
			"returncode": proc.returncode,
		}

	stderr = stderr_tail or ""
	stdout = (stdout_b or b"").decode("utf-8", errors="replace").strip()

	# Non-zero returncode includes crashes (e.g., SIGSEGV => -11).
	if proc.returncode != 0:
		return {
			"ok": False,
			"retry": True,
			"kind": "RendererCrashed",
			"message": f"renderer exit={proc.returncode}",
			"stderr": stderr[-4000:],
			"stdout": stdout[-4000:],
			"returncode": proc.returncode,
		}

	try:
		obj = json.loads(stdout) if stdout else None
	except Exception as ex:
		return {
			"ok": False,
			"retry": True,
			"kind": "BadRendererOutput",
			"message": f"invalid renderer JSON: {type(ex).__name__}: {ex}",
			"stderr": stderr[-4000:],
			"stdout": stdout[-4000:],
		}
	if not isinstance(obj, dict):
		return {
			"ok": False,
			"retry": True,
			"kind": "BadRendererOutput",
			"message": "renderer JSON must be an object",
			"stderr": stderr[-4000:],
			"stdout": stdout[-4000:],
		}
	if stderr.strip():
		obj.setdefault("stderr", stderr[-4000:])
	return obj


def fetch_wad_from_wadinfo(*, wad_id: str, wadinfo_base_url: str) -> Dict[str, Any]:
	if not _valid_uuid(wad_id):
		raise ValueError(f"invalid wad_id uuid: {wad_id}")
	url = f"{wadinfo_base_url.rstrip('/')}/wad/{wad_id}"
	r = requests.get(url, timeout=_wadinfo_timeout_seconds())
	# Let 404 / 5xx surface distinctly.
	r.raise_for_status()
	obj = r.json()
	if not isinstance(obj, dict):
		raise ValueError("wadinfo response must be a JSON object")
	return obj


def _wad_entry_from_wadinfo_meta(wad_meta: Dict[str, Any]) -> Tuple[Dict[str, Any], Optional[Dict[str, Any]]]:
	"""Build the minimal wad_entry + extracted dict expected by meta.py helpers."""
	file_meta = wad_meta.get("file")
	content = wad_meta.get("content")
	sources = wad_meta.get("sources")

	file_type = None
	if isinstance(file_meta, dict):
		v = file_meta.get("type")
		if isinstance(v, str) and v.strip():
			file_type = v.strip()

	iwads_guess = None
	engines_guess = None
	if isinstance(content, dict):
		v = content.get("iwads_guess")
		if isinstance(v, list):
			iwads_guess = [x for x in v if isinstance(x, str) and x.strip()]
		v = content.get("engines_guess")
		if isinstance(v, list):
			engines_guess = [x for x in v if isinstance(x, str) and x.strip()]

	extracted = None
	if isinstance(sources, dict):
		v = sources.get("extracted")
		if isinstance(v, dict):
			extracted = v

	wad_entry: Dict[str, Any] = {}
	if file_type is not None:
		wad_entry["type"] = file_type
	if iwads_guess is not None:
		wad_entry["iwads"] = iwads_guess
	if engines_guess is not None:
		wad_entry["engines"] = engines_guess
	return wad_entry, extracted


def signal_ready() -> None:
	ready_file = os.getenv("DORCH_READY_FILE")
	if ready_file:
		try:
			with open(ready_file, "w", encoding="utf-8") as f:
				f.write(f"ready {time.time()}\n")
		except Exception as ex:
			meta.eprint(f"Could not write ready file {ready_file}: {type(ex).__name__}: {ex}")


def render_one_wad_screenshots(
	*,
	wad_id: str,
	wadinfo_base_url: str,
	s3_wads,
	wad_bucket: str,
	images_bucket: str,
	images_endpoint: str,
	width: int,
	height: int,
	count: int,
	panorama: bool,
) -> Optional[Dict[str, List[Dict[str, Any]]]]:
	if not _valid_uuid(wad_id):
		raise ValueError("wad_id must be a UUID")
	obj = fetch_wad_from_wadinfo(wad_id=wad_id, wadinfo_base_url=wadinfo_base_url)
	meta_obj = obj.get("meta")
	if not isinstance(meta_obj, dict):
		raise ValueError("wadinfo wad JSON missing 'meta' object")
	sha1 = str(meta_obj.get("sha1") or "").strip().lower()
	if not _valid_sha1(sha1):
		raise ValueError("wadinfo meta.sha1 must be 40 hex chars")

	wad_entry, extracted_hint = _wad_entry_from_wadinfo_meta(meta_obj)
	wad_type = str(wad_entry.get("type") or "UNKNOWN")
	ext = meta.TYPE_TO_EXT.get(wad_type, None) or "wad"

	s3_key = meta.resolve_s3_key(s3_wads, wad_bucket, sha1, ext)

	with tempfile.TemporaryDirectory(prefix="dorch_img_") as td:
		gz_path = os.path.join(td, f"{sha1}.{ext}.gz")
		file_path = os.path.join(td, f"{sha1}.{ext}")
		output_path = os.path.join(td, "output_screenshots")

		meta.download_s3_to_path(s3_wads, wad_bucket, s3_key, gz_path)
		meta.gunzip_file(gz_path, file_path)

		# IWAD selection
		wad_type_upper = str(wad_entry.get("type") or "").upper()
		if wad_type_upper == "IWAD" and ext == "wad":
			iwad_path = Path(file_path)
			files_for_render: List[Path] = []
		else:
			extracted = extracted_hint
			if extracted is None:
				extracted = meta.extract_metadata_from_file(file_path, ext)
			iwad_path = meta.deduce_iwad_path_from_meta(wad_entry, extracted)
			# Render the downloaded PWAD/PK3 itself. (Passing the extracted metadata
			# dict here breaks map detection and makes jobs look like "no maps".)
			files_for_render = [Path(file_path)]

		os.makedirs(output_path, exist_ok=True)
		config = RenderConfig(
			wad_id=wad_id,
			iwad=iwad_path,
			files=files_for_render,
			output=Path(output_path),
			num=int(count),
			width=int(width),
			height=int(height),
			panorama=bool(panorama),
			invulnerable=True,
			format="webp",
			panorama_format="webp",
		)
		try:
			render_screenshots(config)
		except NoMapsError as e:
			if _PROM_AVAILABLE:
				_SCREENSHOT_NO_MAPS_TOTAL.inc()
			print(f"{wad_id}: {e}", file=sys.stderr)
			return None
		meta.upload_screenshots(
			sha1=sha1,
			path=output_path,
			bucket=images_bucket,
			endpoint=images_endpoint,
		)
		public_base_url = _spaces_public_base_url(bucket=images_bucket, endpoint=images_endpoint)
		return _collect_map_image_payloads(
			sha1=sha1,
			output_root=output_path,
			public_base_url=public_base_url,
		)


async def _run(args: argparse.Namespace) -> None:
	shutdown = asyncio.Event()
	fast_exit = False

	def _request_shutdown() -> None:
		nonlocal fast_exit
		fast_exit = True
		shutdown.set()

	def _immediate_shutdown() -> None:
		print("screenshot-worker: immediate shutdown requested", file=sys.stderr)
		sys.exit(1)

	try:
		loop = asyncio.get_running_loop()
		loop.add_signal_handler(signal.SIGTERM, _immediate_shutdown)
		loop.add_signal_handler(signal.SIGINT, _immediate_shutdown)
	except NotImplementedError:
		pass

	region_name = os.getenv("AWS_REGION") or os.getenv("AWS_DEFAULT_REGION")
	wad_bucket = _env_str("DORCH_WAD_BUCKET", "wadarchive2")
	wad_endpoint = _env_str("DORCH_WAD_ENDPOINT", "https://nyc3.digitaloceanspaces.com")
	images_bucket = _env_str("DORCH_IMAGES_BUCKET", "wadimages2")
	images_endpoint = _env_str("DORCH_IMAGES_ENDPOINT", "https://nyc3.digitaloceanspaces.com")

	default_width = _env_int("DORCH_SCREENSHOT_WIDTH", 800)
	default_height = _env_int("DORCH_SCREENSHOT_HEIGHT", 600)
	default_count = _env_int("DORCH_SCREENSHOT_COUNT", 3)
	default_panorama = _env_bool("DORCH_PANORAMA", False)
	wadinfo_base_url = _env_str("WADINFO_BASE_URL", WADINFO_BASE_URL)

	print(f"region_name: {region_name}", file=sys.stderr)
	print(f"wad_endpoint: {wad_endpoint}", file=sys.stderr)
	print(f"wad_bucket: {wad_bucket}", file=sys.stderr)
	print(f"images_endpoint: {images_endpoint}", file=sys.stderr)
	print(f"images_bucket: {images_bucket}", file=sys.stderr)
	print(f"default_width: {default_width}", file=sys.stderr)
	print(f"default_height: {default_height}", file=sys.stderr)
	print(f"default_count: {default_count}", file=sys.stderr)
	print(f"default_panorama: {default_panorama}", file=sys.stderr)
	print(f"wadinfo_base_url: {wadinfo_base_url}", file=sys.stderr)

	_maybe_start_prometheus_http_server(worker="screenshot-worker")

	s3_wads = boto3.client(
		"s3",
		endpoint_url=wad_endpoint,
		region_name=region_name,
	)

	nc = await connect_nats()
	try:
		js = nc.jetstream()
		await ensure_stream(js, STREAM_NAME, subjects=[f"{SUBJECT_PREFIX}.*.{SUBJECT_SUFFIX}"])

		durable = args.durable
		subject = f"{SUBJECT_PREFIX}.*.{SUBJECT_SUFFIX}"
		sub = await js.pull_subscribe(
			subject=subject,
			durable=durable,
			stream=STREAM_NAME,
		)
		signal_ready()
		meta.eprint(f"screenshot-worker: consuming from stream={STREAM_NAME} subject={subject} durable={durable}")

		while not shutdown.is_set():
			fetch_task = asyncio.create_task(sub.fetch(args.batch, timeout=args.fetch_timeout))
			shutdown_task = asyncio.create_task(shutdown.wait())
			done, _pending = await asyncio.wait(
				{fetch_task, shutdown_task},
				return_when=asyncio.FIRST_COMPLETED,
			)

			if shutdown_task in done:
				fetch_task.cancel()
				with contextlib.suppress(Exception):
					await fetch_task
				break

			shutdown_task.cancel()
			try:
				msgs = await fetch_task
			except (asyncio.TimeoutError, NatsTimeoutError):
				# Fetch timeout is normal; loop and try again.
				continue
			except Exception as ex:
				# Don't silently swallow auth/consumer/config errors.
				meta.eprint(f"screenshot-worker: fetch failed: {type(ex).__name__}: {ex}")
				await asyncio.sleep(0.5)
				continue

			for msg in msgs:
				if shutdown.is_set():
					try:
						await msg.nak()
					except Exception:
						pass
					continue

				job_start = time.perf_counter()
				if _PROM_AVAILABLE:
					_SCREENSHOT_IN_PROGRESS.inc()
				wad_id = None
				try:
					wad_id = msg.data.decode("utf-8").strip().strip('"')
					sub_id = wad_id_from_subject(msg.subject)
					if sub_id and sub_id != wad_id:
						wad_id = sub_id
					if not _valid_uuid(wad_id):
						raise ValueError(f"invalid wad_id uuid: {wad_id}")

					work_task = asyncio.create_task(_run_renderer_subprocess(wad_id=wad_id))
					shutdown_task = asyncio.create_task(shutdown.wait())
					done, _pending = await asyncio.wait(
						{work_task, shutdown_task},
						return_when=asyncio.FIRST_COMPLETED,
					)

					if shutdown_task in done:
						try:
							await msg.nak()
						except Exception:
							pass
						if _PROM_AVAILABLE:
							_SCREENSHOT_JOBS_TOTAL.labels("aborted").inc()
						work_task.cancel()
						work_task.add_done_callback(lambda t: t.exception() if not t.cancelled() else None)
						break

					shutdown_task.cancel()
					result = await work_task
					if result.get("ok") is True:
						map_images = result.get("map_images")
						if map_images is None:
							if _PROM_AVAILABLE:
								_SCREENSHOT_NO_MAPS_TOTAL.inc()
						else:
							for map_name, items in map_images.items():
								put_wad_map_images_to_wadinfo(
									wadinfo_base_url=wadinfo_base_url,
									wad_id=wad_id,
									map_name=map_name,
									items=items,
								)
						await msg.ack()
						if _PROM_AVAILABLE:
							_SCREENSHOT_JOBS_TOTAL.labels("success").inc()
					else:
						retry = bool(result.get("retry", True))
						kind = str(result.get("kind") or "RendererError")
						message = str(result.get("message") or "")
						stderr_tail = str(result.get("stderr") or "")
						meta.eprint(f"screenshot-worker: renderer failed kind={kind} retry={retry} wad_id={wad_id} msg={message}")
						if stderr_tail.strip():
							meta.eprint(f"screenshot-worker: renderer stderr (tail): {stderr_tail[-4000:]}")
						if _PROM_AVAILABLE:
							_SCREENSHOT_JOBS_TOTAL.labels("failure").inc()
							_SCREENSHOT_EXCEPTIONS_TOTAL.labels(kind).inc()

						delivered = None
						with contextlib.suppress(Exception):
							delivered = getattr(getattr(msg, "metadata", None), "num_delivered", None)
						max_deliveries = _max_deliveries()
						too_many = isinstance(delivered, int) and delivered >= max_deliveries
						if (not retry) or too_many:
							await msg.ack()
						else:
							with contextlib.suppress(Exception):
								await msg.nak()
				except Exception as ex:
					meta.eprint(f"screenshot-worker: job failed: {type(ex).__name__}: {ex}")
					if _PROM_AVAILABLE:
						_SCREENSHOT_JOBS_TOTAL.labels("failure").inc()
						_SCREENSHOT_EXCEPTIONS_TOTAL.labels(type(ex).__name__).inc()
						try:
							await msg.nak()
						except Exception:
							pass
				finally:
					if _PROM_AVAILABLE:
						_SCREENSHOT_IN_PROGRESS.dec()
						_SCREENSHOT_JOB_DURATION_SECONDS.observe(max(0.0, time.perf_counter() - job_start))

				if shutdown.is_set():
					break
	finally:
		if fast_exit:
			try:
				await nc.flush(timeout=int(nats_flush_timeout_seconds()))
			except Exception:
				pass
			await nc.close()
		else:
			await nc.drain()


def main() -> None:
	ap = argparse.ArgumentParser(description="Consume dorch screenshot jobs from NATS JetStream")
	ap.add_argument(
		"--durable",
		default=os.getenv("DORCH_IMAGES_DURABLE", "screenshot-worker"),
		help="JetStream durable consumer name",
	)
	ap.add_argument(
		"--batch",
		type=int,
		default=int(os.getenv("DORCH_IMAGES_BATCH", "1")),
		help="Fetch batch size",
	)
	ap.add_argument(
		"--fetch-timeout",
		type=float,
		default=float(os.getenv("DORCH_IMAGES_FETCH_TIMEOUT", "1.0")),
		help="Fetch timeout seconds",
	)
	args = ap.parse_args()

	try:
		asyncio.run(_run(args))
	except KeyboardInterrupt:
		raise SystemExit(130)


if __name__ == "__main__":
	main()

