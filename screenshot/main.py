#!/usr/bin/env python3

from __future__ import annotations

import argparse
import math
import os
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

import numpy as np
from PIL import Image


def _ensure_parent_dir(path: Path) -> None:
	path.parent.mkdir(parents=True, exist_ok=True)


def _clamp(x: float, lo: float, hi: float) -> float:
	return max(lo, min(hi, x))


def _wrap_angle_deg(a: float) -> float:
	a = a % 360.0
	if a < 0:
		a += 360.0
	return a


def _angle_distance_deg(a: float, b: float) -> float:
	# Smallest signed difference magnitude in degrees.
	d = (a - b) % 360.0
	if d > 180.0:
		d = 360.0 - d
	return abs(d)


def _halton(index: int, base: int) -> float:
	# Deterministic low-discrepancy sample in [0, 1).
	f = 1.0
	r = 0.0
	i = index
	while i > 0:
		f = f / base
		r = r + f * (i % base)
		i = i // base
	return r


def _ld_sequence(i: int) -> Tuple[float, float, float, float]:
	# 4D low-discrepancy point.
	return (
		_halton(i, 2),
		_halton(i, 3),
		_halton(i, 5),
		_halton(i, 7),
	)


@dataclass(frozen=True)
class Candidate:
	screen: np.ndarray  # HxWx3 uint8
	x: float
	y: float
	z: float
	angle: float
	pitch: float
	score: float
	pickup: bool


@dataclass(frozen=True)
class Thing:
	x: int
	y: int
	angle: int
	type: int
	flags: int


def _parse_wad_map_names(wad_path: Path) -> List[str]:
	"""Return map marker lumps found in a WAD in appearance order.

	Supports common Doom map markers:
	- MAP01..MAP99 style
	- E1M1 style

	We identify a map marker lump by checking that subsequent lumps include
	typical required lumps (THINGS and LINEDEFS). This is intentionally simple
	and robust for many WADs.
	"""

	data = wad_path.read_bytes()
	if len(data) < 12:
		raise ValueError(f"WAD too small: {wad_path}")

	ident = data[0:4]
	if ident not in (b"IWAD", b"PWAD"):
		raise ValueError(f"Not a WAD file (bad header {ident!r}): {wad_path}")

	num_lumps, dir_offset = struct.unpack_from("<II", data, 4)
	if dir_offset + num_lumps * 16 > len(data):
		raise ValueError(f"WAD directory out of range: {wad_path}")

	names: List[str] = []
	for i in range(num_lumps):
		entry_off = dir_offset + i * 16
		# <ii8s : filepos, size, name
		_filepos, _size, raw_name = struct.unpack_from("<II8s", data, entry_off)
		name = raw_name.split(b"\x00", 1)[0].decode("ascii", errors="ignore").upper()
		names.append(name)

	def is_map_marker(n: str) -> bool:
		if len(n) == 5 and n[0] == "E" and n[2] == "M" and n[1].isdigit() and n[3].isdigit() and n[4].isdigit():
			return True
		if len(n) == 5 and n.startswith("MAP") and n[3:].isdigit():
			return True
		return False

	# A very lightweight heuristic: map marker followed soon by THINGS and LINEDEFS.
	out: List[str] = []
	for i, n in enumerate(names):
		if not is_map_marker(n):
			continue
		window = names[i + 1 : i + 15]
		if "THINGS" in window and "LINEDEFS" in window:
			out.append(n)
	return out


def _read_wad_directory(wad_path: Path) -> List[Tuple[int, int, str]]:
	"""Return list of (filepos, size, name) for each lump."""
	data = wad_path.read_bytes()
	if len(data) < 12:
		raise ValueError(f"WAD too small: {wad_path}")

	ident = data[0:4]
	if ident not in (b"IWAD", b"PWAD"):
		raise ValueError(f"Not a WAD file (bad header {ident!r}): {wad_path}")

	num_lumps, dir_offset = struct.unpack_from("<II", data, 4)
	if dir_offset + num_lumps * 16 > len(data):
		raise ValueError(f"WAD directory out of range: {wad_path}")

	out: List[Tuple[int, int, str]] = []
	for i in range(num_lumps):
		entry_off = dir_offset + i * 16
		filepos, size, raw_name = struct.unpack_from("<II8s", data, entry_off)
		name = raw_name.split(b"\x00", 1)[0].decode("ascii", errors="ignore").upper()
		out.append((int(filepos), int(size), name))
	return out


