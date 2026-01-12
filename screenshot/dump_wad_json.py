#!/usr/bin/env python3
import argparse
import json
import os
import re
import struct
from typing import Dict, Any, List, Optional, Tuple

MAP_RE = re.compile(r"^(MAP\d\d|E\dM\d)$")

# Key thing IDs (vanilla Doom/Doom II) :contentReference[oaicite:2]{index=2}
KEY_THING_IDS = {
    5: "blue",
    6: "yellow",
    13: "red",
    38: "red_skull",
    39: "yellow_skull",
    40: "blue_skull",
}

# Common monster thing IDs (vanilla + Doom II)
# (IDs shown in the classic thing tables/specs) :contentReference[oaicite:3]{index=3}
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
    7: "spider_mastermind",   # commonly 7 in editor tables
    64: "archvile",
    66: "revenant",
    67: "mancubus",
    68: "arachnotron",
    69: "hell_knight",
    71: "pain_elemental",
    3003: "baron",
}

# Linedef specials for exits/teleports
# Secret exit types (51/124/198) :contentReference[oaicite:4]{index=4}
SECRET_EXIT_SPECIALS = {51, 124, 198}

# Teleport examples commonly used in Doom/Boom maps (39, 97, 125, 126, etc.) :contentReference[oaicite:5]{index=5}
TELEPORT_SPECIALS = {39, 97, 125, 126, 174, 195}

# Record sizes (Doom format)
DOOM_THINGS_REC = 10
DOOM_LINEDEFS_REC = 14
DOOM_SIDEDEFS_REC = 30
DOOM_VERTEXES_REC = 4
DOOM_SECTORS_REC = 26
DOOM_SEGS_REC = 12
DOOM_SSECTORS_REC = 4
DOOM_NODES_REC = 28

# Record sizes (Hexen format map)
HEXEN_THINGS_REC = 20
HEXEN_LINEDEFS_REC = 16


def read_u32le(b: bytes, off: int) -> int:
    return struct.unpack_from("<I", b, off)[0]


def read_i32le(b: bytes, off: int) -> int:
    return struct.unpack_from("<i", b, off)[0]


def parse_wad_directory(path: str) -> Dict[str, Any]:
    with open(path, "rb") as f:
        header = f.read(12)
        if len(header) != 12:
            raise ValueError("File too small to be a WAD")

        ident = header[0:4].decode("ascii", errors="replace")
        lump_count = read_i32le(header, 4)
        dir_offset = read_u32le(header, 8)

        if ident not in ("IWAD", "PWAD"):
            raise ValueError(f"Not a WAD (signature={ident!r})")

        f.seek(0, os.SEEK_END)
        file_size = f.tell()

        dir_size = lump_count * 16
        if dir_offset + dir_size > file_size:
            raise ValueError("Directory extends past EOF")

        f.seek(dir_offset)
        directory = f.read(dir_size)

    lumps = []
    for i in range(lump_count):
        base = i * 16
        lump_off = read_u32le(directory, base + 0)
        lump_size = read_u32le(directory, base + 4)
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

    blocks = []
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
    # Within a map block, lump names are unique in normal maps; pick first match
    for l in block["lumps"]:
        if l["name"] == name:
            return l
    return None


def read_lump_bytes(wad_path: str, lump: Dict[str, Any]) -> bytes:
    with open(wad_path, "rb") as f:
        f.seek(lump["offset"])
        return f.read(lump["size"])


def safe_count(size: int, rec: int) -> int:
    return size // rec if rec > 0 else 0


def detect_map_format(block: Dict[str, Any]) -> str:
    linedefs = find_lump(block, "LINEDEFS")
    things = find_lump(block, "THINGS")
    if not linedefs or not things:
        return "unknown"

    ls = linedefs["size"]
    ts = things["size"]

    # Heuristic: Doom linedefs are 14 bytes; Hexen are 16 bytes
    doom_ok = (ls % DOOM_LINEDEFS_REC == 0) and (ts % DOOM_THINGS_REC == 0)
    hex_ok  = (ls % HEXEN_LINEDEFS_REC == 0) and (ts % HEXEN_THINGS_REC == 0)

    if doom_ok and not hex_ok:
        return "doom"
    if hex_ok and not doom_ok:
        return "hexen"
    if doom_ok and hex_ok:
        # ambiguous; prefer Doom unless BEHAVIOR/ACS etc indicates Hexen-ish maps
        if find_lump(block, "BEHAVIOR") is not None:
            return "hexen"
        return "doom"

    return "unknown"


def parse_doom_things(things_bytes: bytes) -> List[Tuple[int, int]]:
    """
    Returns list of (thing_type, flags) for Doom-format THINGS.
    THINGS record is 5 * int16: x, y, angle, type, flags
    Flags bits: 0 skill1-2, 1 skill3, 2 skill4-5, 3 ambush, 4 multiplayer-only :contentReference[oaicite:6]{index=6}
    """
    out = []
    if len(things_bytes) % DOOM_THINGS_REC != 0:
        return out

    for (x, y, angle, ttype, flags) in struct.iter_unpack("<hhhhh", things_bytes):
        out.append((ttype, flags))
    return out


def parse_doom_linedefs_specials(linedefs_bytes: bytes) -> List[int]:
    """
    Doom linedef record: v1, v2, flags, special, tag, right, left (7 * int16)
    """
    out = []
    if len(linedefs_bytes) % DOOM_LINEDEFS_REC != 0:
        return out
    for (v1, v2, flags, special, tag, right, left) in struct.iter_unpack("<hhhhhhh", linedefs_bytes):
        out.append(int(special))
    return out


