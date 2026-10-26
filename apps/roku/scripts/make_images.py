"""Draws the watched mark of tiles into images/watched.png. The icons and
splash screens are pley's logo, rendered with the other apps' icons by
scripts/render-icons.mjs at the repository's root. Standard library only.

    python scripts/make_images.py
"""

import math
import os
import struct
import zlib

DARK = (0x16, 0x19, 0x1E)  # the app's background (components/shared/Ui.brs)
ACCENT = (0x38, 0xBD, 0xF8)  # the app's accent

# The watched mark, drawn at this size on tiles (components/items/Tile.brs).
MARK_SIZE = 40


def segment_distance(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return math.hypot(px - (ax + t * dx), py - (ay + t * dy))


def render_mark(size):
    """RGBA rows of the watched mark: a check on an accent disc."""
    check = [(0.27, 0.52), (0.43, 0.68), (0.74, 0.36)]
    half_stroke = 0.065
    samples = 4
    rows = []
    for row in range(size):
        line = bytearray()
        for col in range(size):
            disc = ink = 0
            for sy in range(samples):
                for sx in range(samples):
                    x = (col + (sx + 0.5) / samples) / size
                    y = (row + (sy + 0.5) / samples) / size
                    if math.hypot(x - 0.5, y - 0.5) <= 0.5:
                        disc += 1
                        near = min(segment_distance(x, y, *check[i], *check[i + 1]) for i in range(len(check) - 1))
                        if near <= half_stroke:
                            ink += 1
            total = samples * samples
            cover = ink / disc if disc else 0
            for channel in range(3):
                line.append(round(ACCENT[channel] * (1 - cover) + DARK[channel] * cover))
            line.append(round(255 * disc / total))
        rows.append(bytes(line))
    return rows


def write_png(path, width, height, rows):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + row for row in rows)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as out:
        out.write(png)


def main():
    folder = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "images")
    os.makedirs(folder, exist_ok=True)
    write_png(os.path.join(folder, "watched.png"), MARK_SIZE, MARK_SIZE, render_mark(MARK_SIZE))
    print("images/watched.png", MARK_SIZE, "x", MARK_SIZE)


if __name__ == "__main__":
    main()
