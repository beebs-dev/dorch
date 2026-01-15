#!/usr/bin/env python3

from __future__ import annotations

import argparse
import math
import os
import struct
import sys
import zipfile
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

import numpy as np
from PIL import Image


def _iwads_dir() -> Path:
    dir = os.getenv("IWADS_DIR", None)
    if dir is not None:
        return Path(dir).resolve()
    return (Path(__file__).resolve().parent / "../../wads").resolve()

class ScreenshotsError(RuntimeError):
	pass


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
	return _parse_wad_map_names_bytes(wad_path.read_bytes(), source=str(wad_path))


def _is_map_marker(name: str) -> bool:
	name = (name or "").upper()
	if len(name) == 5 and name[0] == "E" and name[2] == "M" and name[1].isdigit() and name[3].isdigit() and name[4].isdigit():
		return True
	if len(name) == 5 and name.startswith("MAP") and name[3:].isdigit():
		return True
	return False


def _parse_wad_map_names_bytes(data: bytes, *, source: str) -> List[str]:
	if len(data) < 12:
		raise ValueError(f"WAD too small: {source}")

	ident = data[0:4]
	if ident not in (b"IWAD", b"PWAD"):
		raise ValueError(f"Not a WAD file (bad header {ident!r}): {source}")

	num_lumps, dir_offset = struct.unpack_from("<II", data, 4)
	if dir_offset + num_lumps * 16 > len(data):
		raise ValueError(f"WAD directory out of range: {source}")

	names: List[str] = []
	for i in range(num_lumps):
		entry_off = dir_offset + i * 16
		_filepos, _size, raw_name = struct.unpack_from("<II8s", data, entry_off)
		name = raw_name.split(b"\x00", 1)[0].decode("ascii", errors="ignore").upper()
		names.append(name)

	# A very lightweight heuristic: map marker followed soon by THINGS and LINEDEFS.
	out: List[str] = []
	for i, n in enumerate(names):
		if not _is_map_marker(n):
			continue
		window = names[i + 1 : i + 15]
		if "THINGS" in window and "LINEDEFS" in window:
			out.append(n)
	return out


def _parse_pk3_map_names(pk3_path: Path) -> List[str]:
	"""Return map names found in a PK3 (ZIP) by scanning embedded *.wad entries."""
	from collections import OrderedDict

	ordered = OrderedDict()
	with zipfile.ZipFile(pk3_path) as zf:
		for info in zf.infolist():
			if getattr(info, "is_dir", lambda: False)():
				continue
			name = str(info.filename)
			if not name.lower().endswith(".wad"):
				continue
			try:
				data = zf.read(info)
				maps = _parse_wad_map_names_bytes(data, source=f"{pk3_path}:{name}")
			except Exception:
				continue
			for m in maps:
				if m in ordered:
					ordered.pop(m)
				ordered[m] = True
	return list(ordered.keys())


def _read_wad_directory(wad_path: Path) -> List[Tuple[int, int, str]]:
	"""Return list of (filepos, size, name) for each lump."""
	data = wad_path.read_bytes()
	return _read_wad_directory_bytes(data, source=str(wad_path))


def _read_wad_directory_bytes(data: bytes, *, source: str) -> List[Tuple[int, int, str]]:
	if len(data) < 12:
		raise ValueError(f"WAD too small: {source}")

	ident = data[0:4]
	if ident not in (b"IWAD", b"PWAD"):
		raise ValueError(f"Not a WAD file (bad header {ident!r}): {source}")

	num_lumps, dir_offset = struct.unpack_from("<II", data, 4)
	if dir_offset + num_lumps * 16 > len(data):
		raise ValueError(f"WAD directory out of range: {source}")

	out: List[Tuple[int, int, str]] = []
	for i in range(num_lumps):
		entry_off = dir_offset + i * 16
		filepos, size, raw_name = struct.unpack_from("<II8s", data, entry_off)
		name = raw_name.split(b"\x00", 1)[0].decode("ascii", errors="ignore").upper()
		out.append((int(filepos), int(size), name))
	return out


def _extract_map_lump_bytes(wad_path: Path, map_name: str, lump_name: str) -> Optional[bytes]:
	"""Extract a map-associated lump (e.g., THINGS) from a WAD for a given map marker."""
	data = wad_path.read_bytes()
	found_map, lump = _extract_map_lump_bytes_from_wad_bytes(data, map_name=map_name, lump_name=lump_name, source=str(wad_path))
	return lump if found_map else None


def _extract_map_lump_bytes_from_wad_bytes(
	data: bytes,
	*,
	map_name: str,
	lump_name: str,
	source: str,
) -> Tuple[bool, Optional[bytes]]:
	"""Return (found_map, lump_bytes) for a given map in a WAD byte blob."""
	map_name = map_name.upper()
	lump_name = lump_name.upper()
	directory = _read_wad_directory_bytes(data, source=source)
	names = [n for _, _, n in directory]
	try:
		start = names.index(map_name)
	except ValueError:
		return False, None

	# Map lumps follow the marker until the next marker or end.
	for filepos, size, n in directory[start + 1 :]:
		if n == lump_name:
			if filepos + size > len(data):
				return True, None
			return True, data[filepos : filepos + size]
		if _is_map_marker(n):
			break
	return True, None


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
	for src in reversed(load_order):
		suffix = str(src.suffix).lower()
		if suffix == ".pk3":
			found_map, points = _pickup_points_for_map_from_pk3(src, map_name)
			if found_map:
				return points
			continue
		try:
			data = src.read_bytes()
			found_map, things_bytes = _extract_map_lump_bytes_from_wad_bytes(
				data,
				map_name=map_name,
				lump_name="THINGS",
				source=str(src),
			)
			if not found_map:
				continue
			if things_bytes is None:
				return []
			things = _parse_things(things_bytes)
			points = [(float(t.x), float(t.y)) for t in things if _is_pickup_thing_type(t.type)]
			return points
		except Exception:
			# Non-WADs (or corrupted WADs) can appear in load order; ignore safely.
			continue
	return []


