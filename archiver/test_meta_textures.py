import os
import struct
import sys
import unittest

# Ensure archiver/ is importable (meta-worker imports `meta` from this directory).
sys.path.insert(0, os.path.dirname(__file__))

import meta  # noqa: E402


def _name8(s: str) -> bytes:
    b = (s or "").encode("ascii", errors="replace")
    if len(b) > 8:
        b = b[:8]
    return b.ljust(8, b"\x00")


def _build_wad(lumps: list[tuple[str, bytes]]) -> bytes:
    # Minimal PWAD builder: header + concatenated lump data + directory.
    data_parts: list[bytes] = []
    entries: list[tuple[int, int, str]] = []

    off = 12
    for name, data in lumps:
        data = data or b""
        entries.append((off, len(data), name))
        data_parts.append(data)
        off += len(data)

    dir_off = off
    dir_bytes = b"".join(
        struct.pack("<II8s", e_off, e_size, _name8(e_name)) for (e_off, e_size, e_name) in entries
    )

    header = struct.pack("<4sii", b"PWAD", len(entries), dir_off)
    return header + b"".join(data_parts) + dir_bytes


def _sidedef(*, upper: str, lower: str, middle: str, sector: int = 0) -> bytes:
    return struct.pack(
        "<hh8s8s8sh",
        0,
        0,
        _name8(upper),
        _name8(lower),
        _name8(middle),
        sector,
    )


def _sector(*, floor: str, ceil: str) -> bytes:
    return struct.pack(
        "<hh8s8shhh",
        0,
        0,
        _name8(floor),
        _name8(ceil),
        0,
        0,
        0,
    )


class TestMetaTexturesHistogram(unittest.TestCase):
    def test_textures_histogram_counts(self) -> None:
        sidedefs = b"".join(
            [
                _sidedef(upper="STONE", lower="-", middle="BRICK"),
                _sidedef(upper="STONE", lower="BRICK", middle="STONE"),
            ]
        )
        sectors = b"".join(
            [
                _sector(floor="FLOOR0_1", ceil="CEIL1_1"),
                _sector(floor="FLOOR0_1", ceil="SKY1"),
            ]
        )

        wad = _build_wad(
            [
                ("MAP01", b""),
                ("SIDEDEFS", sidedefs),
                ("SECTORS", sectors),
            ]
        )

        maps = meta.extract_per_map_stats_from_wad_bytes(wad)
        self.assertEqual(len(maps), 1)
        textures = maps[0]["stats"]["textures"]

        self.assertIsInstance(textures, dict)
        self.assertEqual(textures.get("STONE"), 3)
        self.assertEqual(textures.get("BRICK"), 2)
        self.assertEqual(textures.get("FLOOR0_1"), 2)
        self.assertEqual(textures.get("CEIL1_1"), 1)
        self.assertEqual(textures.get("SKY1"), 1)

    def test_textures_empty_object_not_null(self) -> None:
        wad = _build_wad(
            [
                ("MAP01", b""),
                # No SIDEDEFS/SECTORS
            ]
        )
        maps = meta.extract_per_map_stats_from_wad_bytes(wad)
        self.assertEqual(len(maps), 1)
        textures = maps[0]["stats"]["textures"]
        self.assertIsInstance(textures, dict)
        self.assertEqual(textures, {})


if __name__ == "__main__":
    unittest.main()
