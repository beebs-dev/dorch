#!/usr/bin/env python3

from __future__ import annotations

import argparse
import asyncio
import contextlib
import os
import re
import signal
import sys
import tempfile
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import boto3

import meta
from meta_eda import STREAM_NAME, parse_meta_job, sha1_from_subject
from natsutil import connect_nats, ensure_stream
from screenshots import RenderConfig, render_screenshots


_REDIS_CLIENT: Any = None


def _get_redis_client() -> Optional[Any]:
	"""Best-effort Redis client from env.

	Returns None if Redis isn't configured or redis-py isn't installed.
	"""
	global _REDIS_CLIENT
	if _REDIS_CLIENT is not None:
		return _REDIS_CLIENT if _REDIS_CLIENT is not False else None

	try:
		import redis  # type: ignore
	except Exception:
		_REDIS_CLIENT = False
		return None

	host = (os.getenv("REDIS_HOST") or "").strip()
	if not host:
		_REDIS_CLIENT = False
		return None
	port_s = (os.getenv("REDIS_PORT") or "").strip()
	try:
		port = int(port_s) if port_s else 6379
	except ValueError:
		port = 6379
	username = (os.getenv("REDIS_USERNAME") or "").strip() or None
	password = (os.getenv("REDIS_PASSWORD") or "").strip() or None
	proto = (os.getenv("REDIS_PROTO") or "redis").strip().lower()
	use_ssl = proto == "rediss"

	try:
		client = redis.Redis(
			host=host,
			port=port,
			username=username,
			password=password,
			ssl=use_ssl,
			decode_responses=False,
			socket_connect_timeout=2,
			socket_timeout=30,
		)
		_REDIS_CLIENT = client
		return client
	except Exception as ex:
		meta.eprint(f"Redis disabled (connect failed): {type(ex).__name__}: {ex}")
		_REDIS_CLIENT = False
		return None


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


def analyze_one_wad(
	*,
	sha1: str,
	wad_entry: Dict[str, Any],
	idgames_entry: Optional[Dict[str, Any]],
	s3_wads,
	wad_bucket: str,
	post_to_wadinfo: bool,
	wadinfo_base_url: str,
	render_screens: bool,
	upload_screens: bool,
	screenshot_width: int,
	screenshot_height: int,
	screenshot_count: int,
	panorama: bool,
	images_bucket: str,
	images_endpoint: str,
) -> Dict[str, Any]:
	sha1 = sha1.lower()
	if not _valid_sha1(sha1):
		raise ValueError("sha1 must be 40 hex chars")
	if not isinstance(wad_entry, dict):
		raise ValueError("wad_entry must be a dict")

	redis_client = _get_redis_client()
	redis_key = f"dorch:wad:{sha1}"

	wad_type = str(wad_entry.get("type") or "UNKNOWN")
	ext = meta.TYPE_TO_EXT.get(wad_type, None) or "wad"

	s3_key = meta.resolve_s3_key(s3_wads, wad_bucket, sha1, ext)
	s3_url = f"s3://{wad_bucket}/{s3_key}" if s3_key else None

	expected_hashes = wad_entry.get("hashes") or {}
	expected_sha256 = None
	if isinstance(expected_hashes, dict):
		v = expected_hashes.get("sha256")
		if isinstance(v, str) and v.strip():
			expected_sha256 = v.strip().lower()

	computed_hashes: Optional[Dict[str, str]] = None
	integrity: Optional[Dict[str, Any]] = None
	extracted: Dict[str, Any] = {}
	per_map_stats: List[Dict[str, Any]] = []

	with tempfile.TemporaryDirectory(prefix="dorch_meta_") as td:
		gz_path = os.path.join(td, f"{sha1}.{ext}.gz")
		file_path = os.path.join(td, f"{sha1}.{ext}")
		output_path = os.path.join(td, "output_screenshots")

		try:
			cached_bytes: Optional[bytes] = None
			if redis_client is not None:
				try:
					v = redis_client.get(redis_key)
					if v is not None:
						cached_bytes = bytes(v)
						meta.eprint(f"Redis cache hit {redis_key} ({len(cached_bytes)} bytes)")
				except Exception as ex:
					meta.eprint(f"Redis GET failed for {redis_key}: {type(ex).__name__}: {ex}")

			if cached_bytes is not None:
				with open(file_path, "wb") as f:
					f.write(cached_bytes)
			else:
				meta.download_s3_to_path(s3_wads, wad_bucket, s3_key, gz_path)
				meta.gunzip_file(gz_path, file_path)
				if redis_client is not None:
					try:
						with open(file_path, "rb") as f:
							buf = f.read()
						redis_client.set(redis_key, buf, ex=90 * 60)
						meta.eprint(f"Redis cache set {redis_key} ({len(buf)} bytes, ttl=5400s)")
					except Exception as ex:
						meta.eprint(f"Redis SET failed for {redis_key}: {type(ex).__name__}: {ex}")

			computed_hashes = meta.compute_hashes_for_file(file_path)
			if isinstance(expected_hashes, dict):
				integrity = meta.validate_expected_hashes(expected_hashes, computed_hashes)
			else:
				integrity = None

			extracted = meta.extract_metadata_from_file(file_path, ext)

			# Per-map stats
			if ext == "wad":
				with open(file_path, "rb") as f:
					per_map_stats = meta.extract_per_map_stats_from_wad_bytes(f.read())
			elif ext in {"pk3", "pk7", "pkz", "epk", "pke"}:
				embedded = meta.find_all_wads_in_zip_path(file_path)
				map_lists: List[List[Dict[str, Any]]] = []
				for (_wad_path, wbuf) in embedded:
					map_lists.append(meta.extract_per_map_stats_from_wad_bytes(wbuf))
				per_map_stats = meta.merge_per_map_stats(map_lists)

			if render_screens:
				try:
					# Deduce IWAD for rendering.
					wad_type_upper = str(wad_entry.get("type") or "").upper()
					if wad_type_upper == "IWAD" and ext == "wad":
						iwad_path = Path(file_path)
						files_for_render: List[Path] = []
					else:
						iwad_path = meta.deduce_iwad_path_from_meta(wad_entry, extracted)
						files_for_render = [Path(file_path)]

					if upload_screens:
						os.makedirs(output_path, exist_ok=True)
						config = RenderConfig(
							iwad=iwad_path,
							files=files_for_render,
							output=Path(output_path),
							num=screenshot_count,
							width=screenshot_width,
							height=screenshot_height,
							panorama=panorama,
							invulnerable=True,
						)
						render_screenshots(config)
						meta.upload_screenshots(
							sha1=sha1,
							path=output_path,
							bucket=images_bucket,
							endpoint=images_endpoint,
						)
				except Exception as ex:
					meta.eprint(f"Screenshot rendering/upload failed for {sha1}: {type(ex).__name__}: {ex}")
		except Exception as ex:
			extracted = {
				"format": "unknown",
				"error": f"Download/decompress/extract failed: {type(ex).__name__}: {ex}",
			}
			per_map_stats = []
			computed_hashes = None
			integrity = None

	meta_obj = meta.build_output_object(
		sha1=sha1,
		sha256=(computed_hashes or {}).get("sha256") or expected_sha256,
		s3_url=s3_url,
		extracted=extracted,
		wad_archive=wad_entry,
		idgames=idgames_entry,
		integrity=integrity,
	)
	out_obj = {"meta": meta_obj, "maps": per_map_stats}
	if post_to_wadinfo:
		meta.post_to_wadinfo(out_obj, sha1, wadinfo_base_url=wadinfo_base_url)
	return out_obj