def _pickup_points_for_map_from_pk3(pk3_path: Path, map_name: str) -> Tuple[bool, List[Tuple[float, float]]]:
	"""Return (found_map, pickup_points) by scanning embedded *.wad entries in a PK3."""
	try:
		with zipfile.ZipFile(pk3_path) as zf:
			# Later entries should win; search in reverse order.
			infos = list(zf.infolist())
			for info in reversed(infos):
				if getattr(info, "is_dir", lambda: False)():
					continue
				name = str(info.filename)
				if not name.lower().endswith(".wad"):
					continue
				try:
					data = zf.read(info)
					found_map, things_bytes = _extract_map_lump_bytes_from_wad_bytes(
						data,
						map_name=map_name,
						lump_name="THINGS",
						source=f"{pk3_path}:{name}",
					)
				except Exception:
					continue
				if not found_map:
					continue
				if things_bytes is None:
					return True, []
				things = _parse_things(things_bytes)
				points = [(float(t.x), float(t.y)) for t in things if _is_pickup_thing_type(t.type)]
				return True, points
		return False, []
	except Exception:
		return False, []


def _spread_out_points(points: Sequence[Tuple[float, float]], n: int, seed: int) -> List[Tuple[float, float]]:
	if not points or n <= 0:
		return []
	pts = np.array(points, dtype=np.float32)

	# Start from a point far from the centroid to better maximize spread.
	centroid = np.mean(pts, axis=0)
	start = int(np.argmax(np.sum((pts - centroid) ** 2, axis=1)))
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
	for p in [iwad, *files]:
		maps: List[str] = []
		try:
			suffix = str(p.suffix).lower()
			if suffix == ".pk3":
				maps = _parse_pk3_map_names(p)
			else:
				maps = _parse_wad_map_names(p)
		except Exception:
			# Non-WADs / non-PK3s could appear in -file; ignore safely.
			maps = []
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

	# Hide the first-person weapon sprite (gun) so it never appears in screenshots.
	# Prefer the ViZDoom API when available; otherwise fall back to a ZDoom cvar.
	try:
		if hasattr(game, "set_render_weapon"):
			game.set_render_weapon(False)
	except Exception:
		pass

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
	# Keep console disabled; many commands are unavailable in this build and enabling it
	# is noisy. We navigate to targets using player controls instead.
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
	# Disable UI overlays for clean screenshots.
	args.append("+show_messages 0")
	args.append("+hud 0")
	args.append("+crosshair 0")
	args.append("+automap 0")
	# Some builds don't expose a getter; still pass +map as a strong hint.
	args.append(f"+map {map_name}")
	args.append(f"-skill {int(skill)}")
	# Make exploration easier / more deterministic.
	args.append("+freelook 1")
	args.append("+cl_run 1")
	# Redundant with `set_render_weapon(False)`, but helps on builds that don't expose it.
	args.append("+r_drawplayersprites 0")
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


def _enable_invulnerability(game) -> None:
	"""Best-effort: enable invulnerability via ZDoom cheat commands.

	ViZDoom runs a ZDoom-derived engine; in typical single-player configs, the
	console command `god` enables god mode (invulnerability). We keep this
	best-effort (no hard failure) because some mods/builds can restrict cheats.
	"""
	try:
		game.send_game_command("god")
	except Exception:
		return

	# Advance one tic so the command applies.
	try:
		game.make_action([0, 0, 0, 0, 0, 0, 0.0, 0.0], 1)
	except Exception:
		pass


def _new_episode(game, *, invulnerable: bool) -> None:
	game.new_episode()
	if invulnerable:
		_enable_invulnerability(game)


def _teleport_to(game, *, x: float, y: float) -> bool:
	"""Teleport the player to (x,y) using ZDoom's `warp` console command.

	This ViZDoom/ZDoom build does not support `setpos`, but does support `warp`.
	"""
	from vizdoom import GameVariable

	try:
		before_x = float(game.get_game_variable(GameVariable.POSITION_X))
		before_y = float(game.get_game_variable(GameVariable.POSITION_Y))
	except Exception:
		before_x, before_y = 0.0, 0.0

	# ZDoom expects integers here.
	ix = int(round(float(x)))
	iy = int(round(float(y)))
	game.send_game_command(f"warp {ix} {iy}")
	# Advance one tic so the command applies.
	game.make_action([0, 0, 0, 0, 0, 0, 0.0, 0.0], 1)

	try:
		after_x = float(game.get_game_variable(GameVariable.POSITION_X))
		after_y = float(game.get_game_variable(GameVariable.POSITION_Y))
	except Exception:
		return False

	# Consider it successful if we actually moved substantially and landed near target.
	moved = math.hypot(after_x - before_x, after_y - before_y)
	near = math.hypot(after_x - float(x), after_y - float(y))
	return moved > 8.0 and near < 128.0


