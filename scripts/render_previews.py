#!/usr/bin/env python3
"""Render exported Ratatui fixture frames into README GIFs; no live workload.

Development-only requirement: Pillow and a monospace font. See docs/validation/README.md.
"""
import argparse
from pathlib import Path
import xml.etree.ElementTree as ET

from PIL import Image, ImageDraw, ImageFont


def render(path, font):
    root = ET.parse(path).getroot()
    canvas = Image.new("RGB", (int(root.get("width")), int(root.get("height"))))
    draw = ImageDraw.Draw(canvas)
    for item in root:
        tag = item.tag.split("}")[-1]
        x, y = int(item.get("x", 0)), int(item.get("y", 0))
        if tag == "rect":
            w, h = int(item.get("width")), int(item.get("height"))
            draw.rectangle((x, y, x + w - 1, y + h - 1), fill=item.get("fill"))
        elif tag == "text":
            draw.text((x, y), item.text or "", font=font, fill=item.get("fill"), anchor="ls")
    return canvas


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("frames", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--font", default="/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf")
    args = parser.parse_args()
    font = ImageFont.truetype(args.font, 16)
    args.output.mkdir(parents=True, exist_ok=True)
    for case, name in [("a", "busy-low-wait"), ("b", "busy-high-wait"), ("c", "relaxed"), ("e", "strained")]:
        frames = [render(path, font) for path in sorted((args.frames / case).glob("*.svg"))]
        if not frames:
            parser.error(f"no exported frames for case {case}")
        frames[0].save(args.output / f"{name}.png")
        if name == "relaxed":
            print(f"{name}: static synthetic fixture")
        else:
            # One palette keeps unchanged UI colours stable across GIF frames.
            palette = frames[0].quantize(colors=256)
            indexed = [frame.quantize(palette=palette, dither=Image.Dither.NONE) for frame in frames]
            indexed[0].save(args.output / f"{name}.gif", save_all=True,
                            append_images=indexed[1:], duration=100, loop=0,
                            disposal=2, optimize=False)
            print(f"{name}: {len(frames)} frames, 100 ms/frame, synthetic fixture")



if __name__ == "__main__":
    main()
