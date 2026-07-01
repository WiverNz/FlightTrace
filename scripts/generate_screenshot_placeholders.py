#!/usr/bin/env python3
"""Regenerate `docs/screenshots/*.png` placeholder images in the current
FlightTrace palette. Run from the repo root.

The placeholders are schematic layouts (dark surface, colored badges,
mock rows / chart curves) with a diagonal `PLACEHOLDER` texture overlay,
so `docs/screenshots/README.md` never renders a broken image. Replace
each PNG with a real capture using the recipe in that README.
"""
from __future__ import annotations

import math
import os
import struct
import zlib
from pathlib import Path

# FlightTrace palette — mirrors web/src/styles/tokens.css and
# docs/tech-stack.md. Update in lockstep with those files.
BG = (9, 13, 20, 255)
SURFACE = (17, 23, 34, 255)
RAISED = (22, 29, 42, 255)
BORDER = (39, 50, 68, 255)
TEXT = (230, 237, 247, 255)
MUTED = (154, 168, 189, 255)
ACCENT = (110, 168, 254, 255)
DANGER = (255, 92, 108, 255)
WARN = (245, 198, 107, 255)
SUCCESS = (57, 217, 138, 255)

W, H = 800, 500
OUT_DIR = Path(__file__).resolve().parent.parent / "docs" / "screenshots"


def blend(a, b, t):
    return tuple(int(a[i] * (1 - t) + b[i] * t) for i in range(4))


def new_canvas(color=BG):
    return [[color for _ in range(W)] for _ in range(H)]


def fill_rect(px, x, y, w, h, color):
    for j in range(max(0, y), min(H, y + h)):
        for i in range(max(0, x), min(W, x + w)):
            px[j][i] = color


def stroke_rect(px, x, y, w, h, color, thick=1):
    fill_rect(px, x, y, w, thick, color)
    fill_rect(px, x, y + h - thick, w, thick, color)
    fill_rect(px, x, y, thick, h, color)
    fill_rect(px, x + w - thick, y, thick, h, color)


def diagonal_stamp(px):
    """Repeating diagonal stripes — universal 'placeholder' hint."""
    for y in range(H):
        for x in range(W):
            if (x + y) % 40 in (34, 35, 36):
                r, g, b, a = px[y][x]
                px[y][x] = blend((r, g, b, a), (245, 198, 107, 60), 0.28)


def encode_png(pixels, path: Path) -> int:
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw.extend((r, g, b, a))

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    path.write_bytes(png)
    return os.path.getsize(path)


def runs_list(px):
    fill_rect(px, 0, 0, W, 48, SURFACE)
    fill_rect(px, 24, 18, 140, 12, TEXT)
    fill_rect(px, W - 180, 18, 150, 14, ACCENT)
    fill_rect(px, 0, 60, W, H - 60, SURFACE)
    for i, badge in enumerate([DANGER, DANGER, SUCCESS, DANGER, SUCCESS]):
        y = 80 + i * 78
        fill_rect(px, 24, y, W - 48, 60, RAISED)
        stroke_rect(px, 24, y, W - 48, 60, BORDER)
        fill_rect(px, 44, y + 12, 220, 12, TEXT)
        fill_rect(px, 44, y + 34, 140, 8, MUTED)
        fill_rect(px, W - 180, y + 20, 130, 22, blend(badge, SURFACE, 0.7))
        fill_rect(px, W - 178, y + 22, 126, 18, badge)


def failure_summary(px):
    fill_rect(px, 0, 0, W, 48, SURFACE)
    fill_rect(px, 24, 18, 200, 12, TEXT)
    fill_rect(px, 0, 60, W, 120, RAISED)
    fill_rect(px, 24, 76, 320, 16, TEXT)
    fill_rect(px, 24, 100, 480, 8, MUTED)
    fill_rect(px, 24, 116, 320, 8, MUTED)
    fill_rect(px, W - 180, 90, 140, 30, blend(DANGER, SURFACE, 0.7))
    fill_rect(px, W - 178, 92, 136, 26, DANGER)
    fill_rect(px, 0, 200, W, H - 200, blend(DANGER, BG, 0.06))
    fill_rect(px, 24, 200, 4, H - 220, DANGER)
    fill_rect(px, 40, 216, 500, 14, TEXT)
    fill_rect(px, 40, 236, 620, 10, MUTED)
    fill_rect(px, 40, 250, 600, 10, MUTED)
    tile_colors = [ACCENT, TEXT, DANGER, TEXT, WARN]
    for i in range(5):
        x = 40 + i * 148
        fill_rect(px, x, 280, 136, 80, RAISED)
        stroke_rect(px, x, 280, 136, 80, BORDER)
        fill_rect(px, x + 10, 292, 90, 8, MUTED)
        fill_rect(px, x + 10, 308, 60, 14, tile_colors[i])
    fill_rect(px, 40, 380, W - 80, 90, SURFACE)
    stroke_rect(px, 40, 380, W - 80, 90, BORDER)
    for x in range(60, W - 60):
        t = (x - 60) / (W - 120)
        alt = min(0.28, 0.6 * math.tanh(t * 3))
        y = int(468 - alt * 90 / 0.4)
        for k in range(-1, 2):
            px[y + k][x] = ACCENT
    for x in range(60, W - 60, 8):
        px[400][x] = SUCCESS
        px[401][x] = SUCCESS


