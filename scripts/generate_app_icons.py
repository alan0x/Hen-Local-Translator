#!/usr/bin/env python3
"""Generate exact, platform-ready Hen Local application icons from the source logo."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageFilter


MASTER_SIZE = 1024
ICONSET_SIZES = (
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
)


def superellipse_mask(size: int, inset: int, exponent: float = 5.0) -> Image.Image:
    scale = 4
    side = size * scale
    center = (side - 1) / 2
    radius = (size / 2 - inset) * scale
    pixels = bytearray(side * side)
    for y in range(side):
        ny = abs((y - center) / radius) ** exponent
        row = y * side
        for x in range(side):
            inside = abs((x - center) / radius) ** exponent + ny <= 1.0
            pixels[row + x] = 255 if inside else 0
    mask = Image.frombytes("L", (side, side), bytes(pixels))
    return mask.resize((size, size), Image.Resampling.LANCZOS)


def create_master(source: Path) -> Image.Image:
    logo = Image.open(source).convert("RGB")
    if logo.width != logo.height:
        side = max(logo.size)
        square = Image.new("RGB", (side, side), "white")
        square.paste(logo, ((side - logo.width) // 2, (side - logo.height) // 2))
        logo = square

    canvas = Image.new("RGBA", (MASTER_SIZE, MASTER_SIZE), (0, 0, 0, 0))
    # macOS Dock icons need optical padding; a nearly full-canvas tile appears
    # larger than neighboring system icons even when the bitmap size matches.
    tile_mask = superellipse_mask(MASTER_SIZE, inset=88)

    shadow_mask = tile_mask.filter(ImageFilter.GaussianBlur(16))
    shadow = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    shadow_alpha = shadow_mask.point(lambda value: round(value * 0.18))
    shadow.putalpha(shadow_alpha)
    canvas.alpha_composite(shadow, (0, 8))

    tile = Image.new("RGBA", canvas.size, "white")
    tile.putalpha(tile_mask)

    # The supplied artwork is only resized and centered; its geometry is never redrawn.
    artwork_size = 860
    artwork = logo.resize((artwork_size, artwork_size), Image.Resampling.LANCZOS).convert("RGBA")
    artwork_offset = (MASTER_SIZE - artwork_size) // 2
    tile.alpha_composite(artwork, (artwork_offset, artwork_offset))
    tile.putalpha(tile_mask)
    canvas.alpha_composite(tile)
    return canvas


def create_transparent_mark(source: Path) -> Image.Image:
    """Extract the exact black artwork without redrawing its JPEG edges."""
    logo = Image.open(source).convert("RGB")
    luminance = logo.convert("L")
    alpha = luminance.point(
        lambda value: 0
        if value >= 250
        else 255
        if value <= 210
        else round((250 - value) / 40 * 255)
    )
    bounds = alpha.getbbox()
    if bounds is None:
        raise RuntimeError("source logo contains no dark artwork")

    mark = Image.new("RGBA", logo.size, (0, 0, 0, 0))
    mark.putalpha(alpha)
    mark = mark.crop(bounds)

    maximum = 960
    scale = min(maximum / mark.width, maximum / mark.height)
    mark = mark.resize(
        (round(mark.width * scale), round(mark.height * scale)),
        Image.Resampling.LANCZOS,
    )
    output = Image.new("RGBA", (MASTER_SIZE, MASTER_SIZE), (0, 0, 0, 0))
    output.alpha_composite(
        mark,
        ((MASTER_SIZE - mark.width) // 2, (MASTER_SIZE - mark.height) // 2),
    )
    return output


def write_iconset(master: Image.Image, iconset: Path) -> None:
    iconset.mkdir(parents=True, exist_ok=True)
    for size, filename in ICONSET_SIZES:
        master.resize((size, size), Image.Resampling.LANCZOS).save(iconset / filename)


def create_icns(master: Image.Image, destination: Path) -> None:
    iconutil = shutil.which("iconutil")
    if not iconutil:
        raise RuntimeError("iconutil is required to create the macOS .icns file")
    with tempfile.TemporaryDirectory(prefix="henlocal-icon-") as temporary:
        iconset = Path(temporary) / "AppIcon.iconset"
        write_iconset(master, iconset)
        subprocess.run(
            [iconutil, "-c", "icns", str(iconset), "-o", str(destination)],
            check=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    args.output_dir.mkdir(parents=True, exist_ok=True)
    master = create_master(args.source)
    create_transparent_mark(args.source).save(
        args.output_dir / "logo-mark.png", optimize=True
    )
    master.save(args.output_dir / "icon.png", optimize=True)
    master.resize((32, 32), Image.Resampling.LANCZOS).save(args.output_dir / "32x32.png")
    master.resize((128, 128), Image.Resampling.LANCZOS).save(args.output_dir / "128x128.png")
    master.resize((256, 256), Image.Resampling.LANCZOS).save(
        args.output_dir / "128x128@2x.png"
    )
    master.save(
        args.output_dir / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    create_icns(master, args.output_dir / "icon.icns")


if __name__ == "__main__":
    main()