def _extract_map_lump_bytes(wad_path: Path, map_name: str, lump_name: str) -> Optional[bytes]:
	"""Extract a map-associated lump (e.g., THINGS) from a WAD for a given map marker."""
	map_name = map_name.upper()
	lump_name = lump_name.upper()
	data = wad_path.read_bytes()
	directory = _read_wad_directory(wad_path)
	names = [n for _, _, n in directory]
	try:
		start = names.index(map_name)
	except ValueError:
		return None

	# Map lumps follow the marker until the next marker or end.
	for filepos, size, n in directory[start + 1 :]:
		if n == lump_name:
			if filepos + size > len(data):
				return None
			return data[filepos : filepos + size]
		if (len(n) == 5 and n.startswith("MAP") and n[3:].isdigit()) or (
			len(n) == 4 and n.startswith("E") and n[1].isdigit() and n[2] == "M" and n[3].isdigit()
		):
			break
	return None


def _parse_things(things_bytes: bytes) -> List[Thing]:
	"""Parse classic Doom THINGS lump (10 bytes per entry)."""
	out: List[Thing] = []
	if len(things_bytes) % 10 != 0:
		# Some ports use Hexen format (20 bytes). We'll ignore for now.
		return out
	for off in range(0, len(things_bytes), 10):
		x, y, angle, type_, flags = struct.unpack_from("<hhhhh", things_bytes, off)
		out.append(Thing(x=int(x), y=int(y), angle=int(angle), type=int(type_), flags=int(flags)))
	return out


def _is_pickup_thing_type(type_id: int) -> bool:
	# Doom / Doom II common pickup thing types.
	pickups = {
		# Weapons
		2001,
		2002,
		2003,
		2004,
		2005,
		2006,
		82,  # Super shotgun
		# Ammo
		2007,
		2008,
		2010,
		2047,
		2048,
		2049,
		2046,
		17,
		8,
		2013,
		# Health / armor
		2011,
		2012,
		2014,
		2015,
		2018,
		2019,
		# Powerups
		2022,
		2023,
		2024,
		2025,
		2026,
		2045,
	}
	return int(type_id) in pickups


def _pickup_points_for_map(iwad: Path, files: Sequence[Path], map_name: str) -> List[Tuple[float, float]]:
	"""Return pickup coordinates from the WAD that provides this map (load-order aware)."""
	load_order = [iwad, *files]
	# Prefer the last WAD in load order that contains the map's THINGS lump.
	for wad in reversed(load_order):
		things_bytes = _extract_map_lump_bytes(wad, map_name, "THINGS")
		if things_bytes is None:
			continue
		things = _parse_things(things_bytes)
		points = [(float(t.x), float(t.y)) for t in things if _is_pickup_thing_type(t.type)]
		return points
	return []


def _spread_out_points(points: Sequence[Tuple[float, float]], n: int, seed: int) -> List[Tuple[float, float]]:
	if not points or n <= 0:
		return []
	pts = np.array(points, dtype=np.float32)
	rng = np.random.default_rng(seed)
	start = int(rng.integers(0, len(pts)))
	selected_idx = [start]

	# Greedy farthest-point sampling in XY.
	d2 = np.sum((pts - pts[start]) ** 2, axis=1)
	for _ in range(min(n, len(pts)) - 1):
		j = int(np.argmax(d2))
		selected_idx.append(j)
		d2 = np.minimum(d2, np.sum((pts - pts[j]) ** 2, axis=1))
	return [points[i] for i in selected_idx]


def _effective_map_list(iwad: Path, files: Sequence[Path]) -> List[str]:
	# Doom load order: iwad then -file pwads. Later pwads can replace maps.
	from collections import OrderedDict

	ordered = OrderedDict()
	for wad in [iwad, *files]:
		try:
			maps = _parse_wad_map_names(wad)
		except Exception:
			# Non-WADs could appear in -file for some ports; ignore safely.
			continue
		for m in maps:
			if m in ordered:
				ordered.pop(m)
			ordered[m] = True
	return list(ordered.keys())


