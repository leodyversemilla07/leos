#!/usr/bin/env python3
"""Generates the Leos launcher icons.

The icons are drawn here rather than committed as binaries so that they can
be regenerated and reviewed as code:

    python3 android/tools/make_icons.py
"""

import os
import struct
import zlib

# The brand green of the wallet.
BACKGROUND = (11, 110, 79)
FOREGROUND = (255, 255, 255)

# Android launcher icon densities.
DENSITIES = {
    "mipmap-mdpi": 48,
    "mipmap-hdpi": 72,
    "mipmap-xhdpi": 96,
    "mipmap-xxhdpi": 144,
    "mipmap-xxxhdpi": 192,
}


def rounded_rect(x, y, size, radius):
    """Whether a pixel is inside a rounded square."""
    inset = size * 0.06
    left, top = inset, inset
    right, bottom = size - inset, size - inset

    if left <= x <= right and top <= y <= bottom:
        # Inside the square, but the corners are rounded.
        cx = min(max(x, left + radius), right - radius)
        cy = min(max(y, top + radius), bottom - radius)
        return (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2
    return False


def glyph(x, y, size):
    """Whether a pixel is part of the L monogram."""
    unit = size / 16.0

    # The vertical stroke of the L.
    if 4.6 * unit <= x <= 6.6 * unit and 3.4 * unit <= y <= 12.4 * unit:
        return True

    # The horizontal stroke of the L.
    if 4.6 * unit <= x <= 11.4 * unit and 10.4 * unit <= y <= 12.4 * unit:
        return True

    # The dot that turns the L into a peso sign.
    if (x - 8.6 * unit) ** 2 + (y - 5.2 * unit) ** 2 <= (1.5 * unit) ** 2:
        return True

    # The diagonal that turns the L into a leon.
    if 8.0 * unit <= x <= 9.0 * unit and 3.4 * unit <= y <= 5.4 * unit:
        return True

    return False


def render(size):
    """Renders the icon as raw RGBA rows."""
    rows = []
    radius = size * 0.24
    for y in range(size):
        row = bytearray()
        for x in range(size):
            if not rounded_rect(x + 0.5, y + 0.5, size, radius):
                row += bytes((0, 0, 0, 0))
            elif glyph(x + 0.5, y + 0.5, size):
                row += bytes((*FOREGROUND, 255))
            else:
                row += bytes((*BACKGROUND, 255))
        rows.append(bytes(row))
    return rows


def write_png(path, rows, size):
    """Writes the rows out as a PNG."""
    raw = b"".join(b"\x00" + row for row in rows)

    def chunk(kind, data):
        body = kind + data
        return (
            struct.pack(">I", len(data))
            + body
            + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )

    with open(path, "wb") as handle:
        handle.write(png)


def main():
    root = os.path.join(os.path.dirname(__file__), "..", "app", "src", "main", "res")
    for folder, size in DENSITIES.items():
        directory = os.path.join(root, folder)
        os.makedirs(directory, exist_ok=True)
        path = os.path.join(directory, "ic_launcher.png")
        write_png(path, render(size), size)
        print(f"wrote {path} ({size}x{size})")

    # The round icon used by some launchers.
    directory = os.path.join(root, "mipmap-xxxhdpi")
    path = os.path.join(directory, "ic_launcher_round.png")
    write_png(path, render(DENSITIES["mipmap-xxxhdpi"]), DENSITIES["mipmap-xxxhdpi"])
    print(f"wrote {path}")


if __name__ == "__main__":
    main()
