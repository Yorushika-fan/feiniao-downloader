#!/usr/bin/env python3
"""Generate a colorful, polished icon set for 飞鸟下载器 (FeiNiao Downloader)."""
from __future__ import annotations
import os
import math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFilter


ROOT = Path(__file__).resolve().parent.parent
ICON_DIR = ROOT / "src-tauri" / "icons"
ICON_DIR.mkdir(parents=True, exist_ok=True)


def hex_to_rgb(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return tuple(int(h[i : i + 2], 16) for i in (0, 2, 4))


def gradient_image(size: int, c1: tuple[int, int, int], c2: tuple[int, int, int]) -> Image.Image:
    img = Image.new("RGB", (size, size), c1)
    px = img.load()
    for y in range(size):
        for x in range(size):
            t = (x + y) / (2 * (size - 1))
            r = int(c1[0] + (c2[0] - c1[0]) * t)
            g = int(c1[1] + (c2[1] - c1[1]) * t)
            b = int(c1[2] + (c2[2] - c1[2]) * t)
            px[x, y] = (r, g, b)
    return img


def rounded_mask(size: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle((0, 0, size, size), radius=radius, fill=255)
    return mask


def make_source(size: int = 1024) -> Image.Image:
    bg = gradient_image(size, hex_to_rgb("#6366F1"), hex_to_rgb("#A855F7"))

    # Apply rounded mask
    mask = rounded_mask(size, radius=int(size * 0.22))
    rgba = bg.convert("RGBA")
    rgba.putalpha(mask)

    overlay = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    od = ImageDraw.Draw(overlay)

    # Soft highlight
    od.ellipse(
        (-size * 0.2, -size * 0.5, size * 0.9, size * 0.5),
        fill=(255, 255, 255, 35),
    )

    # Downward arrow icon centered
    cx, cy = size // 2, size // 2
    shaft_w = int(size * 0.16)
    shaft_h = int(size * 0.36)
    od.rounded_rectangle(
        (
            cx - shaft_w // 2,
            cy - shaft_h // 2 - int(size * 0.05),
            cx + shaft_w // 2,
            cy + shaft_h // 2 - int(size * 0.05),
        ),
        radius=shaft_w // 3,
        fill=(255, 255, 255, 245),
    )
    # Arrow head triangle
    head_w = int(size * 0.36)
    head_h = int(size * 0.20)
    head_top_y = cy + shaft_h // 2 - int(size * 0.10)
    od.polygon(
        [
            (cx - head_w // 2, head_top_y),
            (cx + head_w // 2, head_top_y),
            (cx, head_top_y + head_h),
        ],
        fill=(255, 255, 255, 245),
    )

    # Tray / ground bar
    od.rounded_rectangle(
        (
            cx - int(size * 0.27),
            cy + int(size * 0.30),
            cx + int(size * 0.27),
            cy + int(size * 0.34),
        ),
        radius=int(size * 0.02),
        fill=(255, 255, 255, 230),
    )

    rgba = Image.alpha_composite(rgba, overlay)
    return rgba


def save_png(img: Image.Image, name: str, size: int) -> None:
    out = img.resize((size, size), Image.LANCZOS)
    out.save(ICON_DIR / name, "PNG")


def main() -> None:
    src = make_source(1024)
    # Tauri default-required names
    save_png(src, "32x32.png", 32)
    save_png(src, "128x128.png", 128)
    save_png(src, "128x128@2x.png", 256)
    save_png(src, "icon.png", 1024)
    # Windows ico placeholder (Tauri tooling will accept it)
    src.resize((256, 256), Image.LANCZOS).save(ICON_DIR / "icon.ico", format="ICO", sizes=[(256, 256)])
    # macOS icns: build via iconset
    iconset_dir = ICON_DIR.parent / "icon.iconset"
    iconset_dir.mkdir(exist_ok=True)
    pairs = [
        (16, "icon_16x16.png"),
        (32, "icon_16x16@2x.png"),
        (32, "icon_32x32.png"),
        (64, "icon_32x32@2x.png"),
        (128, "icon_128x128.png"),
        (256, "icon_128x128@2x.png"),
        (256, "icon_256x256.png"),
        (512, "icon_256x256@2x.png"),
        (512, "icon_512x512.png"),
        (1024, "icon_512x512@2x.png"),
    ]
    for sz, fname in pairs:
        src.resize((sz, sz), Image.LANCZOS).save(iconset_dir / fname, "PNG")
    # convert via iconutil if available; otherwise leave PNGs and let Tauri fallback
    icns_target = ICON_DIR / "icon.icns"
    rc = os.system(f"iconutil -c icns {iconset_dir} -o {icns_target}")
    if rc != 0:
        # fallback: copy biggest png as icns (Tauri may accept) - but better skip
        pass
    print("Icons generated in", ICON_DIR)


if __name__ == "__main__":
    main()
