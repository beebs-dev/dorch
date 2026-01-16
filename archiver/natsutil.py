#!/usr/bin/env python3

from __future__ import annotations

import os
import ssl
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


def _env_float(name: str, default: float) -> float:
	v = os.getenv(name)
	if v is None or not v.strip():
		return default
	try:
		return float(v)
	except ValueError:
		return default


def nats_publish_timeout_seconds() -> float:
	"""Timeout for JetStream publish acknowledgements.

	This guards `js.publish()` awaiting the PubAck. Larger payloads and/or a busy
	cluster can make the default too tight.
	"""
	return _env_float("DORCH_NATS_PUBLISH_TIMEOUT", 5.0)


def nats_flush_timeout_seconds() -> float:
	"""Timeout for `nc.flush()` on fast-exit paths."""
	return _env_float("DORCH_NATS_FLUSH_TIMEOUT", 3.0)


async def connect_nats() -> NATS:
	# Local dev defaults to localhost; in Kubernetes default to the `nats` namespace.
	in_k8s = bool(os.getenv("KUBERNETES_SERVICE_HOST"))
	servers = _env("NATS_URL", "nats://nats.nats:4222" if in_k8s else "nats://localhost:4222")
	# If user/password are not provided, use the common dorch defaults in-cluster.
	user = _env("NATS_USER", "app" if in_k8s else "")
	password = _env("NATS_PASSWORD", "devpass" if in_k8s else "")
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
		print(f"Connecting with NATS user '{user}'")
		kwargs["user"] = user
		kwargs["password"] = password
	use_tls = servers.startswith("tls://")
	if use_tls:
		print("Using TLS for NATS connection")
		ctx = ssl.create_default_context()
		kwargs["tls"] = ctx
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