def _init_game(
	*,
	iwad: Path,
	files: Sequence[Path],
	map_name: str,
	seed: int,
	width: int,
	height: int,
	visible: bool,
	no_monsters: bool,
	skill: int,
	episode_timeout: int,
):
	# Imported lazily so `--list-maps` can work without vizdoom installed.
	from vizdoom import Button, DoomGame, GameVariable, Mode, ScreenFormat, ScreenResolution

	game = DoomGame()
	game.set_doom_game_path(str(iwad))

	game.set_screen_format(ScreenFormat.RGB24)

	# Depth buffer helps reject "staring at a wall" screenshots.
	# If a particular build doesn't support this call, it will raise; in that
	# case just continue without depth-based filtering.
	try:
		game.set_depth_buffer_enabled(True)
	except Exception:
		pass

	# VizDoom API compatibility:
	# - Some builds expose `set_screen_width/height`
	# - Others only expose `set_screen_resolution(ScreenResolution.*)`
	if hasattr(game, "set_screen_width") and hasattr(game, "set_screen_height"):
		game.set_screen_width(width)
		game.set_screen_height(height)
	else:
		# Map the requested (width,height) to the closest known preset.
		presets = {
			(160, 120): ScreenResolution.RES_160X120,
			(200, 125): ScreenResolution.RES_200X125,
			(256, 144): ScreenResolution.RES_256X144,
			(320, 180): ScreenResolution.RES_320X180,
			(320, 200): ScreenResolution.RES_320X200,
			(320, 240): ScreenResolution.RES_320X240,
			(400, 225): ScreenResolution.RES_400X225,
			(400, 250): ScreenResolution.RES_400X250,
			(512, 288): ScreenResolution.RES_512X288,
			(640, 360): ScreenResolution.RES_640X360,
			(640, 400): ScreenResolution.RES_640X400,
			(640, 480): ScreenResolution.RES_640X480,
			(800, 450): ScreenResolution.RES_800X450,
			(800, 500): ScreenResolution.RES_800X500,
			(800, 600): ScreenResolution.RES_800X600,
			(1024, 576): ScreenResolution.RES_1024X576,
			(1024, 640): ScreenResolution.RES_1024X640,
			(1024, 768): ScreenResolution.RES_1024X768,
			(1280, 720): ScreenResolution.RES_1280X720,
			(1280, 800): ScreenResolution.RES_1280X800,
			(1280, 960): ScreenResolution.RES_1280X960,
			(1600, 900): ScreenResolution.RES_1600X900,
			(1600, 1000): ScreenResolution.RES_1600X1000,
			(1600, 1200): ScreenResolution.RES_1600X1200,
			(1920, 1080): ScreenResolution.RES_1920X1080,
		}

		if (width, height) in presets:
			game.set_screen_resolution(presets[(width, height)])
		else:
			# Choose the closest preset by Euclidean distance.
			best = min(presets.keys(), key=lambda wh: (wh[0] - width) ** 2 + (wh[1] - height) ** 2)
			game.set_screen_resolution(presets[best])

	game.set_window_visible(visible)
	game.set_mode(Mode.PLAYER)
	game.set_seed(seed)
	game.set_episode_timeout(episode_timeout)

	# Controls
	game.set_available_buttons(
		[
			Button.MOVE_FORWARD,
			Button.MOVE_BACKWARD,
			Button.MOVE_LEFT,
			Button.MOVE_RIGHT,
			Button.USE,
			Button.SPEED,
			Button.TURN_LEFT_RIGHT_DELTA,
			Button.LOOK_UP_DOWN_DELTA,
		]
	)
	game.set_button_max_value(Button.TURN_LEFT_RIGHT_DELTA, 20.0)
	game.set_button_max_value(Button.LOOK_UP_DOWN_DELTA, 12.0)

	# Variables for diversity scoring.
	game.set_available_game_variables(
		[
			GameVariable.POSITION_X,
			GameVariable.POSITION_Y,
			GameVariable.POSITION_Z,
			GameVariable.ANGLE,
			GameVariable.PITCH,
			# Used to detect item pickups (if supported by the underlying build).
			getattr(GameVariable, "ITEMCOUNT", GameVariable.PITCH),
			getattr(GameVariable, "HEALTH", GameVariable.PITCH),
			getattr(GameVariable, "ARMOR", GameVariable.PITCH),
		]
	)

	# Map selection: prefer the dedicated API when present.
	if hasattr(game, "set_doom_map"):
		try:
			game.set_doom_map(map_name)
		except Exception:
			# Fall back to +map.
			pass

	# Launch args.
	args: List[str] = []
	# Some builds don't expose a getter; still pass +map as a strong hint.
	args.append(f"+map {map_name}")
	args.append(f"-skill {int(skill)}")
	# Make exploration easier / more deterministic.
	args.append("+freelook 1")
	args.append("+cl_run 1")
	if no_monsters:
		args.append("-nomonsters")
	if files:
		args.append("-file " + " ".join(str(p) for p in files))
	game.add_game_args(" ".join(args))

	game.init()
	return game


