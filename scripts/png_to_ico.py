#!/usr/bin/env python3
"""Wrap a PNG file as a minimal single-entry Windows .ico.

Windows Vista and later accept a raw PNG payload inside an ICO
directory entry (no BMP re-encoding, no scaling), so we can produce a
valid .ico from any source PNG without a graphics library. Windows
resizes for taskbar/desktop use.

Usage:
    python3 scripts/png_to_ico.py <source.png> <target.ico>
"""

from __future__ import annotations

import struct
import sys
from pathlib import Path


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2

    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])
    png = src.read_bytes()

    # 6-byte ICONDIR header + one 16-byte ICONDIRENTRY = 22-byte prefix.
    # Width/height sentinel 0 means "256 or greater" — the source PNG is
    # 1024×1024, so we take the sentinel path.
    header = struct.pack(
        "<HHH",  # reserved, type=icon(1), count
        0,
        1,
        1,
    )
    entry = struct.pack(
        "<BBBBHHII",  # w, h, colors, reserved, planes, bits, size, offset
        0,  # width = 256+
        0,  # height = 256+
        0,  # palette colors (32-bit = 0)
        0,  # reserved
        1,  # planes
        32,  # bits per pixel
        len(png),
        6 + 16,  # payload offset
    )
    dst.write_bytes(header + entry + png)
    print(f"wrote {dst} ({dst.stat().st_size} B)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
