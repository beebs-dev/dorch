#!/usr/bin/env python3
"""Extract per-map *friendly* names (titles) from a classic Doom WAD.

This script tries, in order:
1) MAPINFO-family text lumps (MAPINFO/ZMAPINFO/EMAPINFO/DMAPINFO/UMAPINFO)
   - supports map header titles (e.g. `map MAP01 "Title"`)
   - supports `levelname = "..."` inside map blocks
   - supports `lookup "KEY"` / `lookup = "KEY"` resolved via LANGUAGE/BEX
2) Fallback to vanilla Doom/Doom II built-in titles based on the map marker.

By default it prints a Python list literal (so you can paste it into code).
"""

from __future__ import annotations

import argparse
import json
import os
import re
import struct
from dataclasses import dataclass
from typing import Dict, Iterable, List, Optional, Tuple


# -----------------------------
# WAD parsing
# -----------------------------


@dataclass(frozen=True)
class WadLump:
    index: int
    name: str
    offset: int
    size: int


WAD_HEADER_RE = re.compile(rb"^(IWAD|PWAD)$")


def _read_u32le(b: bytes, off: int) -> int:
    return struct.unpack_from("<I", b, off)[0]


def parse_wad_directory(path: str) -> Tuple[str, int, List[WadLump]]:
    with open(path, "rb") as f:
        header = f.read(12)
        if len(header) != 12:
            raise ValueError("File too small to be a WAD")

        ident = header[0:4]
        if not WAD_HEADER_RE.match(ident):
            raise ValueError(f"Not a WAD (signature={ident!r})")
        wad_type = ident.decode("ascii", errors="replace")
        lump_count = int.from_bytes(header[4:8], "little", signed=False)
        dir_offset = int.from_bytes(header[8:12], "little", signed=False)

        if lump_count > 200_000:
            raise ValueError("Unreasonable lump count")

        f.seek(0, os.SEEK_END)
        file_size = f.tell()
        dir_size = lump_count * 16
        if dir_offset + dir_size > file_size:
            raise ValueError("Directory extends past EOF")

        f.seek(dir_offset)
        directory = f.read(dir_size)
        if len(directory) != dir_size:
            raise ValueError("Failed to read WAD directory")

    lumps: List[WadLump] = []
    for i in range(lump_count):
        base = i * 16
        lump_off = _read_u32le(directory, base + 0)
        lump_size = _read_u32le(directory, base + 4)
        raw_name = directory[base + 8 : base + 16]
        name = raw_name.split(b"\x00", 1)[0].decode("ascii", errors="replace")
        lumps.append(WadLump(index=i, name=name, offset=lump_off, size=lump_size))

    return wad_type, file_size, lumps


MAP_MARKER_RE = re.compile(r"^(E[1-9]M[1-9]|MAP[0-9]{2})$", re.IGNORECASE)


def detect_maps_from_lumps(lumps: List[WadLump]) -> List[str]:
    """Return map markers in on-disk order.

    Uses a conservative heuristic: marker must be followed soon by core lumps.
    """
    core = {"THINGS", "LINEDEFS", "SIDEDEFS", "VERTEXES", "SECTORS"}
    names = [l.name.upper() for l in lumps]

    found: List[str] = []
    for i, n in enumerate(names):
        if MAP_MARKER_RE.match(n):
            window = set(names[i + 1 : i + 1 + 16])
            if core.issubset(window):
                found.append(n)
    # preserve order, dedupe
    seen = set()
    out: List[str] = []
    for m in found:
        if m not in seen:
            seen.add(m)
            out.append(m)
    return out


# -----------------------------
# Text extraction
# -----------------------------


TEXT_LUMP_NAMES = {
    "MAPINFO",
    "ZMAPINFO",
    "EMAPINFO",
    "DMAPINFO",
    "UMAPINFO",
    "LANGUAGE",
    "BEX",
    "DEHACKED",
}


def _safe_text_decode(b: bytes) -> str:
    try:
        return b.decode("utf-8")
    except UnicodeDecodeError:
        return b.decode("latin-1", errors="replace")


