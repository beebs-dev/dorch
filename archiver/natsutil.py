#!/usr/bin/env python3

from __future__ import annotations

import os
from typing import Optional

import nats
from nats.aio.client import Client as NATS
from nats.js.api import DiscardPolicy, RetentionPolicy, StorageType, StreamConfig


def _env(name: str, default: str = "") -> str:
	v = os.getenv(name)
	if v is None:
		return default
	v = v.strip()
	return v or default


async def connect_nats() -> NATS:
	servers = _env("NATS_URL", "nats://localhost:4222")
	user = _env("NATS_USER", "")
	password = _env("NATS_PASSWORD", "")
	token = _env("NATS_TOKEN", "")
	name = _env("NATS_NAME", "dorch-archiver")
	print(f"Connecting to NATS server at {servers} as {name}...")

	kwargs = {
		"servers": [servers],
		"name": name,
	}
	if token:
		kwargs["token"] = token
	elif user and password:
		kwargs["user"] = user
		kwargs["password"] = password

	# Note: creds / nkeys can be added later if needed.
	return await nats.connect(**kwargs)


async def ensure_stream(js, name: str, subjects: list[str]) -> None:
	try:
		await js.stream_info(name)
		return
	except Exception:
		pass

	max_age_seconds = float(_env("DORCH_META_MAX_AGE_SECONDS", "604800"))  # 7d
	duplicate_window_seconds = float(_env("DORCH_META_DEDUPE_WINDOW_SECONDS", "3600"))
	max_bytes = int(_env("DORCH_META_MAX_BYTES", "0"))  # 0 => unlimited

	cfg = StreamConfig(
		name=name,
		subjects=subjects,
		retention=RetentionPolicy.WORK_QUEUE,
		storage=StorageType.FILE,
		discard=DiscardPolicy.OLD,
		max_age=max_age_seconds,
		max_bytes=max_bytes,
		duplicate_window=duplicate_window_seconds,
	)
	await js.add_stream(cfg)
