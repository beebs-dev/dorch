#!/usr/bin/env python3

from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any, Dict

import boto3

import meta
from screenshot_job import render_one_wad_screenshots


def _env_int(name: str, default: int) -> int:
	v = os.getenv(name)
	if v is None or not v.strip():
		return default
	try:
		return int(v)
	except ValueError:
		return default


def _env_bool(name: str, default: bool) -> bool:
	v = os.getenv(name)
	if v is None or not v.strip():
		return default
	v = v.strip().lower()
	return v in {"1", "true", "yes", "y", "on"}


def _env_str(name: str, default: str) -> str:
	v = os.getenv(name)
	if v is None:
		return default
	v = v.strip()
	return v or default


def _print_json(obj: Dict[str, Any]) -> None:
	sys.stdout.write(json.dumps(obj, separators=(",", ":")))
	sys.stdout.write("\n")
	sys.stdout.flush()


def main() -> int:
	ap = argparse.ArgumentParser(description="Render screenshots for one wad_id (isolated subprocess)")
	ap.add_argument("--wad-id", required=True)
	args = ap.parse_args()

	region_name = os.getenv("AWS_REGION") or os.getenv("AWS_DEFAULT_REGION")
	wad_bucket = _env_str("DORCH_WAD_BUCKET", "wadarchive2")
	wad_endpoint = _env_str("DORCH_WAD_ENDPOINT", "https://nyc3.digitaloceanspaces.com")
	images_bucket = _env_str("DORCH_IMAGES_BUCKET", "wadimages2")
	images_endpoint = _env_str("DORCH_IMAGES_ENDPOINT", "https://nyc3.digitaloceanspaces.com")
	default_width = _env_int("DORCH_SCREENSHOT_WIDTH", 800)
	default_height = _env_int("DORCH_SCREENSHOT_HEIGHT", 600)
	default_count = _env_int("DORCH_SCREENSHOT_COUNT", 1)
	default_panorama = _env_bool("DORCH_PANORAMA", False)
	wadinfo_base_url = _env_str("WADINFO_BASE_URL", "http://localhost:8000")

	s3_wads = boto3.client(
		"s3",
		endpoint_url=wad_endpoint,
		region_name=region_name,
	)

	try:
		map_images = render_one_wad_screenshots(
			wad_id=args.wad_id,
			wadinfo_base_url=wadinfo_base_url,
			s3_wads=s3_wads,
			wad_bucket=wad_bucket,
			images_bucket=images_bucket,
			images_endpoint=images_endpoint,
			width=int(default_width),
			height=int(default_height),
			count=int(default_count),
			panorama=bool(default_panorama),
		)
		_print_json({"ok": True, "map_images": map_images})
		return 0
	except meta.S3KeyResolutionError as ex:
		_print_json({"ok": False, "retry": False, "kind": "S3KeyResolutionError", "message": str(ex)})
		return 0
	except Exception as ex:
		# Treat as retryable by default; worker will cap retries.
		_print_json({"ok": False, "retry": True, "kind": type(ex).__name__, "message": str(ex)})
		return 0


if __name__ == "__main__":
	raise SystemExit(main())
