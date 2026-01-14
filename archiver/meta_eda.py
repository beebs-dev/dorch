#!/usr/bin/env python3

from __future__ import annotations

import json
import os
import time
from dataclasses import dataclass
from typing import Any, Dict, Optional

STREAM_NAME = os.getenv("DORCH_META_STREAM", "DORCH_META")
SUBJECT_PREFIX = os.getenv("DORCH_META_SUBJECT_PREFIX", "dorch.wad")
SUBJECT_SUFFIX = os.getenv("DORCH_META_SUBJECT_SUFFIX", "meta")


def subject_for_sha1(sha1: str) -> str:
	sha1 = (sha1 or "").strip().lower()
	return f"{SUBJECT_PREFIX}.{sha1}.{SUBJECT_SUFFIX}"


def sha1_from_subject(subject: str) -> Optional[str]:
	# Expected: dorch.wad.{sha1}.meta
	parts = (subject or "").split(".")
	if len(parts) < 4:
		return None
	# tolerate prefix changing length, but enforce suffix
	if parts[-1] != SUBJECT_SUFFIX:
		return None
	sha1 = parts[-2].strip().lower()
	if len(sha1) != 40:
		return None
	return sha1


@dataclass(frozen=True)
class MetaJob:
	version: int
	sha1: str
	wad_entry: Dict[str, Any]
	idgames_entry: Optional[Dict[str, Any]]
	dispatched_at: float

	def to_bytes(self) -> bytes:
		obj: Dict[str, Any] = {
			"version": int(self.version),
			"sha1": self.sha1,
			"wad_entry": self.wad_entry,
			"idgames_entry": self.idgames_entry,
			"dispatched_at": float(self.dispatched_at),
		}
		return json.dumps(obj, ensure_ascii=False).encode("utf-8")


def parse_meta_job(payload: bytes) -> MetaJob:
	obj = json.loads(payload.decode("utf-8"))
	if not isinstance(obj, dict):
		raise ValueError("job payload must be a JSON object")
	version = int(obj.get("version") or 1)
	sha1 = str(obj.get("sha1") or "").lower()
	wad_entry = obj.get("wad_entry")
	idgames_entry = obj.get("idgames_entry")
	dispatched_at = float(obj.get("dispatched_at") or 0.0)
	if not isinstance(wad_entry, dict):
		raise ValueError("job wad_entry must be an object")
	if idgames_entry is not None and not isinstance(idgames_entry, dict):
		raise ValueError("job idgames_entry must be an object or null")
	if not sha1 or len(sha1) != 40:
		raise ValueError("job sha1 must be 40 hex chars")
	if dispatched_at <= 0:
		dispatched_at = time.time()
	return MetaJob(
		version=version,
		sha1=sha1,
		wad_entry=wad_entry,
		idgames_entry=idgames_entry,
		dispatched_at=dispatched_at,
	)