def _state_to_candidate(game) -> Optional[Candidate]:
	state = game.get_state()
	if state is None:
		return None
	if state.screen_buffer is None:
		return None

	# ViZDoom provides HxWx3 uint8 already for RGB24.
	screen = np.array(state.screen_buffer, copy=True)

	vars_ = state.game_variables
	if vars_ is None or len(vars_) < 5:
		return None
	x, y, z, angle, pitch = (float(vars_[0]), float(vars_[1]), float(vars_[2]), float(vars_[3]), float(vars_[4]))
	angle = _wrap_angle_deg(angle)
	pitch = float(pitch)

	# Always keep camera centered vertically.
	# We enforce pitch->0 in control; accept small drift.
	if abs(pitch) > 3.0:
		return None

	# Compute a cheap "interestingness" score from image statistics.
	# This helps filter out close-up walls and bland shots.
	img = screen.astype(np.float32) / 255.0
	lum = (0.2126 * img[..., 0]) + (0.7152 * img[..., 1]) + (0.0722 * img[..., 2])
	lum_std = float(lum.std())

	# Edge density via simple gradient magnitude.
	dx = np.abs(lum[:, 1:] - lum[:, :-1])
	dy = np.abs(lum[1:, :] - lum[:-1, :])
	# Match shapes by trimming.
	mag = dx[:-1, :] + dy[:, :-1]
	edge_density = float((mag > 0.08).mean())

	depth_median = None
	depth_std = 0.0
	try:
		depth = getattr(state, "depth_buffer", None)
		if depth is not None:
			d = np.array(depth, copy=False).astype(np.float32)
			# Some builds encode invalid pixels as 0; ignore those.
			d = d[np.isfinite(d)]
			d = d[d > 1e-6]
			if d.size > 0:
				depth_median = float(np.median(d))
				depth_std = float(np.std(d))
	except Exception:
		depth_median = None
		depth_std = 0.0

	# If the median depth is very small, we're likely pressed against a wall.
	near_wall_penalty = 0.0
	if depth_median is not None:
		if depth_median < 0.10:
			near_wall_penalty = 1.0
		elif depth_median < 0.18:
			near_wall_penalty = 0.5

	score = (
		2.2 * edge_density
		+ 1.1 * lum_std
		+ 0.4 * depth_std
		+ (0.15 * float(depth_median) if depth_median is not None else 0.0)
		- 1.3 * near_wall_penalty
	)

	return Candidate(screen=screen, x=x, y=y, z=z, angle=angle, pitch=pitch, score=float(score), pickup=False)


def _capture_best_yaw_sweep(game, *, pickup: bool) -> Optional[Candidate]:
	"""Pick the best of a few nearby yaw angles.

	This reduces "staring at a wall" frames by sampling a small left/right sweep,
	while returning the camera to its original yaw.
	"""

	best = _state_to_candidate(game)
	if best is not None and pickup:
		best = Candidate(
			screen=best.screen,
			x=best.x,
			y=best.y,
			z=best.z,
			angle=best.angle,
			pitch=best.pitch,
			score=best.score,
			pickup=True,
		)
	if game.is_episode_finished():
		return best

	# Turn deltas are limited by the button max value; keep within that.
	sweep = [18.0, -36.0]
	for d in sweep:
		game.make_action([0, 0, 0, 0, 0, 0, float(d), 0.0], 1)
		if game.is_episode_finished():
			return best
		cand = _state_to_candidate(game)
		if cand is not None and pickup:
			cand = Candidate(
				screen=cand.screen,
				x=cand.x,
				y=cand.y,
				z=cand.z,
				angle=cand.angle,
				pitch=cand.pitch,
				score=cand.score,
				pickup=True,
			)
		if cand is not None and (best is None or cand.score > best.score):
			best = cand

	# Restore yaw.
	game.make_action([0, 0, 0, 0, 0, 0, 18.0, 0.0], 1)
	return best