def _normalize_whitespace(s: str) -> str:
    s = s.replace("\r\n", "\n").replace("\r", "\n")
    s = re.sub(r"[ \t]+\n", "\n", s)
    s = re.sub(r"\n{3,}", "\n\n", s)
    return s.strip()


def _strip_mapinfo_comments(text: str) -> str:
    # Remove /* ... */ blocks first, then // line comments.
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    text = re.sub(r"//.*?$", "", text, flags=re.MULTILINE)
    return text


def extract_text_lumps(path: str, lumps: List[WadLump], max_each: int = 256_000) -> Dict[str, str]:
    out: Dict[str, str] = {}
    with open(path, "rb") as f:
        for l in lumps:
            name = l.name.upper()
            if name not in TEXT_LUMP_NAMES:
                continue
            if l.size <= 0 or l.size > max_each:
                continue
            f.seek(l.offset)
            chunk = f.read(l.size)
            if not chunk:
                continue

            # Skip obviously-binary blobs (DEHACKED/BEX can contain some nulls).
            if b"\x00" in chunk[:256] and name not in {"DEHACKED", "BEX"}:
                continue

            text = _normalize_whitespace(_safe_text_decode(chunk))
            if text:
                out[name] = text
    return out


# -----------------------------
# LANGUAGE/BEX string tables
# -----------------------------


_KV_QUOTED_RE = re.compile(r"(?m)^\s*([A-Za-z0-9_$.]+)\s*=\s*\"([^\"]*)\"\s*;?\s*$")
_KV_BARE_RE = re.compile(r"(?m)^\s*([A-Za-z0-9_$.]+)\s*=\s*([^\r\n;]+)\s*;?\s*$")


def _parse_bex_strings(text: str) -> Dict[str, str]:
    # Boom's BEX has a [STRINGS] section with KEY = VALUE lines.
    # Values are often unquoted; keep conservative.
    t = _strip_mapinfo_comments(text)
    lines = t.splitlines()
    in_strings = False
    out: Dict[str, str] = {}
    for line in lines:
        raw = line.strip()
        if not raw:
            continue
        if raw.startswith("[") and raw.endswith("]"):
            in_strings = raw.strip("[]").strip().lower() == "strings"
            continue
        if not in_strings:
            continue

        m = _KV_QUOTED_RE.match(line)
        if m:
            out[m.group(1).strip()] = m.group(2)
            continue
        m = _KV_BARE_RE.match(line)
        if m:
            key = m.group(1).strip()
            val = m.group(2).strip()
            # Trim surrounding quotes if present.
            if len(val) >= 2 and val[0] == '"' and val[-1] == '"':
                val = val[1:-1]
            out[key] = val
    return out


def _parse_language_strings(text: str) -> Dict[str, str]:
    # ZDoom LANGUAGE uses sections like: [enu default]
    # and KEY = "Value"; lines.
    t = _strip_mapinfo_comments(text)
    out: Dict[str, str] = {}
    for m in _KV_QUOTED_RE.finditer(t):
        out[m.group(1).strip()] = m.group(2)
    return out


def build_string_table(text_lumps: Dict[str, str]) -> Dict[str, str]:
    out: Dict[str, str] = {}
    lang = text_lumps.get("LANGUAGE")
    if lang:
        out.update(_parse_language_strings(lang))
    bex = text_lumps.get("BEX")
    if bex:
        out.update(_parse_bex_strings(bex))
    return out


# -----------------------------
# MAPINFO-family parsing (best-effort)
# -----------------------------


_MAP_DECL_RE = re.compile(
    r"(?im)^\s*map\s+([A-Za-z0-9]+)\b(?:(?:\s+\"([^\"]+)\")|(?:\s+lookup\s+\"([^\"]+)\"))?",
)


def _find_matching_brace(text: str, open_brace_pos: int) -> Optional[int]:
    depth = 0
    for i in range(open_brace_pos, len(text)):
        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
    return None


def _extract_map_block(text: str, decl_end: int) -> Optional[Tuple[int, int, str]]:
    # Find first '{' after the declaration.
    open_pos = text.find("{", decl_end)
    if open_pos < 0:
        return None
    close_pos = _find_matching_brace(text, open_pos)
    if close_pos is None:
        return None
    return (open_pos, close_pos, text[open_pos + 1 : close_pos])