def telemetry(px):
    fill_rect(px, 0, 0, W, 48, SURFACE)
    fill_rect(px, 24, 18, 200, 12, TEXT)
    fill_rect(px, 24, 80, W - 48, 40, RAISED)
    fill_rect(px, 40, 92, 180, 14, MUTED)
    fill_rect(px, 24, 140, W - 48, 240, SURFACE)
    stroke_rect(px, 24, 140, W - 48, 240, BORDER)
    for y in [200, 250, 300, 350]:
        for x in range(50, W - 40, 6):
            px[y][x] = BORDER
    for x in range(60, W - 60):
        t = (x - 60) / (W - 120)
        alt = min(0.28, 0.6 * math.tanh(t * 3))
        y = int(370 - alt * 200 / 0.4)
        for k in range(-1, 2):
            px[y + k][x] = ACCENT
    for x in range(60, W - 60, 10):
        fill_rect(px, x, 190, 6, 2, SUCCESS)
    for y in range(150, 380):
        if y % 6 < 3:
            px[y][W - 200] = DANGER
    fill_rect(px, 24, 400, 260, 80, RAISED)
    stroke_rect(px, 24, 400, 260, 80, BORDER)
    for x in range(35, 275):
        t = (x - 35) / 240
        y = int(440 - 30 * math.sin(t * math.pi * 2))
        px[y][x] = ACCENT
        px[y + 1][x] = ACCENT
    fill_rect(px, 300, 400, 240, 80, RAISED)
    stroke_rect(px, 300, 400, 240, 80, BORDER)
    for i in range(3):
        fill_rect(px, 320, 412 + i * 22, 80, 6, MUTED)
        fill_rect(px, 420, 412 + i * 22, 100, 10, TEXT)


def desktop_window(px):
    fill_rect(px, 0, 0, W, 30, RAISED)
    fill_rect(px, 12, 10, 12, 12, DANGER)
    fill_rect(px, 28, 10, 12, 12, WARN)
    fill_rect(px, 44, 10, 12, 12, SUCCESS)
    fill_rect(px, W // 2 - 60, 10, 120, 12, MUTED)
    fill_rect(px, 0, 30, W, 60, SURFACE)
    fill_rect(px, 24, 48, 140, 12, TEXT)
    fill_rect(px, W - 180, 48, 150, 14, ACCENT)
    fill_rect(px, 0, 90, W, H - 90, SURFACE)
    for i, badge in enumerate([DANGER, SUCCESS, DANGER]):
        y = 110 + i * 100
        fill_rect(px, 24, y, W - 48, 80, RAISED)
        stroke_rect(px, 24, y, W - 48, 80, BORDER)
        fill_rect(px, 44, y + 16, 240, 16, TEXT)
        fill_rect(px, 44, y + 44, 160, 10, MUTED)
        fill_rect(px, W - 180, y + 26, 140, 26, blend(badge, SURFACE, 0.7))
        fill_rect(px, W - 178, y + 28, 136, 22, badge)
    stroke_rect(px, 0, 0, W, H, BORDER, thick=2)


SCENES = [
    ("runs-list.png", runs_list),
    ("run-detail-failure-summary.png", failure_summary),
    ("run-detail-telemetry.png", telemetry),
    ("desktop-window.png", desktop_window),
]


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name, draw in SCENES:
        px = new_canvas()
        draw(px)
        diagonal_stamp(px)
        size = encode_png(px, OUT_DIR / name)
        print(f"{name} {size} B")


if __name__ == "__main__":
    main()