def _score_images_batch(rgb_images: np.ndarray, *, prefer_gpu: bool) -> np.ndarray:
	"""Return a score per image. Uses CuPy on CUDA if available."""

	if rgb_images.ndim != 4 or rgb_images.shape[-1] != 3:
		raise ValueError("Expected (B,H,W,3) RGB batch")

	# Try GPU first.
	if prefer_gpu:
		try:
			import cupy as cp  # type: ignore

			x = cp.asarray(rgb_images, dtype=cp.float32) / 255.0
			lum = 0.2126 * x[..., 0] + 0.7152 * x[..., 1] + 0.0722 * x[..., 2]
			lum_std = cp.std(lum, axis=(1, 2))
			dx = cp.abs(lum[:, :, 1:] - lum[:, :, :-1])
			dy = cp.abs(lum[:, 1:, :] - lum[:, :-1, :])
			mag = dx[:, :-1, :] + dy[:, :, :-1]
			edge_density = cp.mean(mag > 0.08, axis=(1, 2))
			score = 2.2 * edge_density + 1.1 * lum_std
			return cp.asnumpy(score)
		except Exception:
			pass

	# CPU fallback.
	x = rgb_images.astype(np.float32) / 255.0
	lum = 0.2126 * x[..., 0] + 0.7152 * x[..., 1] + 0.0722 * x[..., 2]
	lum_std = np.std(lum, axis=(1, 2))
	dx = np.abs(lum[:, :, 1:] - lum[:, :, :-1])
	dy = np.abs(lum[:, 1:, :] - lum[:, :-1, :])
	mag = dx[:, :-1, :] + dy[:, :, :-1]
	edge_density = np.mean(mag > 0.08, axis=(1, 2))
	return (2.2 * edge_density + 1.1 * lum_std).astype(np.float32)


def _teleport_to(game, x: float, y: float) -> bool:
	"""Best-effort teleport. Returns True if position changes noticeably."""
	try:
		from vizdoom import GameVariable

		before_x = float(game.get_game_variable(GameVariable.POSITION_X))
		before_y = float(game.get_game_variable(GameVariable.POSITION_Y))
	except Exception:
		before_x, before_y = 0.0, 0.0

	# Try common ZDoom-style console commands.
	try:
		game.send_game_command(f"setpos {x:.0f} {y:.0f} 0")
	except Exception:
		try:
			game.send_game_command(f"setpos {x:.0f} {y:.0f}")
		except Exception:
			return False

	# Let engine apply command.
	try:
		game.advance_action(1)
	except Exception:
		pass

	try:
		from vizdoom import GameVariable

		after_x = float(game.get_game_variable(GameVariable.POSITION_X))
		after_y = float(game.get_game_variable(GameVariable.POSITION_Y))
		return math.hypot(after_x - before_x, after_y - before_y) > 8.0
	except Exception:
		return True


def _center_pitch(game) -> None:
	from vizdoom import GameVariable

	# A few iterations to pull pitch to 0.
	for _ in range(6):
		try:
			cur_pitch = float(game.get_game_variable(GameVariable.PITCH))
		except Exception:
			cur_pitch = 0.0
		look = _clamp((-cur_pitch) * 0.9, -12.0, 12.0)
		game.make_action([0, 0, 0, 0, 0, 0, 0.0, float(look)], 1)
		if abs(cur_pitch) < 1.0:
			break


def _best_direction_at_location(
	game,
	*,
	prefer_gpu: bool,
	steps: int = 18,
	turn_step: float = 20.0,
) -> Optional[Candidate]:
	"""Render a full 360 yaw sweep and keep the most interesting direction."""

	frames: List[np.ndarray] = []
	cands: List[Optional[Candidate]] = []

	# Ensure we start from a centered pitch.
	_center_pitch(game)

	# Capture at current angle then rotate through the circle.
	for _ in range(steps):
		cand = _state_to_candidate(game)
		cands.append(cand)
		frames.append(cand.screen if cand is not None else np.zeros((1, 1, 3), dtype=np.uint8))
		game.make_action([0, 0, 0, 0, 0, 0, float(turn_step), 0.0], 1)
		if game.is_episode_finished():
			break

	# If we didn't get enough frames, bail out.
	valid_idx = [i for i, c in enumerate(cands) if c is not None]
	if not valid_idx:
		return None

	batch = np.stack([frames[i] for i in valid_idx], axis=0)
	scores = _score_images_batch(batch, prefer_gpu=prefer_gpu)
	best_local = int(valid_idx[int(np.argmax(scores))])

	# After steps turns, we're back at (roughly) the start; turn to best.
	for _ in range(best_local):
		game.make_action([0, 0, 0, 0, 0, 0, float(turn_step), 0.0], 1)
		if game.is_episode_finished():
			break

	best = _state_to_candidate(game)
	return best


def _feature_distance(a: Candidate, b: Candidate, pos_scale: float) -> float:
	dx = (a.x - b.x) / pos_scale
	dy = (a.y - b.y) / pos_scale
	dz = (a.z - b.z) / pos_scale
	da = _angle_distance_deg(a.angle, b.angle) / 90.0
	dp = abs(a.pitch - b.pitch) / 45.0
	return math.sqrt(dx * dx + dy * dy + dz * dz + da * da + dp * dp)