def _pick_title_for_block(*, header_title: Optional[str], header_lookup: Optional[str], block_text: str, strings: Dict[str, str]) -> Optional[str]:
    if header_title and header_title.strip():
        return header_title.strip()

    # Prefer levelname/title/name assignments.
    for key in ("levelname", "title", "name"):
        m = re.search(rf'(?is)\b{re.escape(key)}\s*=\s*\"([^\"]+)\"', block_text)
        if m:
            return m.group(1).strip()

    # ZDoom-style lookup inside a block.
    m = re.search(r'(?is)\blookup\s*(?:=\s*)?\"([^\"]+)\"', block_text)
    if m:
        k = m.group(1).strip()
        v = strings.get(k)
        if v:
            return v.strip()

    # Header lookup as a last attempt.
    if header_lookup:
        v = strings.get(header_lookup.strip())
        if v:
            return v.strip()

    return None


def parse_map_titles_from_text(text_lumps: Dict[str, str], strings: Dict[str, str]) -> Dict[str, str]:
    """Return mapping map_marker -> friendly title."""
    combined: List[str] = []
    for k in ("UMAPINFO", "ZMAPINFO", "MAPINFO", "EMAPINFO", "DMAPINFO"):
        t = text_lumps.get(k)
        if t:
            combined.append(t)

    if not combined:
        return {}

    text = "\n\n".join(combined)
    text = _strip_mapinfo_comments(text)

    out: Dict[str, str] = {}
    for m in _MAP_DECL_RE.finditer(text):
        map_id = (m.group(1) or "").strip().upper()
        if not MAP_MARKER_RE.match(map_id):
            continue

        header_title = (m.group(2) or "").strip() or None
        header_lookup = (m.group(3) or "").strip() or None

        block = _extract_map_block(text, m.end())
        block_text = ""
        if block is not None:
            block_text = block[2]

        title = _pick_title_for_block(
            header_title=header_title,
            header_lookup=header_lookup,
            block_text=block_text,
            strings=strings,
        )
        if title:
            out[map_id] = title
    return out


# -----------------------------
# Vanilla fallback tables
# -----------------------------


DOOM2_TITLES: Dict[str, str] = {
    "MAP01": "Entryway",
    "MAP02": "Underhalls",
    "MAP03": "The Gantlet",
    "MAP04": "The Focus",
    "MAP05": "The Waste Tunnels",
    "MAP06": "The Crusher",
    "MAP07": "Dead Simple",
    "MAP08": "Tricks and Traps",
    "MAP09": "The Pit",
    "MAP10": "Refueling Base",
    "MAP11": "Circle of Death",
    "MAP12": "The Factory",
    "MAP13": "Downtown",
    "MAP14": "The Inmost Dens",
    "MAP15": "Industrial Zone",
    "MAP16": "Suburbs",
    "MAP17": "Tenements",
    "MAP18": "The Courtyard",
    "MAP19": "The Citadel",
    "MAP20": "Gotcha!",
    "MAP21": "Nirvana",
    "MAP22": "The Catacombs",
    "MAP23": "Barrels o' Fun",
    "MAP24": "The Chasm",
    "MAP25": "Bloodfalls",
    "MAP26": "The Abandoned Mines",
    "MAP27": "Monster Condo",
    "MAP28": "The Spirit World",
    "MAP29": "The Living End",
    "MAP30": "Icon of Sin",
    "MAP31": "Wolfenstein",
    "MAP32": "Grosse",
}


