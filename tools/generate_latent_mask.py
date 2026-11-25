#!/usr/bin/env python3
"""Regenerate the raw 800x450 alpha matte for the experimental phase lettering."""
from PIL import Image, ImageDraw
from generate_assets import ASSETS, WIDTH, HEIGHT, SCALE, font


def make_mask():
    mask = Image.new("L", (WIDTH * SCALE, HEIGHT * SCALE))
    brush = ImageDraw.Draw(mask)
    message = "翻拍死全家"
    face = font(45, weight=660)
    left, top, right, bottom = brush.textbbox((0, 0), message, font=face, language="zh-Hans")
    x = (WIDTH * SCALE - (right - left)) // 2 - left
    brush.text((x, 39 * SCALE - top), message, fill=255, font=face, language="zh-Hans")
    return mask.resize((WIDTH, HEIGHT), Image.Resampling.LANCZOS)


if __name__ == "__main__":
    ASSETS.mkdir(parents=True, exist_ok=True)
    (ASSETS / "latent-mask.raw").write_bytes(make_mask().tobytes())