def _select_diverse(candidates: Sequence[Candidate], n: int) -> List[Candidate]:
	if not candidates:
		return []

	# Prefer selecting from pickup-anchored candidates (spread out across the map).
	pickup_candidates = [c for c in candidates if c.pickup]
	non_pickup_candidates = [c for c in candidates if not c.pickup]
	if len(pickup_candidates) >= n:
		candidates = pickup_candidates
	else:
		candidates = pickup_candidates + non_pickup_candidates

	# Prefer high-score candidates, then maximize diversity among them.
	ordered = sorted(candidates, key=lambda c: c.score, reverse=True)
	pool_size = min(len(ordered), max(n * 80, 400))
	pool = ordered[:pool_size]
	if n >= len(pool):
		return list(pool)

	# Normalize position scale using candidate spread.
	xs = np.array([c.x for c in pool], dtype=np.float32)
	ys = np.array([c.y for c in pool], dtype=np.float32)
	zs = np.array([c.z for c in pool], dtype=np.float32)
	spread = float(np.sqrt(np.var(xs) + np.var(ys) + np.var(zs)))
	pos_scale = max(spread, 64.0)

	selected: List[Candidate] = [pool[0]]
	min_d = [_feature_distance(pool[i], selected[0], pos_scale=pos_scale) for i in range(len(pool))]
	for _ in range(n - 1):
		# Greedy farthest-point sampling.
		j = int(np.argmax(np.array(min_d, dtype=np.float32)))
		selected.append(pool[j])
		for i in range(len(pool)):
			d = _feature_distance(pool[i], pool[j], pos_scale=pos_scale)
			if d < min_d[i]:
				min_d[i] = d
	return selected


