#!/usr/bin/env python3
"""Compose the phone illustration of the README from the phone-size screenshots.

    python3 scripts/compose-mobile.py [out.png] [shot.png ...]

Default: screenshots/home-mobile.png, mobile.png and lap-mobile.png (written by
screenshot-home.mjs, screenshot.mjs and screenshot-sessions.mjs --spawn), composed into
../../docs/images/mobile.png.

Every screenshot is taken at 390 CSS px wide with a device scale factor of 2 (780 px). Each one
is shown **whole and at the same scale**: the first screen of the page (390 × 844 CSS px), never
a zoomed-in corner. Needs Pillow.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
SHOTS = HERE.parent / "screenshots"
DEFAULT_OUT = HERE.parent.parent.parent / "docs" / "images" / "mobile.png"
DEFAULT_SHOTS = ["home-mobile.png", "mobile.png", "lap-mobile.png"]

CSS_W, CSS_H = 390, 844  # one phone screen, CSS px
SCALE = 1.25  # output px per CSS px
GAP = 44
PAD = 44
RADIUS = 34
BEZEL = 8
BG = (11, 14, 20)
FRAME = (38, 45, 58)


def phone(path: Path) -> Image.Image:
    shot = Image.open(path).convert("RGB")
    ratio = shot.width / CSS_W  # device scale factor of the screenshot
    first_screen = shot.crop((0, 0, shot.width, min(shot.height, round(CSS_H * ratio))))
    w, h = round(CSS_W * SCALE), round(CSS_H * SCALE)
    screen = Image.new("RGB", (w, h), BG)
    resized = first_screen.resize(
        (w, round(first_screen.height * w / first_screen.width)), Image.LANCZOS
    )
    screen.paste(resized, (0, 0))

    # Rounded screen inside a thin frame.
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, w - 1, h - 1), RADIUS, fill=255)
    framed = Image.new("RGB", (w + 2 * BEZEL, h + 2 * BEZEL), BG)
    ImageDraw.Draw(framed).rounded_rectangle(
        (0, 0, framed.width - 1, framed.height - 1), RADIUS + BEZEL, fill=FRAME
    )
    framed.paste(screen, (BEZEL, BEZEL), mask)
    return framed


def main() -> None:
    args = sys.argv[1:]
    out = Path(args[0]) if args else DEFAULT_OUT
    shots = [Path(a) for a in args[1:]] or [SHOTS / n for n in DEFAULT_SHOTS]
    missing = [str(s) for s in shots if not s.is_file()]
    if missing:
        sys.exit("missing screenshot(s): " + ", ".join(missing))

    phones = [phone(s) for s in shots]
    width = 2 * PAD + sum(p.width for p in phones) + GAP * (len(phones) - 1)
    height = 2 * PAD + max(p.height for p in phones)
    canvas = Image.new("RGB", (width, height), BG)
    x = PAD
    for p in phones:
        canvas.paste(p, (x, PAD))
        x += p.width + GAP
    canvas.save(out, optimize=True)
    print(f"{out} ({width}×{height})")


if __name__ == "__main__":
    main()
