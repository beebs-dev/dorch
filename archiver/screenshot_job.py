#!/usr/bin/env python3

from __future__ import annotations

import contextlib
import os
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple
from urllib.parse import urlparse

import meta
from screenshots import NoMapsError, RenderConfig, render_screenshots


def _spaces_public_base_url(*, bucket: str, endpoint: str) -> str:
	"""Return public base URL like https://{bucket}.{region}.digitaloceanspaces.com."""
	bucket = (bucket or "").strip()
	if not bucket:
		raise ValueError("bucket must be non-empty")
	parsed = urlparse((endpoint or "").strip())
	host = (parsed.netloc or parsed.path or "").strip()
	if not host:
		raise ValueError(f"invalid endpoint (missing host): {endpoint!r}")
	return f"https://{bucket}.{host.rstrip('/')}"


def _valid_uuid(v: str) -> bool:
	import uuid

	try:
		uuid.UUID(str(v))
		return True
	except Exception:
		return False


def _valid_sha1(v: str) -> bool:
	import re

	return bool(re.fullmatch(r"[0-9a-f]{40}", (v or "").strip().lower()))


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


def _collect_map_image_payloads(
	*,
	sha1: str,
	output_root: str,
	public_base_url: str,
) -> Dict[str, List[Dict[str, Any]]]:
	"""Build {map_name: [wadimage-json]} from local render output."""
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


def fetch_wad_from_wadinfo(*, wad_id: str, wadinfo_base_url: str) -> Dict[str, Any]:
	import requests

	url = f"{wadinfo_base_url.rstrip('/')}/wad/{wad_id}"
	r = requests.get(url, timeout=float(os.getenv("DORCH_WADINFO_TIMEOUT", "10")))
	r.raise_for_status()
	obj = r.json()
	if not isinstance(obj, dict):
		raise ValueError("wadinfo response must be a JSON object")
	return obj


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
	"""Render and upload screenshots for a wad_id, returning wadinfo payload.

	Returns None when there are no maps to render.
	May raise exceptions (including meta.S3KeyResolutionError).
	"""
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

		wad_type_upper = str(wad_entry.get("type") or "").upper()
		if wad_type_upper == "IWAD" and ext == "wad":
			iwad_path = Path(file_path)
			files_for_render: List[Path] = []
		else:
			extracted = extracted_hint
			if extracted is None:
				extracted = meta.extract_metadata_from_file(file_path, ext)
			iwad_path = meta.deduce_iwad_path_from_meta(wad_entry, extracted)
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
			# Keep renderer stdout JSON-only; route progress prints to stderr.
			with contextlib.redirect_stdout(sys.stderr):
				render_screenshots(config)
		except NoMapsError:
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