def _gather_candidates(
	*,
	game,
	n: int,
	seed: int,
	warmup_steps: int,
	max_steps: int,
	frame_skip: int,
	keep_every: int,
) -> List[Candidate]:
	from vizdoom import GameVariable

	rng = np.random.default_rng(seed)

	# Warmup: run forward/strafe a bit while keeping pitch centered.
	for i in range(warmup_steps):
		u1, u2, u3, u4 = _ld_sequence(i + 1)
		turn = (u1 * 2.0 - 1.0) * 10.0
		# Mostly forward, occasional strafe.
		move_forward = u3 > 0.2
		strafe_left = (u4 < 0.1)
		strafe_right = (u4 > 0.9)
		# Keep pitch centered at 0.
		try:
			cur_pitch = float(game.get_game_variable(GameVariable.PITCH))
		except Exception:
			cur_pitch = 0.0
		look = _clamp((-cur_pitch) * 0.9, -12.0, 12.0)

		action = [0, 0, 0, 0, 0, 0, 0.0, 0.0]
		action[0] = 1 if move_forward else 0
		action[2] = 1 if strafe_left else 0
		action[3] = 1 if strafe_right else 0
		action[6] = float(turn)
		action[7] = float(look)
		game.make_action(action, frame_skip)
		if game.is_episode_finished():
			game.new_episode()

	candidates: List[Candidate] = []
	seen = set()

	# Generate candidates: a longer walk with low-discrepancy steering.
	# We add:
	# - SPEED (run)
	# - USE to open doors
	# - stuck detection to escape corners/doors
	target_candidates = max(n * 30, 250)
	last_pos: Optional[Tuple[float, float]] = None
	stuck_steps = 0

	# Pickup detection (best-effort; variables can differ across builds).
	def safe_var(var) -> Optional[float]:
		try:
			return float(game.get_game_variable(var))
		except Exception:
			return None

	itemcount_var = getattr(GameVariable, "ITEMCOUNT", None)
	health_var = getattr(GameVariable, "HEALTH", None)
	armor_var = getattr(GameVariable, "ARMOR", None)
	last_itemcount = safe_var(itemcount_var) if itemcount_var is not None else None
	last_health = safe_var(health_var) if health_var is not None else None
	last_armor = safe_var(armor_var) if armor_var is not None else None
	for t in range(max_steps):
		u1, u2, u3, u4 = _ld_sequence(t + 17)
		turn = (u1 * 2.0 - 1.0) * 20.0

		# Keep pitch centered at 0.
		try:
			cur_pitch = float(game.get_game_variable(GameVariable.PITCH))
		except Exception:
			cur_pitch = 0.0
		look = _clamp((-cur_pitch) * 0.9, -12.0, 12.0)

		# Discrete movement choices, but driven by low-discrepancy values.
		p = u3
		move_forward = p < 0.70
		move_backward = 0.70 <= p < 0.78
		move_left = 0.78 <= p < 0.89
		move_right = 0.89 <= p

		# Occasionally reduce turning to get clean compositions.
		if u4 < 0.15:
			turn *= 0.25
		if u4 > 0.85:
			look *= 0.25

		# Stuck detection: if we haven't moved much, try USE + a big turn.
		try:
			px = float(game.get_game_variable(GameVariable.POSITION_X))
			py = float(game.get_game_variable(GameVariable.POSITION_Y))
		except Exception:
			px, py = 0.0, 0.0
		if last_pos is not None:
			d = math.hypot(px - last_pos[0], py - last_pos[1])
			if d < 1.0:
				stuck_steps += 1
			else:
				stuck_steps = 0
		last_pos = (px, py)

		use = 0
		if (t % 45 == 0) or (stuck_steps >= 8):
			use = 1
		if stuck_steps >= 8:
			turn = (1.0 if (t % 2 == 0) else -1.0) * 35.0
			look = _clamp(-cur_pitch * 0.8, -12.0, 12.0)

		speed = 1  # run to reach more areas

		action = [0, 0, 0, 0, 0, 0, 0.0, 0.0]
		action[0] = 1 if move_forward else 0
		action[1] = 1 if move_backward else 0
		action[2] = 1 if move_left else 0
		action[3] = 1 if move_right else 0
		action[4] = int(use)
		action[5] = int(speed)
		action[6] = float(turn)
		action[7] = float(look)

		game.make_action(action, frame_skip)
		if game.is_episode_finished():
			game.new_episode()
			continue

		# Detect pickup events and capture a candidate at that location.
		picked_up = False
		if itemcount_var is not None:
			cur_itemcount = safe_var(itemcount_var)
			if cur_itemcount is not None and last_itemcount is not None and cur_itemcount > last_itemcount:
				picked_up = True
				last_itemcount = cur_itemcount
			elif cur_itemcount is not None:
				last_itemcount = cur_itemcount
		if not picked_up and health_var is not None:
			cur_health = safe_var(health_var)
			if cur_health is not None and last_health is not None and cur_health > last_health:
				picked_up = True
				last_health = cur_health
			elif cur_health is not None:
				last_health = cur_health
		if not picked_up and armor_var is not None:
			cur_armor = safe_var(armor_var)
			if cur_armor is not None and last_armor is not None and cur_armor > last_armor:
				picked_up = True
				last_armor = cur_armor
			elif cur_armor is not None:
				last_armor = cur_armor

		if picked_up:
			cand = _capture_best_yaw_sweep(game, pickup=True)
			if cand is not None:
				key = (
					int(cand.x // 32.0),
					int(cand.y // 32.0),
					int(cand.z // 16.0),
					int(_wrap_angle_deg(cand.angle) // 12.0),
					0,
				)
				if key not in seen:
					seen.add(key)
					candidates.append(cand)
					if len(candidates) >= target_candidates:
						break

		if t % keep_every != 0:
			continue

		cand = _capture_best_yaw_sweep(game, pickup=False)
		if cand is None:
			continue

		# Quantize pose to avoid near-duplicates.
		key = (
			int(cand.x // 32.0),
			int(cand.y // 32.0),
			int(cand.z // 16.0),
			int(_wrap_angle_deg(cand.angle) // 12.0),
			int(_clamp(cand.pitch, -89.0, 89.0) // 8.0),
		)
		if key in seen:
			continue
		seen.add(key)
		candidates.append(cand)

		if len(candidates) >= target_candidates:
			break

	# Shuffle slightly so selection doesn't always favor early frames.
	rng.shuffle(candidates)
	return candidates


def _save_image(arr: np.ndarray, out_path: Path, fmt: str, quality: int) -> None:
	_ensure_parent_dir(out_path)
	img = Image.fromarray(arr, mode="RGB")
	fmt_u = fmt.upper()
	if fmt_u in ("JPG", "JPEG"):
		img.save(out_path, format="JPEG", quality=quality, optimize=True)
	elif fmt_u == "PNG":
		img.save(out_path, format="PNG", optimize=True)
	else:
		raise ValueError(f"Unknown format: {fmt}")


def main(argv: Optional[Sequence[str]] = None) -> int:
	parser = argparse.ArgumentParser(
		description=(
			"Render N diverse first-person screenshots for every map in a WAD using ViZDoom.\n"
			"Default output layout: ${output}/${map_name}/${i}.jpg"
		)
	)
	parser.add_argument("--iwad", required=True, help="Path to IWAD (e.g., doom2.wad)")
	parser.add_argument(
		"--files",
		nargs="*",
		default=[],
		help="Additional WAD/PK3 files to load (like Doom -file). Order matters.",
	)
	parser.add_argument("--output", required=True, help="Output directory")
	parser.add_argument("-n", "--num", type=int, default=16, help="Screenshots per map")
	parser.add_argument("--seed", type=int, default=1234, help="Base RNG seed")
	parser.add_argument("--width", type=int, default=800)
	parser.add_argument("--height", type=int, default=600)
	parser.add_argument("--format", choices=["jpg", "png"], default="jpg")
	parser.add_argument("--jpeg-quality", type=int, default=92)
	parser.add_argument("--visible", action="store_true", help="Show the VizDoom window")
	parser.add_argument("--no-monsters", action="store_true", help="Pass -nomonsters")
	parser.add_argument("--skill", type=int, default=3, help="Doom skill 1-5")
	parser.add_argument("--episode-timeout", type=int, default=6000)
	parser.add_argument("--warmup-steps", type=int, default=40)
	parser.add_argument("--max-steps", type=int, default=8000)
	parser.add_argument("--frame-skip", type=int, default=4)
	parser.add_argument("--keep-every", type=int, default=6)
	parser.add_argument(
		"--prefer-gpu",
		action="store_true",
		help="Prefer GPU-accelerated direction scoring via CuPy if available",
	)
	parser.add_argument(
		"--list-maps",
		action="store_true",
		help="List detected maps from IWAD + --files and exit",
	)
	args = parser.parse_args(argv)

	iwad = Path(args.iwad)
	files = [Path(p) for p in args.files]
	out_root = Path(args.output)
	out_root.mkdir(parents=True, exist_ok=True)

	maps = _effective_map_list(iwad, files)
	if args.list_maps:
		for m in maps:
			print(m)
		return 0

	if not maps:
		raise SystemExit("No maps detected in IWAD/--files (WAD parsing heuristic found none).")
	if args.num <= 0:
		raise SystemExit("--num must be > 0")

	# Import VizDoom only when actually rendering.
	import vizdoom  # noqa: F401

	for mi, map_name in enumerate(maps):
		map_seed = int((args.seed * 1000003 + mi * 9176) & 0x7FFFFFFF)

		# New approach: choose globally-distributed pickup points as start locations.
		pickup_points = _pickup_points_for_map(iwad, files, map_name)
		starts = _spread_out_points(pickup_points, n=int(args.num), seed=map_seed)

		game = _init_game(
			iwad=iwad,
			files=files,
			map_name=map_name,
			seed=map_seed,
			width=args.width,
			height=args.height,
			visible=bool(args.visible),
			no_monsters=bool(args.no_monsters),
			skill=int(args.skill),
			episode_timeout=int(args.episode_timeout),
		)
		try:
			ext = "jpg" if args.format == "jpg" else "png"
			map_dir = out_root / map_name
			map_dir.mkdir(parents=True, exist_ok=True)
			saved = 0

			if starts:
				for i, (sx, sy) in enumerate(starts):
					game.new_episode()
					_teleport_to(game, sx, sy)
					_center_pitch(game)
					best = _best_direction_at_location(game, prefer_gpu=bool(args.prefer_gpu))
					if best is None:
						continue
					out_path = map_dir / f"{i}.{ext}"
					_save_image(best.screen, out_path, fmt=args.format, quality=int(args.jpeg_quality))
					saved += 1
			else:
				# Fallback to exploration if the map has no parseable pickups.
				game.new_episode()
				candidates = _gather_candidates(
					game=game,
					n=int(args.num),
					seed=map_seed,
					warmup_steps=int(args.warmup_steps),
					max_steps=int(args.max_steps),
					frame_skip=int(args.frame_skip),
					keep_every=int(args.keep_every),
				)
				selected = _select_diverse(candidates, n=int(args.num))
				for i, cand in enumerate(selected):
					out_path = map_dir / f"{i}.{ext}"
					_save_image(cand.screen, out_path, fmt=args.format, quality=int(args.jpeg_quality))
					saved += 1

			print(f"{map_name}: saved {saved}/{args.num} images")
		finally:
			game.close()

	return 0


if __name__ == "__main__":
	raise SystemExit(main())
