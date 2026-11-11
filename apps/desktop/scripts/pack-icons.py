#!/usr/bin/env python3
"""Packs the PNG frames rendered by make-icons.mjs into icon.ico and icon.icns.

    pack-icons.py <frames dir with 16.png … 1024.png> <src-tauri/icons>

Each frame is used at its own size (no resampling): the .ico holds 16, 24, 32, 48, 64 and
256 px (Windows: notification area, task bar, Explorer, installer), the .icns 16 to 1024 px (1x and 2x).
"""

import struct
import sys
from pathlib import Path

from PIL import Image

ICO_SIZES = [16, 24, 32, 48, 64, 256]
# ICNS chunk types holding a PNG: (type, pixel size). 1x and 2x ("@2x") variants.
ICNS_CHUNKS = [
    ("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256), ("ic09", 512),
    ("ic10", 1024), ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512),
]


def frames(src: Path, sizes):
    return [Image.open(src / f"{s}.png").convert("RGBA") for s in sizes]


def main() -> None:
    src, out = Path(sys.argv[1]), Path(sys.argv[2])
    ico = frames(src, ICO_SIZES)
    ico[-1].save(
        out / "icon.ico",
        format="ICO",
        sizes=[(s, s) for s in ICO_SIZES],
        append_images=ico[:-1],
    )
    # Written by hand: every chunk is the PNG rendered at that size, as is.
    chunks = b"".join(
        kind.encode() + struct.pack(">I", 8 + len(data)) + data
        for kind, size in ICNS_CHUNKS
        for data in [(src / f"{size}.png").read_bytes()]
    )
    (out / "icon.icns").write_bytes(b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks)
    for name in ("icon.ico", "icon.icns"):
        im = Image.open(out / name)
        print(out / name, sorted(im.info.get("sizes", [])))


if __name__ == "__main__":
    main()