DOOM1_TITLES: Dict[str, str] = {
    "E1M1": "Hangar",
    "E1M2": "Nuclear Plant",
    "E1M3": "Toxin Refinery",
    "E1M4": "Command Control",
    "E1M5": "Phobos Lab",
    "E1M6": "Central Processing",
    "E1M7": "Computer Station",
    "E1M8": "Phobos Anomaly",
    "E1M9": "Military Base",
    "E2M1": "Deimos Anomaly",
    "E2M2": "Containment Area",
    "E2M3": "Refinery",
    "E2M4": "Deimos Lab",
    "E2M5": "Command Center",
    "E2M6": "Halls of the Damned",
    "E2M7": "Spawning Vats",
    "E2M8": "Tower of Babel",
    "E2M9": "Fortress of Mystery",
    "E3M1": "Hell Keep",
    "E3M2": "Slough of Despair",
    "E3M3": "Pandemonium",
    "E3M4": "House of Pain",
    "E3M5": "Unholy Cathedral",
    "E3M6": "Mt. Erebus",
    "E3M7": "Limbo",
    "E3M8": "Dis",
    "E3M9": "Warrens",
    "E4M1": "Hell Beneath",
    "E4M2": "Perfect Hatred",
    "E4M3": "Sever the Wicked",
    "E4M4": "Unruly Evil",
    "E4M5": "They Will Repent",
    "E4M6": "Against Thee Wickedly",
    "E4M7": "And Hell Followed",
    "E4M8": "Unto The Cruel",
    "E4M9": "Fear",
}


def fallback_title(map_marker: str, strings: Dict[str, str]) -> Optional[str]:
    """Best-effort map title without MAPINFO.

    Prefer string table overrides (LANGUAGE/BEX), then vanilla hard-coded tables.
    """
    mm = (map_marker or "").strip().upper()

    # LANGUAGE/BEX: Doom II style keys (HUSTR_1..HUSTR_32)
    m = re.fullmatch(r"MAP(\d\d)", mm)
    if m:
        idx = int(m.group(1))
        key = f"HUSTR_{idx}"
        v = strings.get(key)
        if v:
            return v.strip()

    # LANGUAGE/BEX: Doom I style keys (HUSTR_E1M1, etc.)
    m = re.fullmatch(r"E([1-9])M([1-9])", mm)
    if m:
        key = f"HUSTR_E{m.group(1)}M{m.group(2)}"
        v = strings.get(key)
        if v:
            return v.strip()

    # Built-in vanilla fallbacks
    if mm in DOOM1_TITLES:
        return DOOM1_TITLES[mm]
    if mm in DOOM2_TITLES:
        return DOOM2_TITLES[mm]
    return None


# -----------------------------
# Public API
# -----------------------------


def extract_friendly_map_names(wad_path: str) -> List[str]:
    _wad_type, _file_size, lumps = parse_wad_directory(wad_path)
    maps = detect_maps_from_lumps(lumps)
    text_lumps = extract_text_lumps(wad_path, lumps)
    strings = build_string_table(text_lumps)
    titles_by_map = parse_map_titles_from_text(text_lumps, strings)

    out: List[str] = []
    for m in maps:
        title = titles_by_map.get(m)
        if not title:
            title = fallback_title(m, strings)
        out.append(title or m)
    return out


def extract_map_title_pairs(wad_path: str) -> List[Tuple[str, str]]:
    _wad_type, _file_size, lumps = parse_wad_directory(wad_path)
    maps = detect_maps_from_lumps(lumps)
    text_lumps = extract_text_lumps(wad_path, lumps)
    strings = build_string_table(text_lumps)
    titles_by_map = parse_map_titles_from_text(text_lumps, strings)

    out: List[Tuple[str, str]] = []
    for m in maps:
        title = titles_by_map.get(m) or fallback_title(m, strings) or m
        out.append((m, title))
    return out


def _main() -> None:
    ap = argparse.ArgumentParser(
        description="Extract per-map friendly names (titles) from a WAD",
    )
    ap.add_argument("wad_path", help="Path to .wad")
    ap.add_argument(
        "--json",
        action="store_true",
        help="Print JSON instead of a Python list literal",
    )
    ap.add_argument(
        "--with-markers",
        action="store_true",
        help="Return list of (marker, title) pairs",
    )
    args = ap.parse_args()

    if args.with_markers:
        pairs = extract_map_title_pairs(args.wad_path)
        if args.json:
            print(json.dumps(pairs, indent=2))
        else:
            print(repr(pairs))
        return

    names = extract_friendly_map_names(args.wad_path)
    if args.json:
        print(json.dumps(names, indent=2))
    else:
        print(repr(names))


if __name__ == "__main__":
    _main()


