#!/usr/bin/env python3

from __future__ import annotations

import argparse
import asyncio
import json
import re
import signal
import sys
import time
from typing import Any, Dict, List, Optional

import meta
from meta_eda import MetaJob, STREAM_NAME, subject_for_sha1
from natsutil import connect_nats, ensure_stream, nats_flush_timeout_seconds, nats_publish_timeout_seconds


def _valid_sha1(s: str) -> bool:
	return bool(re.fullmatch(r"[0-9a-f]{40}", (s or "").lower()))


def _read_jsonl_lookup(*, path: str, wanted_sha1s: set[str]) -> Dict[str, Dict[str, Any]]:
	"""Read JSONL file of objects with an _id sha1 field.

	Only keeps entries whose _id exists in wanted_sha1s.
	"""
	lookup: Dict[str, Dict[str, Any]] = {}
	with open(path, "r", encoding="utf-8") as f:
		for line in f:
			line = line.strip()
			if not line:
				continue
			obj = json.loads(line)
			if not isinstance(obj, dict):
				continue
			sha1 = str(obj.get("_id") or "").lower()
			if not _valid_sha1(sha1):
				continue
			if sha1 not in wanted_sha1s:
				continue
			lookup.setdefault(sha1, obj)
	return lookup


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
		pass

	# Load inputs (support URL like meta.py)
	wads_json = args.wads_json
	idgames_json = args.idgames_json
	readmes_json = args.readmes_json
	if meta.is_http_url(wads_json):
		meta.eprint(f"Downloading wads.json: {wads_json} -> /tmp/wads.json")
		meta.download_url_to_file(wads_json, "/tmp/wads.json")
		wads_json = "/tmp/wads.json"
	if meta.is_http_url(idgames_json):
		meta.eprint(f"Downloading idgames.json: {idgames_json} -> /tmp/idgames.json")
		meta.download_url_to_file(idgames_json, "/tmp/idgames.json")
		idgames_json = "/tmp/idgames.json"
	if meta.is_http_url(readmes_json):
		meta.eprint(f"Downloading readmes.json: {readmes_json} -> /tmp/readmes.json")
		meta.download_url_to_file(readmes_json, "/tmp/readmes.json")
		readmes_json = "/tmp/readmes.json"

	wads_data = meta.read_json_file(wads_json)
	idgames_data = meta.read_json_file(idgames_json)
	if not isinstance(wads_data, list):
		raise SystemExit("wads.json must be a JSON array")
	if not isinstance(idgames_data, list):
		raise SystemExit("idgames.json must be a JSON array")

	# Build idgames lookup keyed by sha1.
	wad_sha1s = {str(w.get("_id", "")).lower() for w in wads_data if isinstance(w, dict) and w.get("_id")}
	id_lookup = meta.build_idgames_lookup(idgames_data, wad_sha1s)
	readme_lookup = _read_jsonl_lookup(path=readmes_json, wanted_sha1s=wad_sha1s)

	nc = await connect_nats()
	try:
		js = nc.jetstream()
		await ensure_stream(js, STREAM_NAME, subjects=["dorch.wad.*.meta"])

		publish_timeout = nats_publish_timeout_seconds()

		total = len(wads_data)
		start = max(0, int(args.start))
		end = total if args.limit <= 0 else min(total, start + int(args.limit))

		published = 0
		for idx in range(start, end):
			if shutdown.is_set():
				break

			wad_entry = wads_data[idx]
			if not isinstance(wad_entry, dict):
				continue
			sha1 = str(wad_entry.get("_id") or "").lower()
			if not _valid_sha1(sha1):
				continue

			if args.smoke_test_id and args.smoke_test_id not in sha1:
				continue

			job = MetaJob(
				version=2,
				sha1=sha1,
				wad_entry=wad_entry,
				idgames_entry=id_lookup.get(sha1),
				readmes_entry=readme_lookup.get(sha1),
				dispatched_at=time.time(),
			)

			subj = subject_for_sha1(sha1)
			headers = {} #{"Nats-Msg-Id": f"dorch-meta:{sha1}"} # TODO
			await js.publish(subj, job.to_bytes(), headers=headers, timeout=publish_timeout)
			published += 1
			if args.sleep > 0:
				try:
					await asyncio.wait_for(shutdown.wait(), timeout=args.sleep)
				except asyncio.TimeoutError:
					pass

		print(f"Dispatched {published} jobs to stream {STREAM_NAME}")
	finally:
		if fast_exit:
			try:
				await nc.flush(timeout=nats_flush_timeout_seconds())
			except Exception:
				pass
			await nc.close()
		else:
			await nc.drain()


def main() -> None:
	ap = argparse.ArgumentParser(description="Dispatch dorch meta jobs to NATS JetStream")
	ap.add_argument("--wads-json", required=True, help="Path or URL to wads.json")
	ap.add_argument("--idgames-json", required=True, help="Path or URL to idgames.json")
	ap.add_argument("--readmes-json", required=True, help="Path or URL to readmes.json (JSONL)")
	ap.add_argument("--limit", type=int, default=0, help="Dispatch only N wads (0 = all)")
	ap.add_argument("--start", type=int, default=0, help="Start index into wads.json array")
	ap.add_argument("--sleep", type=float, default=0.0, help="Sleep seconds between publishes")
	ap.add_argument("--smoke-test-id", default=None, help="Only dispatch SHA1s containing this substring")
	args = ap.parse_args()

	# Async entry
	try:
		asyncio.run(_run(args))
	except KeyboardInterrupt:
		raise SystemExit(130)
	except Exception as ex:
		print(f"meta-dispatch failed: {type(ex).__name__}: {ex}", file=sys.stderr)
		raise


if __name__ == "__main__":
	main()
