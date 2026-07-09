#!/usr/bin/env python3
"""Regenerate CSP app icons (Claude / Anthropic warm terracotta palette)."""

from __future__ import annotations

import math
import os
import random
import shutil
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent

# Match desktop/src/styles.css Claude tokens
ACCENT = (217, 119, 87)       # #D97757
ACCENT_DEEP = (201, 100, 66)  # #C96442
TEXT = (245, 244, 240)        # #F5F4F0

# Deterministic organic jitter for the starburst (hand-drawn feel).
_STAR_SEED = 42


def _load_font(size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    candidates = [
        "/System/Library/Fonts/SFNSRounded.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/Library/Fonts/Arial Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    ]
    for path in candidates:
        if os.path.isfile(path):
            try:
                return ImageFont.truetype(path, size)
            except OSError:
                continue
    return ImageFont.load_default()


def _lerp(a: int, b: int, t: float) -> int:
    return int(a + (b - a) * t)


def _draw_starburst(draw: ImageDraw.ImageDraw, size: int) -> None:
    """White organic asterisk in upper-center (Claude-style mark)."""
    cx = size * 0.5
    cy = size * 0.38
    arm_count = 6
    inner_r = size * 0.04
    outer_r = size * 0.17
    line_w = max(2, int(size * 0.038))

    rng = random.Random(_STAR_SEED)
    for i in range(arm_count):
        base_angle = (2 * math.pi * i / arm_count) - math.pi / 2
        angle = base_angle + rng.uniform(-0.09, 0.09)
        length_scale = rng.uniform(0.88, 1.08)
        r_outer = outer_r * length_scale
        r_inner = inner_r * rng.uniform(0.75, 1.05)

        x0 = cx + math.cos(angle) * r_inner
        y0 = cy + math.sin(angle) * r_inner
        x1 = cx + math.cos(angle) * r_outer
        y1 = cy + math.sin(angle) * r_outer

        # Slight curve via a midpoint offset perpendicular to the arm.
        mid_x = (x0 + x1) / 2
        mid_y = (y0 + y1) / 2
        perp = angle + math.pi / 2
        bend = rng.uniform(-size * 0.012, size * 0.012)
        ctrl_x = mid_x + math.cos(perp) * bend
        ctrl_y = mid_y + math.sin(perp) * bend

        draw.line([(x0, y0), (ctrl_x, ctrl_y), (x1, y1)], fill=TEXT, width=line_w)


def _draw_letterspaced_text(
    draw: ImageDraw.ImageDraw,
    label: str,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    y: int,
    size: int,
    tracking: float,
) -> None:
    """Draw label centered horizontally with extra letter spacing."""
    widths: list[int] = []
    bboxes: list[tuple[int, int, int, int]] = []
    for ch in label:
        bbox = draw.textbbox((0, 0), ch, font=font)
        bboxes.append(bbox)
        widths.append(bbox[2] - bbox[0])

    total_w = sum(widths) + tracking * (len(label) - 1)
    x = (size - total_w) / 2

    for ch, bbox, w in zip(label, bboxes, widths):
        draw.text((x - bbox[0], y - bbox[1]), ch, font=font, fill=TEXT)
        x += w + tracking


def render_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    px = img.load()

    # Warm terracotta vertical gradient — no gray overlays or center shadows.
    for y in range(size):
        t = y / max(size - 1, 1)
        r = _lerp(ACCENT[0], ACCENT_DEEP[0], t)
        g = _lerp(ACCENT[1], ACCENT_DEEP[1], t)
        b = _lerp(ACCENT[2], ACCENT_DEEP[2], t)
        for x in range(size):
            px[x, y] = (r, g, b, 255)

    draw = ImageDraw.Draw(img)
    _draw_starburst(draw, size)

    font_size = max(8, int(size * 0.18))
    font = _load_font(font_size)
    label = "CSP"
    tracking = max(1.0, size * 0.02)

    # Measure tallest glyph for bottom placement.
    sample_bbox = draw.textbbox((0, 0), "C", font=font)
    th = sample_bbox[3] - sample_bbox[1]
    bottom_pad = size * 0.14
    ty = int(size - bottom_pad - th)

    _draw_letterspaced_text(draw, label, font, ty, size, tracking)

    return img


def write_png(path: Path, size: int) -> None:
    render_icon(size).save(path, format="PNG")
    print(f"wrote {path.name} ({size}px)")


def write_ico(path: Path) -> None:
    frames = [render_icon(s) for s in (16, 24, 32, 48, 64, 128, 256)]
    frames[0].save(
        path,
        format="ICO",
        sizes=[(f.width, f.height) for f in frames],
        append_images=frames[1:],
    )
    print(f"wrote {path.name}")


def write_icns(path: Path) -> None:
    iconset = ROOT / "CSP.iconset"
    if iconset.exists():
        shutil.rmtree(iconset)
    iconset.mkdir()

    mapping = {
        "icon_16x16.png": 16,
        "icon_16x16@2x.png": 32,
        "icon_32x32.png": 32,
        "icon_32x32@2x.png": 64,
        "icon_128x128.png": 128,
        "icon_128x128@2x.png": 256,
        "icon_256x256.png": 256,
        "icon_256x256@2x.png": 512,
        "icon_512x512.png": 512,
        "icon_512x512@2x.png": 1024,
    }
    for name, sz in mapping.items():
        render_icon(sz).save(iconset / name, format="PNG")

    subprocess.run(
        ["iconutil", "-c", "icns", str(iconset), "-o", str(path)],
        check=True,
    )
    shutil.rmtree(iconset)
    print(f"wrote {path.name}")


def main() -> int:
    write_png(ROOT / "32x32.png", 32)
    write_png(ROOT / "64x64.png", 64)
    write_png(ROOT / "128x128.png", 128)
    write_png(ROOT / "128x128@2x.png", 256)
    write_png(ROOT / "icon.png", 512)

    square_map = {
        "Square30x30Logo.png": 30,
        "Square44x44Logo.png": 44,
        "Square71x71Logo.png": 71,
        "Square89x89Logo.png": 89,
        "Square107x107Logo.png": 107,
        "Square142x142Logo.png": 142,
        "Square150x150Logo.png": 150,
        "Square284x284Logo.png": 284,
        "Square310x310Logo.png": 310,
        "StoreLogo.png": 50,
    }
    for name, sz in square_map.items():
        write_png(ROOT / name, sz)

    write_ico(ROOT / "icon.ico")
    if sys.platform == "darwin":
        write_icns(ROOT / "icon.icns")
    else:
        print("skip icon.icns (not macOS)")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
