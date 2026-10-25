#!/usr/bin/env python3
"""Draws the port's icon (crates/holiday/res/icon.png and icon.ico): an
island with a palm tree in the sea under the sun. It's the port's own, not
the game's. Needs Pillow.

Usage: tools/make-icon.py
"""
import math
from pathlib import Path

from PIL import Image, ImageDraw

S = 1024  # drawn large, scaled down smooth
OUT = Path(__file__).resolve().parent.parent / "crates/holiday/res"


def draw() -> Image.Image:
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    mask = Image.new("L", (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle((32, 32, S - 32, S - 32), radius=200, fill=255)
    scene = Image.new("RGBA", (S, S))
    d = ImageDraw.Draw(scene)
    # the sky, lighter towards the horizon
    for y in range(S):
        t = y / (S * 0.62)
        d.line(((0, y), (S, y)), fill=(int(70 + 140 * t), int(160 + 70 * t), int(235 + 15 * t)))
    # the sun
    d.ellipse((610, 130, 850, 370), fill=(255, 214, 64))
    # the sea
    sea_top = int(S * 0.62)
    for y in range(sea_top, S):
        t = (y - sea_top) / (S - sea_top)
        d.line(((0, y), (S, y)), fill=(int(30 - 20 * t), int(140 - 60 * t), int(200 - 50 * t)))
    for i, y in enumerate(range(sea_top + 70, S, 80)):
        x0 = 120 + (i % 2) * 260
        for x in range(x0, S, 520):
            d.arc((x, y, x + 120, y + 40), 200, 340, fill=(160, 220, 245), width=10)
    # the island
    d.ellipse((150, sea_top - 70, 870, sea_top + 90), fill=(240, 214, 140))
    d.ellipse((230, sea_top - 60, 790, sea_top + 30), fill=(250, 228, 160))
    # the palm's trunk, curved, in segments
    base, top = (520, sea_top - 30), (420, 330)
    pts = []
    for k in range(13):
        t = k / 12
        x = base[0] + (top[0] - base[0]) * t + math.sin(t * math.pi) * 70
        y = base[1] + (top[1] - base[1]) * t
        pts.append((x, y))
    for k in range(12):
        w = int(46 - 18 * k / 12)
        d.line((pts[k], pts[k + 1]), fill=(130, 84, 44), width=w)
        d.line((pts[k], pts[k + 1]), fill=(160, 108, 60), width=max(4, w - 22))
    # the fronds
    cx, cy = pts[-1]
    for ang, length, droop in ((200, 290, 90), (160, 300, 110), (235, 230, 60),
                               (330, 300, 110), (20, 280, 100), (300, 230, 60), (270, 170, 20)):
        a = math.radians(ang)
        ex, ey = cx + math.cos(a) * length, cy - math.sin(a) * length * 0.55 + droop
        mx, my = (cx + ex) / 2, (cy + ey) / 2 - 60
        curve = []
        for k in range(21):
            t = k / 20
            x = (1 - t) ** 2 * cx + 2 * (1 - t) * t * mx + t * t * ex
            y = (1 - t) ** 2 * cy + 2 * (1 - t) * t * my + t * t * ey
            curve.append((x, y))
        for k in range(20):
            w = int(54 * (1 - k / 20) + 8)
            d.line((curve[k], curve[k + 1]), fill=(34, 139, 60), width=w)
        d.line(curve, fill=(70, 180, 80), width=8)
    # coconuts
    for dx, dy in ((-22, 18), (14, 26), (-4, 40)):
        d.ellipse((cx + dx - 20, cy + dy - 20, cx + dx + 20, cy + dy + 20), fill=(110, 70, 30))
    img.paste(scene, (0, 0), mask)
    return img


def main() -> None:
    big = draw()
    big.resize((512, 512), Image.LANCZOS).save(OUT / "icon.png")
    big.resize((256, 256), Image.LANCZOS).save(
        OUT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    )


if __name__ == "__main__":
    main()