def signal_ready() -> None:
	ready_file = os.getenv("DORCH_READY_FILE")
	if ready_file:
		try:
			with open(ready_file, "w", encoding="utf-8") as f:
				f.write(f"ready {time.time()}\n")
		except Exception as ex:
			meta.eprint(f"Could not write ready file {ready_file}: {type(ex).__name__}: {ex}")
			
async def _run(args: argparse.Namespace) -> None:
	shutdown = asyncio.Event()
	fast_exit = False

	def _request_shutdown() -> None:
		nonlocal fast_exit
		fast_exit = True
		shutdown.set()

	try:
		loop = asyncio.get_running_loop()
		loop.add_signal_handler(signal.SIGTERM, _request_shutdown)
		loop.add_signal_handler(signal.SIGINT, _request_shutdown)
	except NotImplementedError:
		# add_signal_handler is not available on some platforms (e.g. Windows)
		pass

	# JetStream + S3 clients
	region_name = os.getenv("AWS_REGION") or os.getenv("AWS_DEFAULT_REGION")
	wad_bucket = _env_str("DORCH_WAD_BUCKET", "wadarchive2")
	wad_endpoint = _env_str("DORCH_WAD_ENDPOINT", "https://nyc3.digitaloceanspaces.com")
	images_bucket = _env_str("DORCH_IMAGES_BUCKET", "wadimages")
	images_endpoint = _env_str("DORCH_IMAGES_ENDPOINT", "https://nyc3.digitaloceanspaces.com")
	print(f'region_name: {region_name}', file=sys.stderr)
	print(f'wad_endpoint: {wad_endpoint}', file=sys.stderr)
	print(f'wad_bucket: {wad_bucket}', file=sys.stderr)
	print(f'images_endpoint: {images_endpoint}', file=sys.stderr)
	print(f'images_bucket: {images_bucket}', file=sys.stderr)

	post_to_wadinfo = _env_bool("DORCH_POST_TO_WADINFO", True)
	wadinfo_base_url = _env_str("WADINFO_BASE_URL", "http://localhost:8000")
	print(f'wadinfo_base_url: {wadinfo_base_url}', file=sys.stderr)

	render_screens = _env_bool("DORCH_RENDER_SCREENSHOTS", False)
	upload_screens = _env_bool("DORCH_UPLOAD_SCREENSHOTS", False)
	screenshot_width = _env_int("DORCH_SCREENSHOT_WIDTH", 800)
	screenshot_height = _env_int("DORCH_SCREENSHOT_HEIGHT", 600)
	screenshot_count = _env_int("DORCH_SCREENSHOT_COUNT", 5)
	panorama = _env_bool("DORCH_PANORAMA", False)
	print(f'render_screens: {render_screens}', file=sys.stderr)
	print(f'upload_screens: {upload_screens}', file=sys.stderr)
	print(f'screenshot_width: {screenshot_width}', file=sys.stderr)
	print(f'screenshot_height: {screenshot_height}', file=sys.stderr)
	print(f'screenshot_count: {screenshot_count}', file=sys.stderr)
	print(f'panorama: {panorama}', file=sys.stderr)

	s3_wads = boto3.client(
		"s3",
		endpoint_url=wad_endpoint,
		region_name=region_name,
	)

	nc = await connect_nats()
	try:
		js = nc.jetstream()
		await ensure_stream(js, STREAM_NAME, subjects=["dorch.wad.*.meta"])

		durable = args.durable
		sub = await js.pull_subscribe(
			subject="dorch.wad.*.meta",
			durable=durable,
			stream=STREAM_NAME,
		)
		signal_ready()
		meta.eprint(f"meta-worker: consuming from stream={STREAM_NAME} durable={durable}")
		while not shutdown.is_set():
			fetch_task = asyncio.create_task(sub.fetch(args.batch, timeout=args.fetch_timeout))
			shutdown_task = asyncio.create_task(shutdown.wait())
			done, pending = await asyncio.wait(
				{fetch_task, shutdown_task},
				return_when=asyncio.FIRST_COMPLETED,
			)

			if shutdown_task in done:
				fetch_task.cancel()
				with contextlib.suppress(Exception):
					await fetch_task
				break

			shutdown_task.cancel()
			msgs = []
			try:
				msgs = await fetch_task
			except Exception:
				# fetch timeout is normal; loop
				continue

			for msg in msgs:
				if shutdown.is_set():
					# Best-effort immediate redelivery for any fetched-but-unprocessed messages.
					try:
						await msg.nak()
					except Exception:
						pass
					continue

				try:
					job = parse_meta_job(msg.data)
					sha1 = job.sha1
					# Subject is considered authoritative if it contains a sha1.
					sub_sha1 = sha1_from_subject(msg.subject)
					if sub_sha1 and sub_sha1 != sha1:
						sha1 = sub_sha1
					if not _valid_sha1(sha1):
						raise ValueError(f"invalid sha1: {sha1}")

					work_task = asyncio.create_task(
						asyncio.to_thread(
							analyze_one_wad,
							sha1=sha1,
							wad_entry=job.wad_entry,
							idgames_entry=job.idgames_entry,
							s3_wads=s3_wads,
							wad_bucket=wad_bucket,
							post_to_wadinfo=post_to_wadinfo,
							wadinfo_base_url=wadinfo_base_url,
							render_screens=render_screens,
							upload_screens=upload_screens,
							screenshot_width=screenshot_width,
							screenshot_height=screenshot_height,
							screenshot_count=screenshot_count,
							panorama=panorama,
							images_bucket=images_bucket,
							images_endpoint=images_endpoint,
						)
					)
					shutdown_task = asyncio.create_task(shutdown.wait())
					done, pending = await asyncio.wait(
						{work_task, shutdown_task},
						return_when=asyncio.FIRST_COMPLETED,
					)

					if shutdown_task in done:
						# Shutdown requested mid-job: best-effort NAK so it redelivers quickly.
						try:
							await msg.nak()
						except Exception:
							pass
						# Cancel the worker task (it may be running in a thread).
						work_task.cancel()
						# Avoid noisy "Task exception was never retrieved" warnings.
						work_task.add_done_callback(lambda t: t.exception() if not t.cancelled() else None)
						break

					# Job finished; propagate any exception.
					shutdown_task.cancel()
					await work_task
					await msg.ack()
				except Exception as ex:
					meta.eprint(f"meta-worker: job failed: {type(ex).__name__}: {ex}")
					try:
						# Requeue for retry (JetStream redeliver)
						await msg.nak()
					except Exception:
						pass

				if shutdown.is_set():
					break
	finally:
		if fast_exit:
			try:
				await nc.flush(timeout=1)
			except Exception:
				pass
			await nc.close()
		else:
			await nc.drain()


def main() -> None:
	ap = argparse.ArgumentParser(description="Consume dorch meta jobs from NATS JetStream")
	ap.add_argument("--durable", default=os.getenv("DORCH_META_DURABLE", "meta-worker"), help="JetStream durable consumer name")
	ap.add_argument("--batch", type=int, default=int(os.getenv("DORCH_META_BATCH", "1")), help="Fetch batch size")
	ap.add_argument("--fetch-timeout", type=float, default=float(os.getenv("DORCH_META_FETCH_TIMEOUT", "1.0")), help="Fetch timeout seconds")
	args = ap.parse_args()

	try:
		import asyncio
		asyncio.run(_run(args))
	except KeyboardInterrupt:
		raise SystemExit(130)


if __name__ == "__main__":
	main()
