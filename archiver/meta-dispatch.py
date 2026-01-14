#!/usr/bin/env python3

from __future__ import annotations

import argparse
import asyncio
import re
import signal
import sys
import time
from typing import Any, Dict, List, Optional

import meta
from meta_eda import MetaJob, STREAM_NAME, subject_for_sha1
from natsutil import connect_nats, ensure_stream


def _valid_sha1(s: str) -> bool:
	return bool(re.fullmatch(r"[0-9a-f]{40}", (s or "").lower()))


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
	if meta.is_http_url(wads_json):
		meta.eprint(f"Downloading wads.json: {wads_json} -> /tmp/wads.json")
		meta.download_url_to_file(wads_json, "/tmp/wads.json")
		wads_json = "/tmp/wads.json"
	if meta.is_http_url(idgames_json):
		meta.eprint(f"Downloading idgames.json: {idgames_json} -> /tmp/idgames.json")
		meta.download_url_to_file(idgames_json, "/tmp/idgames.json")
		idgames_json = "/tmp/idgames.json"

	wads_data = meta.read_json_file(wads_json)
	idgames_data = meta.read_json_file(idgames_json)
	if not isinstance(wads_data, list):
		raise SystemExit("wads.json must be a JSON array")
	if not isinstance(idgames_data, list):
		raise SystemExit("idgames.json must be a JSON array")

	# Build idgames lookup keyed by sha1.
	wad_sha1s = {str(w.get("_id", "")).lower() for w in wads_data if isinstance(w, dict) and w.get("_id")}
	id_lookup = meta.build_idgames_lookup(idgames_data, wad_sha1s)

	nc = await connect_nats()
	try:
		js = nc.jetstream()
		await ensure_stream(js, STREAM_NAME, subjects=["dorch.wad.*.meta"])

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
				version=1,
				sha1=sha1,
				wad_entry=wad_entry,
				idgames_entry=id_lookup.get(sha1),
				dispatched_at=time.time(),
			)

			subj = subject_for_sha1(sha1)
			headers = {} #{"Nats-Msg-Id": f"dorch-meta:{sha1}"} # TODO
			await js.publish(subj, job.to_bytes(), headers=headers)
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
				await nc.flush(timeout=0.25)
			except Exception:
				pass
			await nc.close()
		else:
			await nc.drain()


def main() -> None:
	ap = argparse.ArgumentParser(description="Dispatch dorch meta jobs to NATS JetStream")
	ap.add_argument("--wads-json", required=True, help="Path or URL to wads.json")
	ap.add_argument("--idgames-json", required=True, help="Path or URL to idgames.json")
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
