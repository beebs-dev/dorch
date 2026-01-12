#!/usr/bin/env python3
"""
Merge WAD Archive + idGames + on-disk extracted metadata into a single coherent JSON object.

Pipeline (per WAD):
1) Load wads.json + idgames.json
2) Build fast sha1 -> idgames entry lookup (only entries that link back to wads.json)
3) For each wads.json entry:
   - Resolve the S3 URL (prefix is unknown; probe a small set of likely prefixes)
   - Download to a temp dir
   - Decompress .gz if needed
   - Extract metadata from the file itself (best-effort):
       * WAD: lump counts, map markers, and embedded text lumps (MAPINFO/EMAPINFO/ZMAPINFO/DEHACKED/etc)
       * PK3/PK7/PKZ/EPK/PKE: scan archive for embedded WADs and text-like files; parse embedded WADs if found
   - Merge into a coherent JSON object with precedence:
       1) extracted
       2) wads.json
       3) linked idgames.json entry
   - Print the merged JSON
   - Delete temp files

Notes:
- This intentionally avoids the idGames "search" API. It only uses idgames.json linkages (hashes[]).
- S3 prefix guessing: attempts first two hex of sha1/md5/sha256 + a few fallbacks.
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import json
import os
import re
import struct
import sys
import tempfile
import time
import zipfile
from dataclasses import dataclass
from typing import Any, Dict, Iterable, List, Optional, Tuple

import requests

DEFAULT_S3_BASE = "https://wadarchive.nyc3.digitaloceanspaces.com"


# -----------------------------
# Per-map stats (ported from dump_wad_json.py)
# -----------------------------

MAP_RE = re.compile(r"^(MAP\d\d|E\dM\d)$")

KEY_THING_IDS = {
    5: "blue",
    6: "yellow",
    13: "red",
    38: "red_skull",
    39: "yellow_skull",
    40: "blue_skull",
}

MONSTER_THING_IDS = {
    3004: "zombieman",
    9: "shotgun_guy",
    65: "chaingun_guy",
    3001: "imp",
    3002: "demon",
    58: "spectre",
    3005: "cacodemon",
    3006: "lost_soul",
    16: "cyberdemon",
    7: "spider_mastermind",
    64: "archvile",
    66: "revenant",
    67: "mancubus",
    68: "arachnotron",
    69: "hell_knight",
    71: "pain_elemental",
    3003: "baron",
}

# Common Doom item/pickup thing types (vanilla).
# Used for a per-map item breakdown similar to the monster breakdown.
ITEM_THING_IDS = {
    # Weapons
    2001: "shotgun",
    82: "super_shotgun",
    2002: "chaingun",
    2003: "rocket_launcher",
    2004: "plasma_rifle",
    2005: "chainsaw",
    2006: "bfg9000",

    # Ammo
    2007: "ammo_clip",
    2048: "ammo_box",
    2008: "shells",
    2049: "shell_box",
    2010: "rocket",
    2046: "rocket_box",
    2047: "cell",
    17: "cell_pack",
    8: "backpack",

    # Health / Armor
    2011: "stimpack",
    2012: "medikit",
    2014: "health_bonus",
    2015: "armor_bonus",
    2018: "green_armor",
    2019: "blue_armor",
    2013: "soulsphere",
    83: "megasphere",

    # Powerups
    2023: "berserk",
    2022: "invulnerability",
    2024: "invisibility",
    2025: "radiation_suit",
    2026: "computer_area_map",
    2045: "light_amp_goggles",
}

SECRET_EXIT_SPECIALS = {51, 124, 198}
TELEPORT_SPECIALS = {39, 97, 125, 126, 174, 195}

DOOM_THINGS_REC = 10
DOOM_LINEDEFS_REC = 14
DOOM_SIDEDEFS_REC = 30
DOOM_VERTEXES_REC = 4
DOOM_SECTORS_REC = 26
DOOM_SEGS_REC = 12
DOOM_SSECTORS_REC = 4
DOOM_NODES_REC = 28

HEXEN_THINGS_REC = 20
HEXEN_LINEDEFS_REC = 16


def _read_u32le(b: bytes, off: int) -> int:
    return struct.unpack_from("<I", b, off)[0]


def _read_i32le(b: bytes, off: int) -> int:
    return struct.unpack_from("<i", b, off)[0]


def parse_wad_directory_bytes(buf: bytes) -> Optional[Dict[str, Any]]:
    if len(buf) < 12:
        return None

    ident = buf[0:4].decode("ascii", errors="replace")
    lump_count = _read_i32le(buf, 4)
    dir_offset = _read_u32le(buf, 8)

    if ident not in ("IWAD", "PWAD"):
        return None
    if lump_count < 0 or lump_count > 200000:
        return None

    file_size = len(buf)
    dir_size = lump_count * 16
    if dir_offset + dir_size > file_size:
        return None

    directory = buf[dir_offset : dir_offset + dir_size]

    lumps: List[Dict[str, Any]] = []
    for i in range(lump_count):
        base = i * 16
        lump_off = _read_u32le(directory, base + 0)
        lump_size = _read_u32le(directory, base + 4)
        raw_name = directory[base + 8 : base + 16]
        name = raw_name.split(b"\x00", 1)[0].decode("ascii", errors="replace")
        lumps.append({"index": i, "name": name, "offset": lump_off, "size": lump_size})

    return {
        "type": ident,
        "file_size": file_size,
        "lumps": lumps,
    }


def build_map_blocks(lumps: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    names = [l["name"] for l in lumps]
    markers = [i for i, n in enumerate(names) if MAP_RE.match(n)]

    blocks: List[Dict[str, Any]] = []
    for idx, start in enumerate(markers):
        end = markers[idx + 1] if idx + 1 < len(markers) else len(lumps)
        block_lumps = lumps[start:end]
        blocks.append({
            "map": names[start],
            "start_index": start,
            "end_index_exclusive": end,
            "lumps": block_lumps,
        })
    return blocks


def find_lump(block: Dict[str, Any], name: str) -> Optional[Dict[str, Any]]:
    for l in block["lumps"]:
        if l["name"] == name:
            return l
    return None


def read_lump_bytes_from_buf(buf: bytes, lump: Dict[str, Any]) -> bytes:
    off = int(lump["offset"])
    size = int(lump["size"])
    if off < 0 or size <= 0 or off >= len(buf):
        return b""
    return buf[off : min(len(buf), off + size)]


def safe_count(size: int, rec: int) -> int:
    return size // rec if rec > 0 else 0


def detect_map_format(block: Dict[str, Any]) -> str:
    linedefs = find_lump(block, "LINEDEFS")
    things = find_lump(block, "THINGS")
    if not linedefs or not things:
        return "unknown"

    ls = linedefs["size"]
    ts = things["size"]

    doom_ok = (ls % DOOM_LINEDEFS_REC == 0) and (ts % DOOM_THINGS_REC == 0)
    hex_ok = (ls % HEXEN_LINEDEFS_REC == 0) and (ts % HEXEN_THINGS_REC == 0)

    if doom_ok and not hex_ok:
        return "doom"
    if hex_ok and not doom_ok:
        return "hexen"
    if doom_ok and hex_ok:
        if find_lump(block, "BEHAVIOR") is not None:
            return "hexen"
        return "doom"

    return "unknown"


def parse_doom_things(things_bytes: bytes) -> List[Tuple[int, int]]:
    out: List[Tuple[int, int]] = []
    if len(things_bytes) % DOOM_THINGS_REC != 0:
        return out
    for (_x, _y, _angle, ttype, flags) in struct.iter_unpack("<hhhhh", things_bytes):
        out.append((int(ttype), int(flags)))
    return out


def parse_doom_linedefs_specials(linedefs_bytes: bytes) -> List[int]:
    out: List[int] = []
    if len(linedefs_bytes) % DOOM_LINEDEFS_REC != 0:
        return out
    for (_v1, _v2, _flags, special, _tag, _right, _left) in struct.iter_unpack("<hhhhhhh", linedefs_bytes):
        out.append(int(special))
    return out


def _decode_name8(b: bytes) -> str:
    # Doom texture names are fixed 8-byte ASCII strings with NUL padding.
    # Keep behavior permissive to handle odd PWADs.
    try:
        s = b.split(b"\x00", 1)[0].decode("ascii", errors="replace")
    except Exception:
        s = ""
    return s.strip()


def parse_doom_sidedefs_texture_names(sidedefs_bytes: bytes) -> List[str]:
    out: List[str] = []
    if len(sidedefs_bytes) % DOOM_SIDEDEFS_REC != 0:
        return out
    for (_xoff, _yoff, upper, lower, middle, _sector) in struct.iter_unpack("<hh8s8s8sh", sidedefs_bytes):
        for raw in (upper, lower, middle):
            name = _decode_name8(raw)
            if not name or name == "-":
                continue
            out.append(name)
    return out


def parse_doom_sectors_texture_names(sectors_bytes: bytes) -> List[str]:
    out: List[str] = []
    if len(sectors_bytes) % DOOM_SECTORS_REC != 0:
        return out
    for (_floor_h, _ceil_h, floor_tex, ceil_tex, _light, _special, _tag) in struct.iter_unpack(
        "<hh8s8shhh",
        sectors_bytes,
    ):
        for raw in (floor_tex, ceil_tex):
            name = _decode_name8(raw)
            if not name or name == "-":
                continue
            out.append(name)
    return out


def map_summary_from_wad_bytes(buf: bytes, block: Dict[str, Any]) -> Dict[str, Any]:
    fmt = detect_map_format(block)

    def lump_count(name: str, rec_size: int) -> int:
        l = find_lump(block, name)
        return safe_count(int(l["size"]), rec_size) if l else 0

    stats: Dict[str, Any] = {
        "things": lump_count("THINGS", DOOM_THINGS_REC if fmt == "doom" else HEXEN_THINGS_REC),
        "linedefs": lump_count("LINEDEFS", DOOM_LINEDEFS_REC if fmt == "doom" else HEXEN_LINEDEFS_REC),
        "sidedefs": lump_count("SIDEDEFS", DOOM_SIDEDEFS_REC),
        "vertices": lump_count("VERTEXES", DOOM_VERTEXES_REC),
        "sectors": lump_count("SECTORS", DOOM_SECTORS_REC),
        "segs": lump_count("SEGS", DOOM_SEGS_REC),
        "ssectors": lump_count("SSECTORS", DOOM_SSECTORS_REC),
        "nodes": lump_count("NODES", DOOM_NODES_REC),
    }

    textures: set[str] = set()
    sidedefs_lump = find_lump(block, "SIDEDEFS")
    if sidedefs_lump:
        sidedefs_bytes = read_lump_bytes_from_buf(buf, sidedefs_lump)
        textures.update(parse_doom_sidedefs_texture_names(sidedefs_bytes))

    sectors_lump = find_lump(block, "SECTORS")
    if sectors_lump:
        sectors_bytes = read_lump_bytes_from_buf(buf, sectors_lump)
        textures.update(parse_doom_sectors_texture_names(sectors_bytes))

    stats["textures"] = sorted(textures)

    mechanics: Dict[str, Any] = {
        "teleports": False,
        "keys": [],
        "secret_exit": False,
    }

    monsters: Dict[str, Any] = {
        "total": 0,
        "by_type": {},
    }

    items: Dict[str, Any] = {
        "total": 0,
        "by_type": {},
    }

    difficulty: Dict[str, Any] = {
        "uv_monsters": 0,
        "hmp_monsters": 0,
        "htr_monsters": 0,
        "uv_items": 0,
        "hmp_items": 0,
        "htr_items": 0,
    }

    compatibility = "unknown"
    if fmt == "doom":
        compatibility = "vanilla_or_boom"
    elif fmt == "hexen":
        compatibility = "hexen"

    linedefs_lump = find_lump(block, "LINEDEFS")
    if linedefs_lump:
        linedefs_bytes = read_lump_bytes_from_buf(buf, linedefs_lump)

        specials: List[int] = []
        if fmt == "doom":
            specials = parse_doom_linedefs_specials(linedefs_bytes)
        else:
            if len(linedefs_bytes) % HEXEN_LINEDEFS_REC == 0:
                for rec in struct.iter_unpack("<hhhhhhhh", linedefs_bytes):
                    specials.append(int(rec[3]))

        if any(s in TELEPORT_SPECIALS for s in specials):
            mechanics["teleports"] = True
        if any(s in SECRET_EXIT_SPECIALS for s in specials):
            mechanics["secret_exit"] = True

    things_lump = find_lump(block, "THINGS")
    if things_lump and fmt == "doom":
        things_bytes = read_lump_bytes_from_buf(buf, things_lump)
        things = parse_doom_things(things_bytes)

        key_set = set()

        total_monsters = 0
        by_type: Dict[str, int] = {}

        total_items = 0
        items_by_type: Dict[str, int] = {}

        uv = 0
        hmp = 0
        htr = 0

        uv_items = 0
        hmp_items = 0
        htr_items = 0

        for ttype, flags in things:
            if ttype in KEY_THING_IDS:
                key_set.add(KEY_THING_IDS[ttype])

            mname = MONSTER_THING_IDS.get(ttype)
            if mname:
                total_monsters += 1
                by_type[mname] = by_type.get(mname, 0) + 1

                if flags & (1 << 2):
                    uv += 1
                if flags & (1 << 1):
                    hmp += 1
                if flags & (1 << 0):
                    htr += 1

            iname = ITEM_THING_IDS.get(ttype)
            if iname:
                total_items += 1
                items_by_type[iname] = items_by_type.get(iname, 0) + 1

                if flags & (1 << 2):
                    uv_items += 1
                if flags & (1 << 1):
                    hmp_items += 1
                if flags & (1 << 0):
                    htr_items += 1

        mechanics["keys"] = sorted(list(key_set))
        monsters["total"] = total_monsters
        monsters["by_type"] = dict(sorted(by_type.items(), key=lambda kv: (-kv[1], kv[0])))
        difficulty["uv_monsters"] = uv
        difficulty["hmp_monsters"] = hmp
        difficulty["htr_monsters"] = htr

        items["total"] = total_items
        items["by_type"] = dict(sorted(items_by_type.items(), key=lambda kv: (-kv[1], kv[0])))
        difficulty["uv_items"] = uv_items
        difficulty["hmp_items"] = hmp_items
        difficulty["htr_items"] = htr_items

    return {
        "map": block["map"],
        "format": fmt,
        "stats": stats,
        "monsters": monsters,
        "items": items,
        "mechanics": mechanics,
        "difficulty": difficulty,
        "compatibility": compatibility,
        "metadata": {
            "title": None,
            "music": None,
            "source": "marker",
        },
    }


def extract_per_map_stats_from_wad_bytes(buf: bytes) -> List[Dict[str, Any]]:
    wad_meta = parse_wad_directory_bytes(buf)
    if not wad_meta:
        return []
    blocks = build_map_blocks(wad_meta["lumps"])
    return [map_summary_from_wad_bytes(buf, b) for b in blocks]


# -----------------------------
# Helpers
# -----------------------------

def eprint(*args: Any, **kwargs: Any) -> None:
    print(*args, file=sys.stderr, **kwargs)


def is_http_url(s: str) -> bool:
    return isinstance(s, str) and (s.startswith("http://") or s.startswith("https://"))


def download_url_to_file(url: str, dest_path: str, *, timeout_s: float = 60.0) -> None:
    """Download url -> dest_path (atomic replace)."""
    parent = os.path.dirname(dest_path) or "."
    os.makedirs(parent, exist_ok=True)

    fd, tmp_path = tempfile.mkstemp(prefix=os.path.basename(dest_path) + ".", dir=parent)
    try:
        with os.fdopen(fd, "wb") as f:
            with requests.get(url, stream=True, timeout=timeout_s) as r:
                r.raise_for_status()
                for chunk in r.iter_content(chunk_size=1024 * 256):
                    if not chunk:
                        continue
                    f.write(chunk)
        os.replace(tmp_path, dest_path)
    except Exception:
        try:
            os.unlink(tmp_path)
        except OSError:
            pass
        raise


def read_json_file(path: str) -> Any:
    with open(path, "r", encoding="utf-8") as f:
        lines = f.readlines()
        items = [normalize_extended_json_numbers(json.loads(line)) for line in lines if line.strip()]
        return items


def normalize_extended_json_numbers(obj: Any) -> Any:
    """Convert common MongoDB Extended JSON number wrappers into plain numbers.

    Examples:
      {"$numberLong": "75964"} -> 75964
      {"$numberInt": "3"} -> 3

    This keeps behavior conservative by only converting *single-key* wrappers.
    """

    if isinstance(obj, list):
        return [normalize_extended_json_numbers(v) for v in obj]

    if not isinstance(obj, dict):
        return obj

    if len(obj) == 1:
        if "$numberLong" in obj:
            try:
                return int(obj["$numberLong"])
            except (TypeError, ValueError):
                return obj
        if "$numberInt" in obj:
            try:
                return int(obj["$numberInt"])
            except (TypeError, ValueError):
                return obj
        if "$numberDouble" in obj:
            try:
                return float(obj["$numberDouble"])
            except (TypeError, ValueError):
                return obj
        if "$numberDecimal" in obj:
            # Decimal can exceed float precision; keep as string if it doesn't parse cleanly.
            v = obj.get("$numberDecimal")
            if v is None:
                return obj
            try:
                return float(v)
            except (TypeError, ValueError):
                return obj

    return {k: normalize_extended_json_numbers(v) for k, v in obj.items()}


def safe_text_decode(b: bytes) -> str:
    # Try UTF-8 first, then latin-1 as a last-resort to keep bytes visible.
    try:
        return b.decode("utf-8")
    except UnicodeDecodeError:
        return b.decode("latin-1", errors="replace")


def normalize_whitespace(s: str) -> str:
    s = s.replace("\r\n", "\n").replace("\r", "\n")
    s = re.sub(r"[ \t]+\n", "\n", s)
    s = re.sub(r"\n{3,}", "\n\n", s)
    return s.strip()


def uniq_preserve(seq: Iterable[str]) -> List[str]:
    seen = set()
    out = []
    for x in seq:
        x = x.strip()
        if not x:
            continue
        if x not in seen:
            seen.add(x)
            out.append(x)
    return out


# -----------------------------
# WAD parsing (classic DOOM WAD)
# -----------------------------

@dataclass
class WadLump:
    name: str
    offset: int
    size: int


WAD_HEADER_RE = re.compile(rb"^(IWAD|PWAD)$")


def parse_wad_lumps(buf: bytes) -> Optional[List[WadLump]]:
    if len(buf) < 12:
        return None
    ident = buf[0:4]
    if not WAD_HEADER_RE.match(ident):
        return None
    numlumps = int.from_bytes(buf[4:8], "little", signed=False)
    infotableofs = int.from_bytes(buf[8:12], "little", signed=False)

    # sanity
    if numlumps < 0 or numlumps > 200000:
        return None
    if infotableofs < 0 or infotableofs + numlumps * 16 > len(buf):
        return None

    lumps: List[WadLump] = []
    p = infotableofs
    for _ in range(numlumps):
        filepos = int.from_bytes(buf[p:p+4], "little", signed=False)
        size = int.from_bytes(buf[p+4:p+8], "little", signed=False)
        name = buf[p+8:p+16].rstrip(b"\x00").decode("ascii", errors="replace")
        p += 16
        # bounds check
        if filepos + size > len(buf):
            # Still return partial listing; extraction will be best-effort.
            lumps.append(WadLump(name=name, offset=filepos, size=max(0, len(buf) - filepos)))
        else:
            lumps.append(WadLump(name=name, offset=filepos, size=size))
    return lumps


MAP_MARKER_RE = re.compile(r"^(E[1-9]M[1-9]|MAP[0-9]{2})$")


def detect_maps_from_lumps(lumps: List[WadLump]) -> List[str]:
    # A very standard heuristic: map marker + following core lumps exist
    core = {"THINGS", "LINEDEFS", "SIDEDEFS", "VERTEXES", "SECTORS"}
    names = [l.name.upper() for l in lumps]

    found: List[str] = []
    for i, n in enumerate(names):
        if MAP_MARKER_RE.match(n):
            # Look ahead a small window for core lumps
            window = set(names[i+1:i+1+16])
            if core.issubset(window):
                found.append(n)
    return uniq_preserve(found)


TEXT_LUMP_NAMES = {
    "MAPINFO",
    "ZMAPINFO",
    "EMAPINFO",
    "DMAPINFO",
    "UMAPINFO",
    "DEHACKED",
    "BEX",
    "SNDINFO",
    "LANGUAGE",
    "LOADACS",
    "KEYCONF",
    "ANIMDEFS",
    "DECORATE",
    "GLDEFS",
    "SBARINFO",
    "MENUDEF",
    "CVARINFO",
    "TEXTURE1",  # not really text, but sometimes contains readable stuff—skip by size heuristic
    "TEXTURE2",
}


def extract_text_lumps(buf: bytes, lumps: List[WadLump], max_each: int = 256_000) -> Dict[str, str]:
    out: Dict[str, str] = {}
    for l in lumps:
        n = l.name.upper()
        if n in TEXT_LUMP_NAMES:
            if l.size <= 0:
                continue
            if l.size > max_each:
                continue
            chunk = buf[l.offset:l.offset + l.size]
            # Skip obviously-binary blobs
            if b"\x00" in chunk[:256] and n not in {"DEHACKED", "BEX"}:
                continue
            text = normalize_whitespace(safe_text_decode(chunk))
            if text:
                out[n] = text
    return out


def guess_names_authors_descriptions_from_text(text_blobs: Dict[str, str]) -> Tuple[List[str], List[str], List[str]]:
    """
    Best-effort. We do not attempt to fully parse all formats; we just capture useful snippets.
    """
    names: List[str] = []
    authors: List[str] = []
    descs: List[str] = []

    for k, t in text_blobs.items():
        # Common: MAPINFO has "levelname" / "author"
        # We'll just regex a few common tokens.
        for m in re.finditer(r'(?i)\blevelname\s*=\s*"([^"]+)"', t):
            names.append(m.group(1).strip())
        for m in re.finditer(r'(?i)\bauthor\s*=\s*"([^"]+)"', t):
            authors.append(m.group(1).strip())
        for m in re.finditer(r'(?i)\btitle\s*=\s*"([^"]+)"', t):
            names.append(m.group(1).strip())

        # DEHACKED often includes "Patch File for DeHackEd v..." and sometimes comments; treat as a description-ish blob.
        if k in {"DEHACKED", "BEX"} and t:
            descs.append(t[:4000].strip())

        # UMAPINFO can contain "levelname" too; already covered
        # SNDINFO/DECORATE/etc not usually author/title, but can be hints.
    return (uniq_preserve(names), uniq_preserve(authors), uniq_preserve(descs))


def extract_from_wad_bytes(buf: bytes) -> Dict[str, Any]:
    lumps = parse_wad_lumps(buf)
    if not lumps:
        return {
            "format": "unknown",
            "error": "Not a classic IWAD/PWAD header (or too small/corrupt)",
        }

    maps = detect_maps_from_lumps(lumps)
    text_lumps = extract_text_lumps(buf, lumps)
    names, authors, descs = guess_names_authors_descriptions_from_text(text_lumps)

    return {
        "format": "wad",
        "lump_count": len(lumps),
        "maps": maps,
        "text_lumps": list(text_lumps.keys()),
        "names": names or None,
        "authors": authors or None,
        "descriptions": descs or None,
        # If you want the full text payloads, toggle this on.
        # "text_payloads": text_lumps,
    }


# -----------------------------
# PK3/Zip scanning
# -----------------------------

TEXTLIKE_EXTS = {
    ".txt", ".md",
    ".mapinfo", ".umapinfo",
    ".deh", ".bex",
    ".decorate", ".zs", ".zc", ".zsc",
    ".acs", ".cfg", ".ini",
    ".json", ".yaml", ".yml",
    ".pk3info",
}


def extract_from_zip_bytes(buf: bytes, max_text_files: int = 20, max_text_each: int = 200_000) -> Dict[str, Any]:
    out: Dict[str, Any] = {
        "format": "zip",
        "embedded_wads": [],
        "text_files": [],
        "names": None,
        "authors": None,
        "descriptions": None,
    }

    names: List[str] = []
    authors: List[str] = []
    descs: List[str] = []

    try:
        with zipfile.ZipFile(io.BytesIO(buf)) as z:
            # Look for embedded WADs + small textlike files
            text_collected = 0
            for info in z.infolist():
                if info.is_dir():
                    continue
                fname = info.filename
                lower = fname.lower()

                # Embedded WADs
                if lower.endswith(".wad") or lower.endswith(".iwad") or lower.endswith(".pwad"):
                    try:
                        wbuf = z.read(info)
                    except Exception:
                        continue
                    wad_meta = extract_from_wad_bytes(wbuf)
                    wad_meta["path"] = fname
                    out["embedded_wads"].append(wad_meta)
                    # Bubble up names/authors/descs
                    for v in (wad_meta.get("names") or []):
                        names.append(v)
                    for v in (wad_meta.get("authors") or []):
                        authors.append(v)
                    for v in (wad_meta.get("descriptions") or []):
                        descs.append(v)
                    continue

                # Textlike
                if text_collected < max_text_files and any(lower.endswith(ext) for ext in TEXTLIKE_EXTS):
                    if info.file_size <= 0 or info.file_size > max_text_each:
                        continue
                    try:
                        tbuf = z.read(info)
                    except Exception:
                        continue
                    # Skip obviously binary
                    if b"\x00" in tbuf[:256]:
                        continue
                    text = normalize_whitespace(safe_text_decode(tbuf))
                    if not text:
                        continue
                    out["text_files"].append({
                        "path": fname,
                        "size": info.file_size,
                        "contents": text,
                    })
                    # Heuristic: treat top-level readme-ish files as descriptions
                    base = os.path.basename(fname).lower()
                    if base in {"readme.txt", "readme.md", "info.txt", "description.txt"} or base.endswith(".txt"):
                        descs.append(text[:8000])
                    text_collected += 1

    except zipfile.BadZipFile:
        out["format"] = "unknown"
        out["error"] = "Not a valid zip/PK3 container"
        return out

    out["names"] = uniq_preserve(names) or None
    out["authors"] = uniq_preserve(authors) or None
    out["descriptions"] = uniq_preserve(descs) or None
    return out


def _score_embedded_wad_candidate(path_in_zip: str, wad_meta: Dict[str, Any], size: int) -> int:
    """
    Heuristic to pick the "primary" WAD inside a PK3-like zip.

    Goals:
    - Prefer WADs under maps/ (common convention)
    - Prefer WADs that actually contain more maps
    - Prefer larger WADs as a weak tie-break
    """
    p = (path_in_zip or "").replace("\\", "/")
    lower = p.lower()

    score = 0
    if lower.startswith("maps/") or "/maps/" in lower:
        score += 10_000

    maps = wad_meta.get("maps")
    if isinstance(maps, list):
        score += min(len(maps), 200) * 100

    # Tie-breaks: classic WAD structure indicators
    lump_count = wad_meta.get("lump_count")
    if isinstance(lump_count, int):
        score += min(lump_count, 50_000) // 10

    # Size: 1 point per 64KiB
    if isinstance(size, int) and size > 0:
        score += size // 65_536

    return int(score)


def find_primary_wad_in_zip_path(zip_path: str) -> Optional[Tuple[str, bytes]]:
    """Return (path_in_zip, wad_bytes) for the best embedded WAD, else None."""
    try:
        with zipfile.ZipFile(zip_path) as z:
            best: Optional[Tuple[int, str, bytes]] = None

            for info in z.infolist():
                if info.is_dir():
                    continue

                fname = info.filename
                lower = fname.lower()
                if not (lower.endswith(".wad") or lower.endswith(".iwad") or lower.endswith(".pwad")):
                    continue

                try:
                    wbuf = z.read(info)
                except Exception:
                    continue

                wad_meta = extract_from_wad_bytes(wbuf)
                if wad_meta.get("format") != "wad":
                    continue

                score = _score_embedded_wad_candidate(fname, wad_meta, int(getattr(info, "file_size", 0) or 0))
                cand = (score, fname, wbuf)
                if best is None or cand[0] > best[0]:
                    best = cand

            if best is None:
                return None
            return (best[1], best[2])

    except zipfile.BadZipFile:
        return None


def find_all_wads_in_zip_path(zip_path: str) -> List[Tuple[str, bytes]]:
    """Return [(path_in_zip, wad_bytes), ...] in zip/infolist order."""
    out: List[Tuple[str, bytes]] = []
    try:
        with zipfile.ZipFile(zip_path) as z:
            for info in z.infolist():
                if info.is_dir():
                    continue
                fname = info.filename
                lower = fname.lower()
                if not (lower.endswith(".wad") or lower.endswith(".iwad") or lower.endswith(".pwad")):
                    continue
                try:
                    wbuf = z.read(info)
                except Exception:
                    continue
                wad_meta = extract_from_wad_bytes(wbuf)
                if wad_meta.get("format") != "wad":
                    continue
                out.append((fname, wbuf))
    except zipfile.BadZipFile:
        return []
    return out


def merge_per_map_stats(map_lists_in_load_order: List[List[Dict[str, Any]]]) -> List[Dict[str, Any]]:
    """Merge per-map stats with 'last loaded wins' semantics.

    If a map name appears multiple times across WADs, the later one replaces the earlier.
    Output ordering follows the *last* occurrence (i.e., overridden maps move later).
    """
    by_name: Dict[str, Dict[str, Any]] = {}
    order: List[str] = []

    for maps in map_lists_in_load_order:
        if not isinstance(maps, list):
            continue
        for m in maps:
            if not isinstance(m, dict):
                continue
            name = m.get("map")
            if not isinstance(name, str) or not name:
                continue
            # If this map was defined before, move it to the end to reflect overwrite.
            if name in by_name:
                try:
                    order.remove(name)
                except ValueError:
                    pass
            by_name[name] = m
            order.append(name)

    return [by_name[n] for n in order if n in by_name]


# -----------------------------
# S3 resolution / downloading
# -----------------------------

TYPE_TO_EXT = {
    # Classic
    "IWAD": "wad",
    "PWAD": "wad",
    "ZWAD": "wad",
    # Quake / etc
    "WAD2": "wad2",
    "WAD3": "wad3",
    # Archives
    "PK3": "pk3",
    "PK7": "pk7",
    "PKZ": "pkz",
    "EPK": "epk",
    "PKE": "pke",
    "UNKNOWN": None,
}


def candidate_prefixes(wad_entry: Dict[str, Any]) -> List[str]:
    """
    The example shows:
      dir = sha1
      filename = <??><sha1>.<ext>.gz
    where ?? was "02" while sha1 started with "00".
    We don't know the rule, so we probe a small set.
    """
    sha1 = (wad_entry.get("_id") or "").lower()
    hashes = wad_entry.get("hashes") or {}
    md5 = (hashes.get("md5") or "").lower()
    sha256 = (hashes.get("sha256") or "").lower()

    cands = []
    for h in [sha1, md5, sha256]:
        if len(h) >= 2:
            cands.append(h[:2])

    # Also probe all shards quickly if requested? That'd be 256 HEADs per file = expensive.
    # Instead, add a few common-sense fallbacks:
    cands += ["00", "01", "02", "03", "ff"]

    # De-dupe preserve
    seen = set()
    out = []
    for p in cands:
        p = p.lower()
        if re.fullmatch(r"[0-9a-f]{2}", p) and p not in seen:
            seen.add(p)
            out.append(p)
    return out


def resolve_s3_url(
    session: requests.Session,
    s3_base: str,
    sha1: str,
    ext: str,
    prefixes: List[str],
    timeout: Tuple[int, int] = (5, 15),
) -> Optional[str]:
    """
    Try HEAD on a handful of candidates:
      {base}/{sha1}/{prefix}{sha1}.{ext}.gz
    """
    # NOTE: Prefix guessing is kept for backwards compatibility with earlier layouts,
    # but the current bucket layout appears to be:
    #   {base}/{sha1-with-leading-00-stripped}/{sha1}.{ext}.gz
    # We keep the 'prefixes' argument in the signature since it is part of the
    # original design, but we currently probe only this one candidate.
    folder_sha1 = sha1.removeprefix("00")
    if len(sha1) != 40:
        return None
    url = f"{s3_base.rstrip('/')}/{folder_sha1}/{sha1}.{ext}.gz"
    try:
        r = session.head(url, timeout=timeout, allow_redirects=True)
        if r.status_code == 200:
            return url
        elif r.status_code in [404, 403]:
            return None
    except requests.RequestException as e:
        return None


def download_to_path(session: requests.Session, url: str, out_path: str) -> None:
    with session.get(url, stream=True, timeout=(10, 60)) as r:
        r.raise_for_status()
        with open(out_path, "wb") as f:
            for chunk in r.iter_content(chunk_size=1024 * 256):
                if chunk:
                    f.write(chunk)


def gunzip_file(src_gz: str, dst_path: str) -> None:
    with gzip.open(src_gz, "rb") as gz:
        with open(dst_path, "wb") as out:
            shutil_copyfileobj(gz, out)


def shutil_copyfileobj(src, dst, length: int = 1024 * 1024) -> None:
    while True:
        buf = src.read(length)
        if not buf:
            return
        dst.write(buf)


# -----------------------------
# Merge logic (precedence rules)
# -----------------------------

def pick_first(*vals: Optional[str]) -> Optional[str]:
    for v in vals:
        if v is None:
            continue
        v = v.strip()
        if v:
            return v
    return None


def merge_lists(*vals: Optional[List[str]]) -> Optional[List[str]]:
    items: List[str] = []
    for v in vals:
        if not v:
            continue
        items.extend(v)
    u = uniq_preserve(items)
    return u or None


def build_output_object(
    sha1: str,
    sha256: Optional[str],
    s3_url: Optional[str],
    extracted: Dict[str, Any],
    wad_archive: Dict[str, Any],
    idgames: Optional[Dict[str, Any]],
    integrity: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    def _compact_extracted(ex: Dict[str, Any]) -> Dict[str, Any]:
        """Avoid embedding large blobs (e.g. text file contents) in sources.extracted."""
        if not isinstance(ex, dict):
            return {}
        if ex.get("format") != "zip":
            return ex
        tfs = ex.get("text_files")
        if not isinstance(tfs, list) or not tfs:
            return ex
        ex2 = dict(ex)
        compact: List[Dict[str, Any]] = []
        for tf in tfs:
            if not isinstance(tf, dict):
                continue
            item: Dict[str, Any] = {}
            if "path" in tf:
                item["path"] = tf.get("path")
            if "size" in tf:
                item["size"] = tf.get("size")
            if item:
                compact.append(item)
        ex2["text_files"] = compact
        return ex2

    def _build_meta_text_files(ex: Dict[str, Any], ig_textfile_val: Optional[str]) -> Optional[List[Dict[str, Any]]]:
        out_files: List[Dict[str, Any]] = []

        # PK3-like (zip) embedded text
        if isinstance(ex, dict) and ex.get("format") == "zip":
            tfs = ex.get("text_files")
            if isinstance(tfs, list):
                for tf in tfs:
                    if not isinstance(tf, dict):
                        continue
                    name = tf.get("path")
                    contents = tf.get("contents")
                    if not isinstance(contents, str) or not contents.strip():
                        continue
                    if not isinstance(name, str) or not name.strip():
                        # Best effort; still include contents.
                        out_files.append({"source": "pk3", "contents": contents})
                    else:
                        out_files.append({"source": "pk3", "name": name, "contents": contents})

        # idgames TXT (stored in idgames.json as latin-1-ish strings)
        if isinstance(ig_textfile_val, str) and ig_textfile_val.strip():
            ig_text = normalize_whitespace(
                safe_text_decode(ig_textfile_val.encode("latin-1", errors="replace"))
            )
            if ig_text:
                out_files.append({"source": "idgames", "contents": ig_text})

        return out_files or None

    # WAD Archive fields
    wa_type = wad_archive.get("type")
    wa_counts = wad_archive.get("counts")
    wa_maps = wad_archive.get("maps")
    wa_engines = wad_archive.get("engines")
    wa_iwads = wad_archive.get("iwads")
    wa_descs = wad_archive.get("descriptions")
    wa_authors = wad_archive.get("authors")
    wa_names = wad_archive.get("names")
    wa_updated = wad_archive.get("updated")
    wa_corrupt = wad_archive.get("corrupt")
    wa_corrupt_msg = wad_archive.get("corruptMessage")
    wa_hashes = wad_archive.get("hashes") or {}

    # idGames fields (if present)
    ig = (idgames or {}).get("content") if isinstance(idgames, dict) else None
    ig_title = ig.get("title") if isinstance(ig, dict) else None
    ig_author = ig.get("author") if isinstance(ig, dict) else None
    ig_date = ig.get("date") if isinstance(ig, dict) else None
    ig_dir = ig.get("dir") if isinstance(ig, dict) else None
    ig_filename = ig.get("filename") if isinstance(ig, dict) else None
    ig_desc = ig.get("description") if isinstance(ig, dict) else None
    ig_credits = ig.get("credits") if isinstance(ig, dict) else None
    ig_textfile = ig.get("textfile") if isinstance(ig, dict) else None
    ig_url = ig.get("url") if isinstance(ig, dict) else None
    ig_id = ig.get("id") if isinstance(ig, dict) else None
    ig_rating = ig.get("rating") if isinstance(ig, dict) else None
    ig_votes = ig.get("votes") if isinstance(ig, dict) else None

    ex_names = extracted.get("names")
    ex_authors = extracted.get("authors")
    ex_descs = extracted.get("descriptions")

    # Coherent, non-redundant top-level picks (precedence: extracted > wads.json > idgames)
    title = pick_first(
        (ex_names or [None])[0] if isinstance(ex_names, list) and ex_names else None,
        (wa_names or [None])[0] if isinstance(wa_names, list) and wa_names else None,
        ig_title,
    )

    authors = merge_lists(
        ex_authors,
        wa_authors,
        [ig_author] if ig_author else None,
    )

    # Description: prefer extracted; else wads.json; else idgames.description; also keep credits/textfile separately
    descriptions = merge_lists(
        ex_descs,
        wa_descs,
        [normalize_whitespace(safe_text_decode(ig_desc.encode("latin-1", errors="replace")))] if isinstance(ig_desc, str) else None,
    )

    # Maps: prefer extracted maps, else WAD Archive maps
    ex_maps = extracted.get("maps") if isinstance(extracted.get("maps"), list) else None
    maps = ex_maps or wa_maps

    out: Dict[str, Any] = {
        "sha1": sha1,
        "sha256": sha256,
        "title": title,
        "authors": authors,
        "descriptions": descriptions,
        "text_files": _build_meta_text_files(extracted, ig_textfile),
        "file": {
            "type": wa_type,
            "size": wad_archive.get("size"),
            "url": s3_url,
            "corrupt": wa_corrupt,
            "corruptMessage": wa_corrupt_msg,
        },
        "content": {
            "maps": maps,
            "counts": wa_counts,
            "engines_guess": wa_engines,
            "iwads_guess": wa_iwads,
        },
        "sources": {
            "wad_archive": {
                "updated": wa_updated,
                # Keep hashes here to avoid redundancy at top-level
                "hashes": wad_archive.get("hashes"),
            },
            "idgames": None,
            "extracted": _compact_extracted(extracted),
        },
    }

    if ig is not None:
        out["sources"]["idgames"] = {
            "id": ig_id,
            "url": ig_url,
            "dir": ig_dir,
            "filename": ig_filename,
            "date": ig_date,
            "title": ig_title,
            "author": ig_author,
            "credits": ig_credits,
            "textfile": ig_textfile,
            "rating": ig_rating,
            "votes": ig_votes,
        }

    if integrity is not None:
        ok = integrity.get("ok")
        msg = integrity.get("message")
        if ok is False:
            out.setdefault("file", {})
            out["file"]["corrupt"] = True
            out["file"]["corruptMessage"] = msg or "Failed integrity checks"

    # Prune nulls for cleanliness
    return prune_nones(out)


def compute_hashes_for_file(path: str) -> Dict[str, str]:
    md5 = hashlib.md5()
    sha1 = hashlib.sha1()
    sha256 = hashlib.sha256()

    with open(path, "rb") as f:
        while True:
            chunk = f.read(1024 * 1024)
            if not chunk:
                break
            md5.update(chunk)
            sha1.update(chunk)
            sha256.update(chunk)

    return {
        "md5": md5.hexdigest(),
        "sha1": sha1.hexdigest(),
        "sha256": sha256.hexdigest(),
    }


def validate_expected_hashes(expected: Dict[str, Any], computed: Dict[str, str]) -> Dict[str, Any]:
    """Return {ok: bool, message: str}.

    Only validates hashes that are present in expected. Missing expected hashes are ignored.
    """
    mismatches: List[str] = []

    for algo in ("md5", "sha1", "sha256"):
        exp = expected.get(algo)
        if not isinstance(exp, str) or not exp.strip():
            continue
        exp = exp.strip().lower()
        got = computed.get(algo)
        if got is None:
            continue
        if exp != got.lower():
            mismatches.append(f"{algo} expected={exp} got={got}")

    if mismatches:
        return {
            "ok": False,
            "message": "Integrity check failed: " + "; ".join(mismatches),
        }

    return {"ok": True, "message": "ok"}


def prune_nones(obj: Any) -> Any:
    if isinstance(obj, dict):
        out = {}
        for k, v in obj.items():
            pv = prune_nones(v)
            if pv is None:
                continue
            if isinstance(pv, (dict, list)) and len(pv) == 0:
                continue
            out[k] = pv
        return out
    if isinstance(obj, list):
        out = [prune_nones(v) for v in obj]
        out = [v for v in out if v is not None and not (isinstance(v, (dict, list)) and len(v) == 0)]
        return out
    return obj


# -----------------------------
# Main
# -----------------------------

def build_idgames_lookup(
    idgames_entries: List[Dict[str, Any]],
    wad_sha1s: set[str],
) -> Dict[str, Dict[str, Any]]:
    """
    Build sha1 -> idgames entry.
    - Ignore idgames entries that don't link back to wads.json
    - If multiple idgames entries link to same sha1, keep the first one encountered
      (fast default). You can customize tie-breaks later (e.g., newest date).
    """
    lookup: Dict[str, Dict[str, Any]] = {}
    for entry in idgames_entries:
        hashes = entry.get("hashes") or []
        if not isinstance(hashes, list) or not hashes:
            continue
        linked = [h.lower() for h in hashes if isinstance(h, str) and h.lower() in wad_sha1s]
        if not linked:
            continue
        for h in linked:
            lookup.setdefault(h, entry)
    return lookup


def extract_metadata_from_file(path: str, ext: str) -> Dict[str, Any]:
    """
    ext is the *decompressed* file extension (wad/pk3/etc).
    """
    with open(path, "rb") as f:
        buf = f.read()

    # WAD
    wad_meta = extract_from_wad_bytes(buf)
    if wad_meta.get("format") == "wad":
        return wad_meta

    # PK3 etc (zip containers)
    if ext in {"pk3", "pk7", "pkz", "epk", "pke"}:
        return extract_from_zip_bytes(buf)

    # Unknown / other
    return {
        "format": "unknown",
        "note": f"Unhandled extension '{ext}'",
        "size": len(buf),
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--wads-json", required=True, help="Path or URL to wads.json")
    ap.add_argument("--idgames-json", required=True, help="Path or URL to idgames.json")
    ap.add_argument("--s3-base", default=DEFAULT_S3_BASE, help="Base URL for public S3 bucket")
    ap.add_argument("--limit", type=int, default=0, help="Process only N wads (0 = all)")
    ap.add_argument("--start", type=int, default=0, help="Start index into wads.json array")
    ap.add_argument("--pretty", action="store_true", help="Pretty-print JSON")
    ap.add_argument("--stream", action="store_true", help="Emit newline-delimited JSON objects (NDJSON)")
    ap.add_argument("--sleep", type=float, default=0.0, help="Sleep seconds between items (politeness)")
    args = ap.parse_args()

    if is_http_url(args.wads_json):
        eprint(f"Downloading wads.json: {args.wads_json} -> /tmp/wads.json")
        try:
            download_url_to_file(args.wads_json, "/tmp/wads.json")
        except Exception as ex:
            raise SystemExit(f"Failed to download --wads-json: {ex}")
        args.wads_json = "/tmp/wads.json"

    if is_http_url(args.idgames_json):
        eprint(f"Downloading idgames.json: {args.idgames_json} -> /tmp/idgames.json")
        try:
            download_url_to_file(args.idgames_json, "/tmp/idgames.json")
        except Exception as ex:
            raise SystemExit(f"Failed to download --idgames-json: {ex}")
        args.idgames_json = "/tmp/idgames.json"

    wads_data = read_json_file(args.wads_json)
    idgames_data = read_json_file(args.idgames_json)

    if not isinstance(wads_data, list):
        raise SystemExit("wads.json must be a JSON array of WAD entries")
    if not isinstance(idgames_data, list):
        raise SystemExit("idgames.json must be a JSON array of idGames entries")

    wad_sha1s = {str(w.get("_id", "")).lower() for w in wads_data if isinstance(w, dict) and w.get("_id")}
    id_lookup = build_idgames_lookup(idgames_data, wad_sha1s)

    session = requests.Session()

    total = len(wads_data)
    start = max(0, args.start)
    end = total if args.limit <= 0 else min(total, start + args.limit)

    out_items: Optional[List[Dict[str, Any]]] = [] if (args.pretty and not args.stream) else None
    first_array_item = True
    if not args.stream and out_items is None:
        sys.stdout.write("[")

    for idx in range(start, end):
        wad_entry = wads_data[idx]
        if not isinstance(wad_entry, dict):
            continue

        sha1 = str(wad_entry.get("_id") or "").lower()
        if not sha1 or not re.fullmatch(r"[0-9a-f]{40}", sha1):
            continue

        expected_hashes = wad_entry.get("hashes") or {}
        expected_sha256 = None
        if isinstance(expected_hashes, dict):
            v = expected_hashes.get("sha256")
            if isinstance(v, str) and v.strip():
                expected_sha256 = v.strip().lower()
        smoke_test_id = "0000e0b4993f0b7130fc3b58abf996bbb4acb287"
        if not re.fullmatch(r"[0-9a-f]{40}", sha1):
            raise ValueError("SHA1 must be 40 hex chars")
        if smoke_test_id is not None and smoke_test_id not in sha1:
            #print(f"Skipping {sha1}: not the test file", file=sys.stderr)
            continue # TEMP: process only one known-good file for testing
        print(f"Processing {smoke_test_id}: {idx + 1}/{total}", file=sys.stderr)
        wad_type = str(wad_entry.get("type") or "UNKNOWN")
        ext = TYPE_TO_EXT.get(wad_type, None) or "wad"  # default best-guess

        prefixes = candidate_prefixes(wad_entry)
        s3_url = resolve_s3_url(session, args.s3_base, sha1, ext, prefixes)

        extracted: Dict[str, Any]
        per_map_stats: List[Dict[str, Any]] = []
        computed_hashes: Optional[Dict[str, str]] = None
        integrity: Optional[Dict[str, Any]] = None
        if not s3_url:
            extracted = {
                "format": "unknown",
                "error": "Could not resolve S3 object URL (prefix mismatch).",
                "tried_prefixes": prefixes,
                "expected_ext": ext,
            }
            meta_obj = build_output_object(
                sha1=sha1,
                sha256=expected_sha256,
                s3_url=None,
                extracted=extracted,
                wad_archive=wad_entry,
                idgames=id_lookup.get(sha1),
                integrity=None,
            )

            out_obj = {"meta": meta_obj, "maps": per_map_stats}

            if args.stream:
                sys.stdout.write(json.dumps(out_obj, indent=2 if args.pretty else None, ensure_ascii=False))
                sys.stdout.write("\n")
            else:
                if out_items is not None:
                    out_items.append(out_obj)
                else:
                    if not first_array_item:
                        sys.stdout.write(",")
                    sys.stdout.write("\n" if not first_array_item else "\n")
                    sys.stdout.write(json.dumps(out_obj, ensure_ascii=False))
                    first_array_item = False

            if args.sleep > 0:
                time.sleep(args.sleep)
            continue

        with tempfile.TemporaryDirectory(prefix="wadmerge_") as td:
            gz_path = os.path.join(td, f"{sha1}.{ext}.gz")
            file_path = os.path.join(td, f"{sha1}.{ext}")

            try:
                download_to_path(session, s3_url, gz_path)

                # Decompress to actual file
                with gzip.open(gz_path, "rb") as gz:
                    with open(file_path, "wb") as out_f:
                        shutil_copyfileobj(gz, out_f)

                computed_hashes = compute_hashes_for_file(file_path)
                if isinstance(expected_hashes, dict):
                    integrity = validate_expected_hashes(expected_hashes, computed_hashes)
                else:
                    integrity = None

                extracted = extract_metadata_from_file(file_path, ext)

                # Per-map stats:
                # - For WADs, run directly
                # - For PK3-like zips, analyze all embedded WADs in load order and merge maps
                if ext == "wad":
                    with open(file_path, "rb") as f:
                        per_map_stats = extract_per_map_stats_from_wad_bytes(f.read())
                elif ext in {"pk3", "pk7", "pkz", "epk", "pke"}:
                    embedded = find_all_wads_in_zip_path(file_path)
                    map_lists: List[List[Dict[str, Any]]] = []
                    for (_wad_path, wbuf) in embedded:
                        map_lists.append(extract_per_map_stats_from_wad_bytes(wbuf))
                    per_map_stats = merge_per_map_stats(map_lists)

            except Exception as ex:
                extracted = {
                    "format": "unknown",
                    "error": f"Download/decompress/extract failed: {type(ex).__name__}: {ex}",
                }
                per_map_stats = []
                computed_hashes = None
                integrity = None

            meta_obj = build_output_object(
                sha1=sha1,
                sha256=(computed_hashes or {}).get("sha256") or expected_sha256,
                s3_url=s3_url,
                extracted=extracted,
                wad_archive=wad_entry,
                idgames=id_lookup.get(sha1),
                integrity=integrity,
            )

            out_obj = {"meta": meta_obj, "maps": per_map_stats}

            if args.stream:
                sys.stdout.write(json.dumps(out_obj, indent=2 if args.pretty else None, ensure_ascii=False))
                sys.stdout.write("\n")
            else:
                if out_items is not None:
                    out_items.append(out_obj)
                else:
                    if not first_array_item:
                        sys.stdout.write(",")
                    sys.stdout.write("\n" if not first_array_item else "\n")
                    sys.stdout.write(json.dumps(out_obj, ensure_ascii=False))
                    first_array_item = False

        # Temp directory auto-deletes here

        if args.sleep > 0:
            time.sleep(args.sleep)

    if not args.stream:
        if out_items is not None:
            sys.stdout.write(json.dumps(out_items, indent=2, ensure_ascii=False))
            sys.stdout.write("\n")
        else:
            sys.stdout.write("\n\n]")
            sys.stdout.write("\n")


if __name__ == "__main__":
    main()
