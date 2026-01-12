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
import io
import json
import os
import re
import sys
import tempfile
import time
import zipfile
from dataclasses import dataclass
from typing import Any, Dict, Iterable, List, Optional, Tuple

import requests

DEFAULT_S3_BASE = "https://wadarchive.nyc3.digitaloceanspaces.com"


# -----------------------------
# Helpers
# -----------------------------

def eprint(*args: Any, **kwargs: Any) -> None:
    print(*args, file=sys.stderr, **kwargs)


def read_json_file(path: str) -> Any:
    with open(path, "r", encoding="utf-8") as f:
        lines = f.readlines()
        items = [json.loads(line) for line in lines if line.strip()]
        return items


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
                    out["text_files"].append({"path": fname, "size": info.file_size})
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
    for p in prefixes:
        url = f"{s3_base.rstrip('/')}/{sha1}/{p}{sha1}.{ext}.gz"
        try:
            r = session.head(url, timeout=timeout, allow_redirects=True)
            if r.status_code == 200:
                return url
        except requests.RequestException:
            continue
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
    s3_url: Optional[str],
    extracted: Dict[str, Any],
    wad_archive: Dict[str, Any],
    idgames: Optional[Dict[str, Any]],
) -> Dict[str, Any]:
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
        "title": title,
        "authors": authors,
        "descriptions": descriptions,
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
            "extracted": extracted,
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

    # Prune nulls for cleanliness
    return prune_nones(out)


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
    ap.add_argument("--wads-json", required=True, help="Path to wads.json")
    ap.add_argument("--idgames-json", required=True, help="Path to idgames.json")
    ap.add_argument("--s3-base", default=DEFAULT_S3_BASE, help="Base URL for public S3 bucket")
    ap.add_argument("--limit", type=int, default=0, help="Process only N wads (0 = all)")
    ap.add_argument("--start", type=int, default=0, help="Start index into wads.json array")
    ap.add_argument("--pretty", action="store_true", help="Pretty-print JSON")
    ap.add_argument("--sleep", type=float, default=0.0, help="Sleep seconds between items (politeness)")
    args = ap.parse_args()

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

    for idx in range(start, end):
        wad_entry = wads_data[idx]
        if not isinstance(wad_entry, dict):
            continue

        sha1 = str(wad_entry.get("_id") or "").lower()
        if not sha1 or not re.fullmatch(r"[0-9a-f]{40}", sha1):
            continue

        wad_type = str(wad_entry.get("type") or "UNKNOWN")
        ext = TYPE_TO_EXT.get(wad_type, None) or "wad"  # default best-guess

        prefixes = candidate_prefixes(wad_entry)
        s3_url = resolve_s3_url(session, args.s3_base, sha1, ext, prefixes)

        extracted: Dict[str, Any]
        if not s3_url:
            extracted = {
                "format": "unknown",
                "error": "Could not resolve S3 object URL (prefix mismatch).",
                "tried_prefixes": prefixes,
                "expected_ext": ext,
            }
            out = build_output_object(
                sha1=sha1,
                s3_url=None,
                extracted=extracted,
                wad_archive=wad_entry,
                idgames=id_lookup.get(sha1),
            )
            print(json.dumps(out, indent=2 if args.pretty else None, ensure_ascii=False))
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

                extracted = extract_metadata_from_file(file_path, ext)

            except Exception as ex:
                extracted = {
                    "format": "unknown",
                    "error": f"Download/decompress/extract failed: {type(ex).__name__}: {ex}",
                }

            out = build_output_object(
                sha1=sha1,
                s3_url=s3_url,
                extracted=extracted,
                wad_archive=wad_entry,
                idgames=id_lookup.get(sha1),
            )
            print(json.dumps(out, indent=2 if args.pretty else None, ensure_ascii=False))

        # Temp directory auto-deletes here

        if args.sleep > 0:
            time.sleep(args.sleep)


if __name__ == "__main__":
    main()