def _walk_to(
	game,
	*,
	target_x: float,
	target_y: float,
	max_steps: int,
	frame_skip: int,
	reach_dist: float = 72.0,
) -> bool:
	"""Autopilot: walk/run toward (target_x,target_y)."""
	from vizdoom import GameVariable

	stuck = 0
	prev_dist: Optional[float] = None

	for t in range(max_steps):
		if game.is_episode_finished():
			return False
		try:
			px = float(game.get_game_variable(GameVariable.POSITION_X))
			py = float(game.get_game_variable(GameVariable.POSITION_Y))
			angle = float(game.get_game_variable(GameVariable.ANGLE))
			pitch = float(game.get_game_variable(GameVariable.PITCH))
		except Exception:
			return False

		dx = target_x - px
		dy = target_y - py
		dist = math.hypot(dx, dy)
		if dist <= reach_dist:
			return True

		# Desired yaw towards target.
		desired = math.degrees(math.atan2(dy, dx))
		delta = ((desired - angle + 540.0) % 360.0) - 180.0
		turn = _clamp(delta * 0.55, -20.0, 20.0)
		look = _clamp((-pitch) * 0.9, -12.0, 12.0)

		# Stuck detection: if not getting closer, try use/strafe/turn.
		if prev_dist is not None and dist >= prev_dist - 1.0:
			stuck += 1
		else:
			stuck = 0
		prev_dist = dist

		use = 1 if (t % 25 == 0 or stuck >= 18) else 0
		speed = 1

		# Default: run forward.
		move_forward = 1
		move_backward = 0
		move_left = 0
		move_right = 0
		if stuck >= 18:
			# Wiggle out.
			move_left = 1 if (t % 2 == 0) else 0
			move_right = 1 if (t % 2 == 1) else 0
			turn = 20.0 if (t % 2 == 0) else -20.0
			move_forward = 0
			move_backward = 1

		action = [move_forward, move_backward, move_left, move_right, use, speed, float(turn), float(look)]
		game.make_action(action, frame_skip)

	return False


