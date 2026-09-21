#!/usr/bin/env python3
"""Derive the application's icon files from the master logo.

The logo is one picture in `doc/img/`; the program needs it as a 256px PNG (the
window, taskbar and in-app mark) and as a multi-size ICO (the executable's PE
resource). This is that step — `build.ps1 -Icons` runs it, everything else just
picks up the two files it writes.

    python tools/make-icons.py                      # the defaults
    python tools/make-icons.py --preview sheet.png  # render a size comparison

It frames and scales, and does nothing to the picture itself: whatever the
master carries — a white outline, a shadow, a background — is what the icons
carry. Treatments belong in the artwork, where they can be judged at full size
and where changing one is a redraw rather than a flag.

Requires Pillow (`pip install pillow`); it is a tool for changing the logo, not
a build dependency, so it is not vendored or wired into `cargo build`.
"""

from __future__ import annotations

import argparse
import io
import struct
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:  # a clear message beats a traceback
    sys.exit("Pillow is required: pip install pillow")

# Sizes Windows picks from for the executable's icon: Explorer's small list, the
# taskbar, Alt-Tab, and the jumbo view. Anything in between is scaled by
# Windows, which is why the list stops at the two sizes it actually renders.
ICO_SIZES = (16, 24, 32, 48, 64, 128, 256)

# The alpha at which a pixel counts as the mark rather than its soft fringe,
# both for measuring what to frame and for flooding a background away.
INK = 8


def parse_color(text: str) -> tuple[int, int, int, int]:
    h = text.lstrip("#")
    if len(h) == 6:
        h += "ff"
    if len(h) != 8:
        raise argparse.ArgumentTypeError(f"expected #rrggbb or #rrggbbaa, got {text!r}")
    return tuple(int(h[i : i + 2], 16) for i in range(0, 8, 2))  # type: ignore[return-value]


def strip_background(im: Image.Image, key: tuple[int, int, int, int], tolerance: int) -> Image.Image:
    """Make a flat background transparent, flooding in from the edges.

    For art exported with a solid backdrop rather than an alpha channel.
    Flooding rather than keying every matching pixel: art tends to contain its
    own background colour somewhere inside it — a highlight, an eye — and those
    have to survive. The corners are used as seeds in case the mark touches an
    edge and splits the background in two.
    """
    out = im.copy()
    w, h = out.size
    for corner in {(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)}:
        if out.getpixel(corner)[3] == 0:
            continue  # already transparent here; nothing to flood
        if _close(out.getpixel(corner), key, tolerance):
            ImageDraw.floodfill(out, corner, (0, 0, 0, 0), thresh=tolerance)
    return out


def _close(a: tuple[int, ...], b: tuple[int, ...], tolerance: int) -> bool:
    return all(abs(int(x) - int(y)) <= tolerance for x, y in zip(a[:3], b[:3]))


def frame(mark: Image.Image, size: int, padding: float) -> Image.Image:
    """Scale `mark` into a `size` square, with `padding` left around it.

    The mark is cropped to its own ink first, so what the picture happens to
    leave on the canvas does not decide where the icon sits inside the square.
    """
    room = max(1, size - 2 * round(size * padding))
    scale = min(room / mark.width, room / mark.height)
    scaled_mark = mark.resize(
        (max(1, round(mark.width * scale)), max(1, round(mark.height * scale))),
        Image.LANCZOS,
    )
    out = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    out.alpha_composite(
        scaled_mark,
        ((size - scaled_mark.width) // 2, (size - scaled_mark.height) // 2),
    )
    return out


def write_ico(path: Path, frames: list[tuple[int, Image.Image]]) -> None:
    """Write a multi-size ICO holding the frames as PNGs.

    Pillow can write an ICO, but it fills the sizes it is given by resizing the
    one image it was handed — which, after the first size is written, is a
    downscale of a downscale. Each size here comes from the master, so the
    container is assembled by hand: it is a small format, a header, one 16-byte
    directory entry per image, then the payloads.
    """
    payloads = []
    for size, im in frames:
        buf = io.BytesIO()
        im.save(buf, format="PNG", optimize=True)
        payloads.append((size, buf.getvalue()))

    out = bytearray(struct.pack("<HHH", 0, 1, len(payloads)))
    offset = 6 + 16 * len(payloads)
    for size, data in payloads:
        # 256 is written as 0: the field is one byte.
        dimension = 0 if size >= 256 else size
        out += struct.pack("<BBBBHHII", dimension, dimension, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    for _, data in payloads:
        out += data
    path.write_bytes(bytes(out))


def preview(frames: list[tuple[int, Image.Image]], path: Path) -> None:
    """Render the sizes on a light and a dark background, side by side.

    What an icon has to survive is the surface it lands on, and a transparent
    PNG on its own does not say whether it does.
    """
    by_size = dict(frames)
    sizes = sorted(by_size, reverse=True)
    cell = max(sizes) + 16  # 1:1, so the small sizes are judged as drawn
    sheet = Image.new("RGB", (8 + cell * len(sizes), 16 + cell * 2), (128, 128, 128))
    for row, background in enumerate((255, 24)):
        for col, size in enumerate(sizes):
            tile = Image.new("RGBA", (cell, cell), (background, background, background, 255))
            tile.alpha_composite(by_size[size], ((cell - size) // 2, (cell - size) // 2))
            sheet.paste(tile.convert("RGB"), (8 + col * cell, 8 + row * cell))
    sheet.save(path)


def main() -> int:
    repo = Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", type=Path, default=repo / "doc/img/logo.png", help="master logo")
    ap.add_argument("--out-dir", type=Path, default=repo / "assets/icon", help="where the icon files go")
    ap.add_argument("--padding", type=float, default=0.04, help="transparent margin around the mark, as a fraction of the icon (default: 0.04)")
    ap.add_argument("--key", type=parse_color, metavar="COLOR", help="make this flat background colour transparent first")
    ap.add_argument("--key-tolerance", type=int, default=8, help="flood tolerance for --key (default: 8)")
    ap.add_argument("--sizes", default=",".join(str(s) for s in ICO_SIZES), help="comma-separated ICO sizes")
    ap.add_argument("--preview", type=Path, help="also write a PNG comparing the sizes on light and dark")
    args = ap.parse_args()

    if not args.source.is_file():
        sys.exit(f"no such logo: {args.source}")

    src = Image.open(args.source).convert("RGBA")
    if args.key:
        src = strip_background(src, args.key, args.key_tolerance)

    bbox = src.getchannel("A").point(lambda v: 255 if v > INK else 0).getbbox()
    if bbox is None:
        sys.exit(f"{args.source} has no visible ink")
    mark = src.crop(bbox)

    sizes = [int(s) for s in args.sizes.split(",") if s.strip()]
    frames = [(size, frame(mark, size, args.padding)) for size in sizes]

    args.out_dir.mkdir(parents=True, exist_ok=True)
    biggest = max(frames, key=lambda f: f[0])
    png_path = args.out_dir / "icon.png"
    biggest[1].save(png_path, optimize=True)
    ico_path = args.out_dir / "icon.ico"
    write_ico(ico_path, frames)

    print(f"{args.source.name} ({src.width}x{src.height}, ink {mark.width}x{mark.height})")
    print(f"  -> {png_path.relative_to(repo)} ({biggest[0]}px)")
    print(f"  -> {ico_path.relative_to(repo)} ({', '.join(str(s) for s in sizes)})")
    if args.preview:
        preview(frames, args.preview)
        print(f"  -> {args.preview} (light and dark)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
