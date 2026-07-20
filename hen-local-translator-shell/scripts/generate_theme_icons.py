#!/usr/bin/env python3
"""Generate Dock icons with a themed tile and contrasting claw marks."""

from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "icons" / "icon.png"
THEMES = {
    "neon-blue": ((0x00, 0x03, 0xFE), (0xFF, 0xFF, 0xFF)),
    "neon-orange": ((0xFF, 0x57, 0x05), (0xFF, 0xFF, 0xFF)),
    "neon-pink": ((0xFF, 0x00, 0x73), (0xFF, 0xFF, 0xFF)),
    "neon-green": ((0x51, 0xF9, 0x1B), (0x10, 0x11, 0x12)),
}


def recolor(
    theme: str,
    accent: tuple[int, int, int],
    foreground: tuple[int, int, int],
) -> None:
    image = Image.open(SOURCE).convert("RGBA")
    pixels = image.load()

    # Preserve the transparent corners and native shadow. Inside the opaque
    # tile, the source luminance cleanly separates the white tile from the
    # black claw marks and also preserves their antialiased edges.
    for y in range(image.height):
        for x in range(image.width):
            red, green, blue, alpha = pixels[x, y]
            luminance = (red + green + blue) / 3
            if alpha < 240:
                continue
            blend = luminance / 255
            pixels[x, y] = (
                round(foreground[0] * (1 - blend) + accent[0] * blend),
                round(foreground[1] * (1 - blend) + accent[1] * blend),
                round(foreground[2] * (1 - blend) + accent[2] * blend),
                alpha,
            )

    image.save(ROOT / "icons" / f"icon-{theme}.png", optimize=True)


for theme_name, (theme_accent, theme_foreground) in THEMES.items():
    recolor(theme_name, theme_accent, theme_foreground)