def _best_direction_at_location(
	game,
	*,
	prefer_gpu: bool,
	base_angle_deg: float,
	steps: int = 18,
	turn_step: float = 20.0,
) -> Optional[Candidate]:
	"""Render a full 360 yaw sweep and keep the most interesting direction."""

	frames: List[np.ndarray] = []
	cands: List[Optional[Candidate]] = []

	# Ensure we start from a centered pitch.
	_center_pitch(game)
	# In this ViZDoom build, console commands like setangle/setpos are unavailable,
	# so we rely on turn deltas.
	using_setangle = False

	# Capture at multiple angles around the circle.
	actual_steps = 0
	for k in range(steps):
		cand = _state_to_candidate(game)
		cands.append(cand)
		frames.append(cand.screen if cand is not None else np.zeros((1, 1, 3), dtype=np.uint8))
		if game.is_episode_finished():
			break
		actual_steps += 1
		if using_setangle:
			# Evenly spaced sweep around base angle.
			ang = base_angle_deg + (360.0 * (k + 1) / float(steps))
			_ = ang
		else:
			game.make_action([0, 0, 0, 0, 0, 0, float(turn_step), 0.0], 1)

	# If we didn't get enough frames, bail out.
	valid_idx = [i for i, c in enumerate(cands) if c is not None]
	if not valid_idx:
		return None

	batch = np.stack([frames[i] for i in valid_idx], axis=0)
	scores = _score_images_batch(batch, prefer_gpu=prefer_gpu)
	best_local = int(valid_idx[int(np.argmax(scores))])

	# Move view to the chosen best angle.
	if using_setangle:
		best_ang = base_angle_deg + (360.0 * best_local / float(steps))
		_ = best_ang
	else:
		# Restore yaw back to the original pose.
		for _ in range(actual_steps):
			game.make_action([0, 0, 0, 0, 0, 0, float(-turn_step), 0.0], 1)
			if game.is_episode_finished():
				break
		# Turn to the best offset.
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
	invulnerable: bool = False,
) -> List[Candidate]:
	from vizdoom import GameVariable

	rng = np.random.default_rng(seed)

	# Always include the initial player spawn location as a valid screenshot candidate.
	# Also use it as a fallback if exploration yields no candidates.
	candidates: List[Candidate] = []
	seen = set()
	spawn_candidate: Optional[Candidate] = None

	def _maybe_capture_spawn_candidate() -> Optional[Candidate]:
		# Best-effort: ensure pitch is centered and we have a valid state.
		try:
			_center_pitch(game)
			game.make_action([0, 0, 0, 0, 0, 0, 0.0, 0.0], 1)
		except Exception:
			pass
		cand = _capture_best_yaw_sweep(game, pickup=False)
		return cand

	def _add_if_new(cand: Candidate) -> None:
		if cand.pickup:
			key = (
				int(cand.x // 32.0),
				int(cand.y // 32.0),
				int(cand.z // 16.0),
				int(_wrap_angle_deg(cand.angle) // 12.0),
				0,
			)
		else:
			key = (
				int(cand.x // 32.0),
				int(cand.y // 32.0),
				int(cand.z // 16.0),
				int(_wrap_angle_deg(cand.angle) // 12.0),
				int(_clamp(cand.pitch, -89.0, 89.0) // 8.0),
			)
		if key in seen:
			return
		seen.add(key)
		candidates.append(cand)

	spawn_candidate = _maybe_capture_spawn_candidate()
	if spawn_candidate is not None:
		_add_if_new(spawn_candidate)

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
			_new_episode(game, invulnerable=invulnerable)
			# If we failed to capture the initial spawn candidate, retry after respawn.
			if spawn_candidate is None:
				spawn_candidate = _maybe_capture_spawn_candidate()
				if spawn_candidate is not None:
					_add_if_new(spawn_candidate)

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
			_new_episode(game, invulnerable=invulnerable)
			# If we failed to capture the initial spawn candidate, retry after respawn.
			if spawn_candidate is None:
				spawn_candidate = _maybe_capture_spawn_candidate()
				if spawn_candidate is not None:
					_add_if_new(spawn_candidate)
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
				_add_if_new(cand)
				if len(candidates) >= target_candidates:
					break

		if t % keep_every != 0:
			continue

		cand = _capture_best_yaw_sweep(game, pickup=False)
		if cand is None:
			continue

		_add_if_new(cand)

		if len(candidates) >= target_candidates:
			break

	# Hard fallback: if everything failed, return at least the spawn candidate.
	if not candidates and spawn_candidate is not None:
		candidates = [spawn_candidate]

	# Shuffle slightly so selection doesn't always favor early frames.
	rng.shuffle(candidates)
	return candidates


def _save_image(arr: np.ndarray, out_path: Path, fmt: str, quality: int, wad_id: Optional[str], map_name: str) -> None:
	_ensure_parent_dir(out_path)
	img = Image.fromarray(arr, mode="RGB")
	fmt_u = fmt.upper()
	if fmt_u in ("JPG", "JPEG"):
		img.save(out_path, format="JPEG", quality=quality, optimize=True)
	elif fmt_u == "PNG":
		img.save(out_path, format="PNG", optimize=True)
	elif fmt_u == "WEBP":
		img.save(out_path, format="WEBP", quality=quality, method=6)
	else:
		raise ValueError(f"Unknown format: {fmt}")
	if wad_id is not None:
		print(f"🖼️  Saved image for {wad_id} {map_name}: {out_path}")
	else:
		print(f"🖼️  Saved image for {map_name}: {out_path}")

def _signed_angle_delta_deg(target: float, current: float) -> float:
	# Return signed delta in [-180, 180].
	d = ((target - current + 540.0) % 360.0) - 180.0
	return float(d)


def _get_game_var_fallback(game, index: int) -> Optional[float]:
	"""Best-effort game variable read.

	Some ViZDoom builds expose variables reliably in `state.game_variables` but may
	throw when using `get_game_variable` for the same variable.
	"""
	try:
		st = game.get_state()
		if st is None or st.game_variables is None:
			return None
		gv = st.game_variables
		if len(gv) <= index:
			return None
		return float(gv[index])
	except Exception:
		return None


def _get_yaw_deg(game) -> Optional[float]:
	from vizdoom import GameVariable

	try:
		return _wrap_angle_deg(float(game.get_game_variable(GameVariable.ANGLE)))
	except Exception:
		v = _get_game_var_fallback(game, 3)
		return _wrap_angle_deg(v) if v is not None else None


def _get_pitch_deg(game) -> Optional[float]:
	from vizdoom import GameVariable

	try:
		return float(game.get_game_variable(GameVariable.PITCH))
	except Exception:
		return _get_game_var_fallback(game, 4)


def _turn_to_yaw(game, *, target_yaw_deg: float, max_steps: int = 80, tol_deg: float = 1.0) -> None:
	target = _wrap_angle_deg(float(target_yaw_deg))
	for _ in range(max_steps):
		cur = _get_yaw_deg(game)
		if cur is None:
			return
		d = _signed_angle_delta_deg(target, cur)
		if abs(d) <= tol_deg:
			return
		turn = _clamp(d * 0.55, -20.0, 20.0)
		game.make_action([0, 0, 0, 0, 0, 0, float(turn), 0.0], 1)
		if game.is_episode_finished():
			return


def _look_to_pitch(game, *, target_pitch_deg: float, max_steps: int = 80, tol_deg: float = 1.0) -> None:
	target = float(target_pitch_deg)
	for _ in range(max_steps):
		cur = _get_pitch_deg(game)
		if cur is None:
			return
		d = target - cur
		if abs(d) <= tol_deg:
			return
		look = _clamp(d * 0.75, -12.0, 12.0)
		game.make_action([0, 0, 0, 0, 0, 0, 0.0, float(look)], 1)
		if game.is_episode_finished():
			return


def _state_to_rgb(game) -> Optional[np.ndarray]:
	state = game.get_state()
	if state is None or state.screen_buffer is None:
		return None
	# ViZDoom provides HxWx3 uint8 already for RGB24.
	return np.array(state.screen_buffer, copy=True)


def _center_crop_square(arr: np.ndarray) -> np.ndarray:
	if arr.ndim != 3 or arr.shape[2] != 3:
		raise ValueError("Expected HxWx3 RGB")
	h, w = int(arr.shape[0]), int(arr.shape[1])
	s = min(h, w)
	y0 = (h - s) // 2
	x0 = (w - s) // 2
	return arr[y0 : y0 + s, x0 : x0 + s, :]


def _resize_rgb(arr: np.ndarray, size: int) -> np.ndarray:
	if int(arr.shape[0]) == int(size) and int(arr.shape[1]) == int(size):
		return arr
	img = Image.fromarray(arr, mode="RGB")
	img = img.resize((int(size), int(size)), resample=Image.BICUBIC)
	return np.array(img, dtype=np.uint8)


def _bilinear_sample_rgb(img: np.ndarray, xs: np.ndarray, ys: np.ndarray) -> np.ndarray:
	# img: HxWx3, xs/ys: (N,) float pixel coords.
	h, w = int(img.shape[0]), int(img.shape[1])
	xs = np.clip(xs, 0.0, float(w - 1))
	ys = np.clip(ys, 0.0, float(h - 1))

	x0 = np.floor(xs).astype(np.int32)
	y0 = np.floor(ys).astype(np.int32)
	x1 = np.clip(x0 + 1, 0, w - 1)
	y1 = np.clip(y0 + 1, 0, h - 1)

	xf = xs - x0.astype(np.float32)
	yf = ys - y0.astype(np.float32)

	# Weights
	wa = (1.0 - xf) * (1.0 - yf)
	wb = xf * (1.0 - yf)
	wc = (1.0 - xf) * yf
	wd = xf * yf

	Ia = img[y0, x0].astype(np.float32)
	Ib = img[y0, x1].astype(np.float32)
	Ic = img[y1, x0].astype(np.float32)
	Id = img[y1, x1].astype(np.float32)

	out = Ia * wa[:, None] + Ib * wb[:, None] + Ic * wc[:, None] + Id * wd[:, None]
	return np.clip(out + 0.5, 0.0, 255.0).astype(np.uint8)


def _cubemap_faces_to_equirect(
	*,
	front: np.ndarray,
	right: np.ndarray,
	back: np.ndarray,
	left: np.ndarray,
	up: np.ndarray,
	down: np.ndarray,
	out_width: int,
	out_height: int,
) -> np.ndarray:
	# We map to a conventional cubemap (posx/negx/posy/negy/posz/negz):
	# - posz = front, negz = back
	# - posx = right, negx = left
	# - posy = up,    negy = down
	faces = {
		"posx": right,
		"negx": left,
		"posy": up,
		"negy": down,
		"posz": front,
		"negz": back,
	}

	# Validate square size.
	s = int(front.shape[0])
	for k, v in faces.items():
		if v.ndim != 3 or v.shape[2] != 3 or int(v.shape[0]) != s or int(v.shape[1]) != s:
			raise ValueError(f"Cubemap face {k} must be {s}x{s}x3")

	w = int(out_width)
	h = int(out_height)
	# Pixel centers in [0,1]
	uu = (np.arange(w, dtype=np.float32) + 0.5) / float(w)
	vv = (np.arange(h, dtype=np.float32) + 0.5) / float(h)
	# lon in [-pi, pi], lat in [pi/2, -pi/2]
	lon = uu * (2.0 * math.pi) - math.pi
	lat = (0.5 - vv) * math.pi
	lon_g, lat_g = np.meshgrid(lon, lat)

	clat = np.cos(lat_g)
	dx = clat * np.cos(lon_g)
	dy = np.sin(lat_g)
	dz = clat * np.sin(lon_g)

	dx_f = dx.reshape(-1)
	dy_f = dy.reshape(-1)
	dz_f = dz.reshape(-1)

	ax = np.abs(dx_f)
	ay = np.abs(dy_f)
	az = np.abs(dz_f)

	out = np.empty((h * w, 3), dtype=np.uint8)

	# Helper to sample a face for a given mask and (sc, tc) in [-1,1].
	def sample_into(mask: np.ndarray, face_key: str, sc: np.ndarray, tc: np.ndarray) -> None:
		if not np.any(mask):
			return
		# Convert to pixel coords (0..s-1). tc=+1 is top.
		px = (sc + 1.0) * 0.5 * float(s - 1)
		py = (1.0 - (tc + 1.0) * 0.5) * float(s - 1)
		cols = _bilinear_sample_rgb(faces[face_key], px.astype(np.float32), py.astype(np.float32))
		out[mask] = cols

	# Major axis selection.
	use_x = (ax >= ay) & (ax >= az)
	use_y = (ay > ax) & (ay >= az)
	use_z = (az > ax) & (az > ay)

	# X faces.
	mask = use_x & (dx_f > 0)
	sc = (-dz_f[mask]) / ax[mask]
	tc = (-dy_f[mask]) / ax[mask]
	sample_into(mask, "posx", sc, tc)

	mask = use_x & (dx_f <= 0)
	sc = (dz_f[mask]) / ax[mask]
	tc = (-dy_f[mask]) / ax[mask]
	sample_into(mask, "negx", sc, tc)

	# Y faces.
	mask = use_y & (dy_f > 0)
	sc = (dx_f[mask]) / ay[mask]
	tc = (dz_f[mask]) / ay[mask]
	sample_into(mask, "posy", sc, tc)

	mask = use_y & (dy_f <= 0)
	sc = (dx_f[mask]) / ay[mask]
	tc = (-dz_f[mask]) / ay[mask]
	sample_into(mask, "negy", sc, tc)

	# Z faces.
	mask = use_z & (dz_f > 0)
	sc = (dx_f[mask]) / az[mask]
	tc = (-dy_f[mask]) / az[mask]
	sample_into(mask, "posz", sc, tc)

	mask = use_z & (dz_f <= 0)
	sc = (-dx_f[mask]) / az[mask]
	tc = (-dy_f[mask]) / az[mask]
	sample_into(mask, "negz", sc, tc)

	return out.reshape((h, w, 3))


def _capture_panorama_bundle(
	*,
	game,
	base_front_rgb: np.ndarray,
	base_yaw_deg: float,
	face_size: int,
	turn_yaw_tol_deg: float = 1.0,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
	# Capture the 6 cubemap faces (front/right/back/left/up/down).
	# We keep the existing front RGB (from the candidate selection) and capture the other faces.
	s = int(face_size)
	# NOTE: Some ViZDoom builds can be flaky about absolute ANGLE reads. For cubemap capture,
	# use relative TURN deltas (same mechanism used in yaw sweep) to guarantee rotation.
	_ = base_yaw_deg
	_ = turn_yaw_tol_deg

	def settle(ticks: int = 2) -> None:
		for _i in range(int(ticks)):
			game.make_action([0, 0, 0, 0, 0, 0, 0.0, 0.0], 1)
			if game.is_episode_finished():
				return

	def turn_relative(delta_yaw_deg: float) -> None:
		remaining = float(delta_yaw_deg)
		# TURN_LEFT_RIGHT_DELTA is capped to 20.0 by our game init.
		for _i in range(256):
			if game.is_episode_finished():
				return
			if abs(remaining) <= 0.5:
				break
			step = _clamp(remaining, -20.0, 20.0)
			game.make_action([0, 0, 0, 0, 0, 0, float(step), 0.0], 1)
			remaining -= step

	def grab_current(pitch: float) -> np.ndarray:
		_look_to_pitch(game, target_pitch_deg=float(pitch))
		settle(2)
		if game.is_episode_finished():
			raise RuntimeError("Episode finished while capturing cubemap face")
		rgb = _state_to_rgb(game)
		if rgb is None:
			raise RuntimeError("Failed to capture cubemap face (no state/screen_buffer)")
		return _resize_rgb(_center_crop_square(rgb), s)

	# Ensure pitch is centered before doing yaw-relative turns.
	_look_to_pitch(game, target_pitch_deg=0.0)
	settle(2)

	front = _resize_rgb(_center_crop_square(np.array(base_front_rgb, copy=False)), s)

	turn_relative(90.0)
	right = grab_current(0.0)
	turn_relative(90.0)
	back = grab_current(0.0)
	turn_relative(90.0)
	left = grab_current(0.0)
	turn_relative(90.0)  # restore to front

	# ZDoom pitch range is usually about [-89, +89]. Use 89 to avoid clamping artifacts.
	up = grab_current(89.0)
	down = grab_current(-89.0)

	# Restore to centered pitch so the rest of the pipeline stays stable.
	_look_to_pitch(game, target_pitch_deg=0.0)

	return front, right, back, left, up, down


@dataclass(frozen=True)
class RenderConfig:
	iwad: Path
	files: Sequence[Path]
	output: Path
	num: int = 16
	seed: int = 1234
	width: int = 800
	height: int = 600
	format: str = "jpg"
	jpeg_quality: int = 92
	webp_quality: int = 70
	panorama: bool = False
	panorama_format: str = "jpg"
	panorama_face_size: int = 0
	panorama_width: int = 0
	panorama_height: int = 0
	visible: bool = False
	no_monsters: bool = False
	invulnerable: bool = False
	skill: int = 3
	episode_timeout: int = 6000
	warmup_steps: int = 40
	max_steps: int = 8000
	frame_skip: int = 4
	keep_every: int = 6
	prefer_gpu: bool = False
	wad_id: Optional[str] = None


def list_maps(iwad: Path, files: Sequence[Path]) -> List[str]:
	"""Return detected map markers in effective load order."""
	return _effective_map_list(Path(iwad), [Path(p) for p in files])


class NoMapsError(ScreenshotsError):
	def __init__(self) -> None:
		super().__init__("No maps detected in IWAD/--files (WAD/PK3 map detection found none).")

def render_screenshots(config: RenderConfig) -> Dict[str, int]:
	"""Render screenshots for all maps and return {map_name: saved_count}.

	This function is intended for programmatic use (importing from other scripts).
	It does not call sys.exit; errors are raised as exceptions.
	"""
	iwad = Path(config.iwad)
	files = [Path(p) for p in config.files]
	out_root = Path(config.output)
	out_root.mkdir(parents=True, exist_ok=True)

	maps = _effective_map_list(iwad, files)
	if not maps:
		raise NoMapsError()
	if int(config.num) <= 0:
		raise ScreenshotsError("num must be > 0")
	print(f"🔍 Detected {len(maps)} maps to render screenshots for.")

	# Import VizDoom only when actually rendering.
	import vizdoom  # noqa: F401

	results: Dict[str, int] = {}
	for mi, map_name in enumerate(maps):
		map_seed = int((int(config.seed) * 1000003 + mi * 9176) & 0x7FFFFFFF)

		# New approach: choose globally-distributed pickup points as start locations.
		pickup_points = _pickup_points_for_map(iwad, files, map_name)
		# Use an oversized, spread-out candidate set so we can skip failed teleports
		# (some points can be unreachable/invalid due to Z or blocking).
		starts = _spread_out_points(pickup_points, n=int(config.num) * 6, seed=map_seed)

		game = _init_game(
			iwad=iwad,
			files=files,
			map_name=map_name,
			seed=map_seed,
			width=int(config.width),
			height=int(config.height),
			visible=bool(config.visible),
			no_monsters=bool(config.no_monsters),
			skill=int(config.skill),
			episode_timeout=max(int(config.episode_timeout), int(config.max_steps) * int(config.frame_skip) + 1000),
		)
		try:
			ext = str(config.format)
			quality = int(config.webp_quality) if str(config.format) == "webp" else int(config.jpeg_quality)
			pano_face_size = (
				int(config.panorama_face_size)
				if int(config.panorama_face_size) > 0
				else int(min(int(config.width), int(config.height)))
			)
			pano_w = int(config.panorama_width) if int(config.panorama_width) > 0 else int(4 * pano_face_size)
			pano_h = int(config.panorama_height) if int(config.panorama_height) > 0 else int(2 * pano_face_size)
			pano_quality = int(config.jpeg_quality)
			map_dir = out_root / map_name
			map_dir.mkdir(parents=True, exist_ok=True)
			saved = 0

			if starts:
				# Tele[port directly to globally-distributed pickup coordinates.
				from vizdoom import GameVariable
				_new_episode(game, invulnerable=bool(config.invulnerable))
				try:
					start_x = float(game.get_game_variable(GameVariable.POSITION_X))
					start_y = float(game.get_game_variable(GameVariable.POSITION_Y))
				except Exception:
					start_x, start_y = 0.0, 0.0
				# Visit far targets first.
				targets = sorted(starts, key=lambda p: -math.hypot(p[0] - start_x, p[1] - start_y))
				used_xy: List[Tuple[float, float]] = []
				rng = np.random.default_rng(map_seed ^ 0x5F3759DF)
				idx = 0
				for tx, ty in targets:
					if saved >= int(config.num):
						break
					if any(math.hypot(tx - ux, ty - uy) < 768.0 for ux, uy in used_xy):
						continue

					_new_episode(game, invulnerable=bool(config.invulnerable))
					ok = _teleport_to(game, x=float(tx), y=float(ty))
					_center_pitch(game)
					if not ok:
						continue
					try:
						px = float(game.get_game_variable(GameVariable.POSITION_X))
						py = float(game.get_game_variable(GameVariable.POSITION_Y))
					except Exception:
						px, py = float(tx), float(ty)
					used_xy.append((px, py))

					base_angle = float(rng.uniform(0.0, 360.0))
					best = _best_direction_at_location(
						game,
						prefer_gpu=bool(config.prefer_gpu),
						base_angle_deg=base_angle,
					)
					if best is None:
						continue
					_save_image(best.screen,
				 				map_dir / "images" / f"{idx}.{ext}",
								fmt=str(config.format),
								quality=quality,
								wad_id=config.wad_id,
								map_name=map_name)
					if bool(config.panorama):
						try:
							front, right, back, left, up, down = _capture_panorama_bundle(
								game=game,
								base_front_rgb=best.screen,
								base_yaw_deg=float(best.angle),
								face_size=pano_face_size,
							)
							pano = _cubemap_faces_to_equirect(
								front=front,
								right=right,
								back=back,
								left=left,
								up=up,
								down=down,
								out_width=pano_w,
								out_height=pano_h,
							)
							_save_image(
								pano,
								map_dir / "pano" / f"pano_{idx}.{str(config.panorama_format)}",
								fmt=str(config.panorama_format),
								quality=pano_quality,
								wad_id=config.wad_id,
								map_name=map_name
							)
						except Exception as e:
							print(f"⚠️ {map_name}: panorama capture failed for shot {idx}: {e}", file=sys.stderr)
					saved += 1
					idx += 1

				# If some pickup teleports fail (unreachable/invalid coordinates),
				# fill the remainder using exploration-based candidates.
				if saved < int(config.num):
					_new_episode(game, invulnerable=bool(config.invulnerable))
					candidates = _gather_candidates(
						game=game,
						n=int(config.num),
						seed=map_seed ^ 0xA53A9E21,
						warmup_steps=int(config.warmup_steps),
						max_steps=int(config.max_steps),
						frame_skip=int(config.frame_skip),
						keep_every=int(config.keep_every),
						invulnerable=bool(config.invulnerable),
					)
					selected = _select_diverse(candidates, n=int(config.num) - saved)
					for j, cand in enumerate(selected, start=idx):
						out_path = map_dir / "images" / f"{j}.{ext}"
						_save_image(cand.screen,
				  					out_path,
									fmt=str(config.format),
									quality=quality,
									wad_id=config.wad_id,
									map_name=map_name)
						if bool(config.panorama):
							try:
								front, right, back, left, up, down = _capture_panorama_bundle(
									game=game,
									base_front_rgb=cand.screen,
									base_yaw_deg=float(cand.angle),
									face_size=pano_face_size,
								)
								#_save_image(front, map_dir / f"{j}_front.{ext}", fmt=str(config.format), quality=quality)
								#_save_image(right, map_dir / f"{j}_right.{ext}", fmt=str(config.format), quality=quality)
								#_save_image(back, map_dir / f"{j}_back.{ext}", fmt=str(config.format), quality=quality)
								#_save_image(left, map_dir / f"{j}_left.{ext}", fmt=str(config.format), quality=quality)
								#_save_image(up, map_dir / f"{j}_up.{ext}", fmt=str(config.format), quality=quality)
								#_save_image(down, map_dir / f"{j}_down.{ext}", fmt=str(config.format), quality=quality)
								pano = _cubemap_faces_to_equirect(
									front=front,
									right=right,
									back=back,
									left=left,
									up=up,
									down=down,
									out_width=pano_w,
									out_height=pano_h,
								)
								_save_image(
									pano,
									map_dir / "pano" / f"pano_{j}.{str(config.panorama_format)}",
									fmt=str(config.panorama_format),
									quality=pano_quality,
									wad_id=config.wad_id,
									map_name=map_name
								)
							except Exception as e:
								print(f"⚠️ {map_name}: panorama capture failed for shot {j}: {e}", file=sys.stderr)
						saved += 1
			else:
				# Fallback to exploration if the map has no parseable pickups.
				_new_episode(game, invulnerable=bool(config.invulnerable))
				candidates = _gather_candidates(
					game=game,
					n=int(config.num),
					seed=map_seed,
					warmup_steps=int(config.warmup_steps),
					max_steps=int(config.max_steps),
					frame_skip=int(config.frame_skip),
					keep_every=int(config.keep_every),
					invulnerable=bool(config.invulnerable),
				)
				selected = _select_diverse(candidates, n=int(config.num))
				for i, cand in enumerate(selected):
					out_path = map_dir / "images" / f"{i}.{ext}"
					_save_image(cand.screen,
				 				out_path,
								fmt=str(config.format),
								quality=quality,
								wad_id=config.wad_id,
								map_name=map_name)
					if bool(config.panorama):
						try:
							front, right, back, left, up, down = _capture_panorama_bundle(
								game=game,
								base_front_rgb=cand.screen,
								base_yaw_deg=float(cand.angle),
								face_size=pano_face_size,
							)
							#_save_image(front, map_dir / f"{i}_front.{ext}", fmt=str(config.format), quality=quality)
							#_save_image(right, map_dir / f"{i}_right.{ext}", fmt=str(config.format), quality=quality)
							#_save_image(back, map_dir / f"{i}_back.{ext}", fmt=str(config.format), quality=quality)
							#_save_image(left, map_dir / f"{i}_left.{ext}", fmt=str(config.format), quality=quality)
							#_save_image(up, map_dir / f"{i}_up.{ext}", fmt=str(config.format), quality=quality)
							#_save_image(down, map_dir / f"{i}_down.{ext}", fmt=str(config.format), quality=quality)
							pano = _cubemap_faces_to_equirect(
								front=front,
								right=right,
								back=back,
								left=left,
								up=up,
								down=down,
								out_width=pano_w,
								out_height=pano_h,
							)
							_save_image(
								pano,
								map_dir / "pano" / f"pano_{i}.{str(config.panorama_format)}",
								fmt=str(config.panorama_format),
								quality=pano_quality,
								wad_id=config.wad_id,
								map_name=map_name
							)
						except Exception as e:
							print(f"⚠️ {map_name}: panorama capture failed for shot {i}: {e}", file=sys.stderr)
					saved += 1

			results[map_name] = int(saved)
		finally:
			game.close()

	return results


def main(argv: Optional[Sequence[str]] = None) -> int:
	parser = argparse.ArgumentParser(
		description=(
			"Render N diverse first-person screenshots for every map in a WAD using ViZDoom.\n"
			"Default output layout: ${output}/${map_name}/${i}.${format}"
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
	parser.add_argument("-n", "--num", type=int, default=5, help="Screenshots per map")
	parser.add_argument("--seed", type=int, default=1234, help="Base RNG seed")
	parser.add_argument("--width", type=int, default=800)
	parser.add_argument("--height", type=int, default=600)
	parser.add_argument("--format", choices=["jpg", "png", "webp"], default="jpg")
	parser.add_argument("--jpeg-quality", type=int, default=92)
	parser.add_argument("--webp-quality", type=int, default=70)
	parser.add_argument(
		"--panorama",
		action="store_true",
		help=(
			"Also write a 6-sided cubemap and a stitched equirectangular web panorama per shot. "
			"When enabled, {i}.{format} is the front face, plus 5 additional faces, plus pano_{i}.(jpg|png)."
		),
	)
	parser.add_argument(
		"--panorama-format",
		choices=["jpg", "png", "webp"],
		default="jpg",
		help="Format for the stitched equirect panorama image (jpg, png, or webp)",
	)
	parser.add_argument(
		"--panorama-face-size",
		type=int,
		default=0,
		help="Cubemap face size in pixels (square). Default: min(--width,--height)",
	)
	parser.add_argument(
		"--panorama-width",
		type=int,
		default=0,
		help="Equirect panorama width in pixels. Default: 4*face_size",
	)
	parser.add_argument(
		"--panorama-height",
		type=int,
		default=0,
		help="Equirect panorama height in pixels. Default: 2*face_size",
	)
	parser.add_argument("--visible", action="store_true", help="Show the VizDoom window")
	parser.add_argument("--no-monsters", action="store_true", help="Pass -nomonsters")
	parser.add_argument(
		"--invulnerable",
		action="store_true",
		help="Enable ZDoom god mode (invulnerability) after each episode start",
	)
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
	if bool(args.list_maps):
		for m in maps:
			print(m)
		return 0

	if not maps:
		print("🕳️ No maps detected in IWAD/--files (WAD/PK3 map detection found none).", file=sys.stderr)
		return 2
	if int(args.num) <= 0:
		print("🚫 --num must be > 0", file=sys.stderr)
		return 2

	config = RenderConfig(
		iwad=iwad,
		files=files,
		output=out_root,
		num=int(args.num),
		seed=int(args.seed),
		width=int(args.width),
		height=int(args.height),
		format=str(args.format),
		jpeg_quality=int(args.jpeg_quality),
		webp_quality=int(args.webp_quality),
		panorama=bool(args.panorama),
		panorama_format=str(args.panorama_format),
		panorama_face_size=int(args.panorama_face_size),
		panorama_width=int(args.panorama_width),
		panorama_height=int(args.panorama_height),
		visible=bool(args.visible),
		no_monsters=bool(args.no_monsters),
		invulnerable=bool(args.invulnerable),
		skill=int(args.skill),
		episode_timeout=int(args.episode_timeout),
		warmup_steps=int(args.warmup_steps),
		max_steps=int(args.max_steps),
		frame_skip=int(args.frame_skip),
		keep_every=int(args.keep_every),
		prefer_gpu=bool(args.prefer_gpu),
	)

	try:
		results = render_screenshots(config)
		for map_name in maps:
			saved = int(results.get(map_name, 0))
			print(f"{map_name}: saved {saved}/{int(args.num)} images")
	except NoMapsError as e:
		print(str(e), file=sys.stderr)
		return 0
	except ScreenshotsError as e:
		print(str(e), file=sys.stderr)
		return 2
	except KeyboardInterrupt:
		print("Interrupted (Ctrl-C)")
		return 0
	except Exception as e:
		# ViZDoom can raise SignalException on SIGINT; treat that as a clean interrupt.
		if type(e).__name__ == "SignalException" and "SIGINT" in str(e):
			print("Interrupted (SIGINT)")
			return 0
		raise

	return 0


if __name__ == "__main__":
	raise SystemExit(main())