def map_summary(wad_path: str, wad_meta: Dict[str, Any], block: Dict[str, Any]) -> Dict[str, Any]:
    fmt = detect_map_format(block)

    # Core lump sizes → counts
    def lump_count(name: str, rec_size: int) -> int:
        l = find_lump(block, name)
        return safe_count(l["size"], rec_size) if l else 0

    stats = {
        "things": lump_count("THINGS", DOOM_THINGS_REC if fmt == "doom" else HEXEN_THINGS_REC),
        "linedefs": lump_count("LINEDEFS", DOOM_LINEDEFS_REC if fmt == "doom" else HEXEN_LINEDEFS_REC),
        "sidedefs": lump_count("SIDEDEFS", DOOM_SIDEDEFS_REC),
        "vertices": lump_count("VERTEXES", DOOM_VERTEXES_REC),
        "sectors": lump_count("SECTORS", DOOM_SECTORS_REC),
        "segs": lump_count("SEGS", DOOM_SEGS_REC),
        "ssectors": lump_count("SSECTORS", DOOM_SSECTORS_REC),
        "nodes": lump_count("NODES", DOOM_NODES_REC),
    }

    mechanics = {
        "teleports": False,
        "keys": [],
        "secret_exit": False,
    }

    monsters = {
        "total": 0,
        "by_type": {},
    }

    difficulty = {
        "uv_monsters": 0,   # skill 4-5 bucket
        "hmp_monsters": 0,  # skill 3 bucket
        "htr_monsters": 0,  # skill 1-2 bucket
    }

    compatibility = "unknown"
    if fmt == "doom":
        compatibility = "vanilla_or_boom"
    elif fmt == "hexen":
        compatibility = "hexen"

    # Parse mechanics + monsters where we can
    linedefs_lump = find_lump(block, "LINEDEFS")
    if linedefs_lump:
        linedefs_bytes = read_lump_bytes(wad_path, linedefs_lump)

        specials: List[int] = []
        if fmt == "doom":
            specials = parse_doom_linedefs_specials(linedefs_bytes)
        else:
            # For hexen format, "specials" are still present but record layout differs.
            # We'll do a lightweight heuristic: take the "special" as the 3rd int16 in a 16-byte record (after v1,v2,flags).
            if len(linedefs_bytes) % HEXEN_LINEDEFS_REC == 0:
                for rec in struct.iter_unpack("<hhhhhhhh", linedefs_bytes):
                    # v1,v2,flags,special, arg1..arg5
                    specials.append(int(rec[3]))

        if any(s in TELEPORT_SPECIALS for s in specials):
            mechanics["teleports"] = True
        if any(s in SECRET_EXIT_SPECIALS for s in specials):
            mechanics["secret_exit"] = True

    things_lump = find_lump(block, "THINGS")
    if things_lump and fmt == "doom":
        things_bytes = read_lump_bytes(wad_path, things_lump)
        things = parse_doom_things(things_bytes)

        key_set = set()

        # Monster totals + difficulty buckets
        total_monsters = 0
        by_type: Dict[str, int] = {}

        uv = 0
        hmp = 0
        htr = 0

        for ttype, flags in things:
            # keys
            if ttype in KEY_THING_IDS:
                key_set.add(KEY_THING_IDS[ttype])

            # monsters
            mname = MONSTER_THING_IDS.get(ttype)
            if mname:
                total_monsters += 1
                by_type[mname] = by_type.get(mname, 0) + 1

                # skill flags bits per spec :contentReference[oaicite:7]{index=7}
                if flags & (1 << 2):  # skill 4-5 (UV/NM)
                    uv += 1
                if flags & (1 << 1):  # skill 3 (HMP)
                    hmp += 1
                if flags & (1 << 0):  # skill 1-2 (ITYTD/HNTR)
                    htr += 1

        mechanics["keys"] = sorted(list(key_set))

        monsters["total"] = total_monsters
        monsters["by_type"] = dict(sorted(by_type.items(), key=lambda kv: (-kv[1], kv[0])))

        difficulty["uv_monsters"] = uv
        difficulty["hmp_monsters"] = hmp
        difficulty["htr_monsters"] = htr
    elif things_lump and fmt != "doom":
        # Still extract keys for hexen if you want; key thing IDs may differ in non-vanilla sets,
        # so we leave keys empty and monster details empty to avoid lying.
        pass

    return {
        "map": block["map"],
        "format": fmt,
        "stats": stats,
        "monsters": monsters,
        "mechanics": mechanics,
        "difficulty": difficulty,
        "compatibility": compatibility,
        "metadata": {
            "title": None,     # WADs often don’t store display titles unless MAPINFO/UMAPINFO exists
            "music": None,
            "source": "marker",
        },
    }

def run(wad_path: str, out_path: Optional[str] = None):
    wad_meta = parse_wad_directory(wad_path)
    blocks = build_map_blocks(wad_meta["lumps"])

    maps = [map_summary(wad_path, wad_meta, b) for b in blocks]

    out_obj = {
        "file": os.path.abspath(wad_path),
        "file_size": wad_meta["file_size"],
        "type": wad_meta["type"],
        "maps": maps,
    }

    data = json.dumps(out_obj, indent=2)

    if out_path:
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(data)
    else:
        print(data)

def _main():
    ap = argparse.ArgumentParser(description="Extract per-map JSON summaries from a WAD")
    ap.add_argument("wad_path")
    ap.add_argument("-o", "--out", help="Output JSON path (default: stdout)")
    args = ap.parse_args()
    run(args.wad_path, args.out)

if __name__ == "__main__":
    _main()
