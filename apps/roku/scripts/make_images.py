"""Draws the app's pictures into images/: the icons ("pley" in plain
geometric letters on the brand's light blue), the splash screens (the same
word in the app's blue on its dark background, so launching doesn't flash
light) and the watched mark of tiles. Standard library only.

    python scripts/make_images.py
"""

import math
import os
import struct
import zlib

BACKGROUND = (0xBA, 0xE6, 0xFD)  # brandLight
INK = (0x0A, 0x16, 0x22)  # brandTile
DARK = (0x16, 0x19, 0x1E)  # the app's background (components/shared/Ui.brs)
ACCENT = (0x38, 0xBD, 0xF8)  # the app's accent, its wordmark

# Letters in units of the x-height: baseline at 0, x-height at 1.
STROKE = 0.24
GAP = 0.2
ASCENDER = 1.42
DESCENDER = -0.5
HALF = STROKE / 2
RADIUS = 0.5 - HALF  # of a bowl's centre line


def capsule(px, py, ax, ay, bx, by):
    """Whether (px, py) is within a stroke from segment a-b (round ends)."""
    dx, dy = bx - ax, by - ay
    t = ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)
    t = max(0.0, min(1.0, t))
    cx, cy = ax + t * dx, ay + t * dy
    return (px - cx) ** 2 + (py - cy) ** 2 <= HALF * HALF


def ring(px, py, cx, cy):
    return abs(math.hypot(px - cx, py - cy) - RADIUS) <= HALF


def letter_p(x, y):
    return capsule(x, y, HALF, DESCENDER + HALF, HALF, 1 - HALF) or ring(x, y, 0.5, 0.5)


def letter_l(x, y):
    return capsule(x, y, HALF, HALF, HALF, ASCENDER - HALF)


E_OPENING = -52  # degrees: the ring stops here, below the bar


def letter_e(x, y):
    distance = math.hypot(x - 0.5, y - 0.5)
    # The bar, out to the ring's outer edge.
    if abs(y - 0.5) <= HALF and distance <= RADIUS + HALF:
        return True
    # The ring's round end.
    end_x = 0.5 + RADIUS * math.cos(math.radians(E_OPENING))
    end_y = 0.5 + RADIUS * math.sin(math.radians(E_OPENING))
    if (x - end_x) ** 2 + (y - end_y) ** 2 <= HALF * HALF:
        return True
    if not ring(x, y, 0.5, 0.5):
        return False
    # The opening at the lower right, under the bar.
    angle = math.degrees(math.atan2(y - 0.5, x - 0.5))
    return not (E_OPENING < angle < 0 and y < 0.5 - HALF)


def letter_y(x, y):
    # The left arm ends on the right one's centre line, its round end hidden.
    tail_x, tail_y = 0.2, DESCENDER + HALF
    t = (1 - HALF - 0.12) / (1 - HALF - tail_y)
    join_x = 1 - HALF + t * (tail_x - (1 - HALF))
    return capsule(x, y, HALF, 1 - HALF, join_x, 0.12) or capsule(x, y, 1 - HALF, 1 - HALF, tail_x, tail_y)


LETTERS = [(letter_p, 1.0), (letter_l, STROKE), (letter_e, 1.0), (letter_y, 1.0)]
WORD_WIDTH = sum(width for _, width in LETTERS) + GAP * (len(LETTERS) - 1)
WORD_HEIGHT = ASCENDER - DESCENDER


def inside(x, y):
    """Whether point (x, y), in word units from the left edge, is ink."""
    left = 0.0
    for draw, width in LETTERS:
        if left - HALF <= x <= left + width + HALF and draw(x - left, y):
            return True
        left += width + GAP
    return False


def render(width, height, word_width, background, ink):
    """RGB rows of an image with the word word_width pixels wide, centred."""
    scale = word_width / WORD_WIDTH
    word_height = WORD_HEIGHT * scale
    left = (width - word_width) / 2
    top = (height - word_height) / 2
    samples = 4
    rows = []
    for row in range(height):
        line = bytearray()
        for col in range(width):
            hits = 0
            near = left - scale <= col <= left + word_width + scale and top - scale <= row <= top + word_height + scale
            if near:
                for sy in range(samples):
                    for sx in range(samples):
                        x = (col + (sx + 0.5) / samples - left) / scale
                        y = ASCENDER - (row + (sy + 0.5) / samples - top) / scale
                        if inside(x, y):
                            hits += 1
            cover = hits / (samples * samples)
            for channel in range(3):
                line.append(round(background[channel] * (1 - cover) + ink[channel] * cover))
        rows.append(bytes(line))
    return rows


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


def write_png(path, width, height, rows, alpha=False):
    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = b"".join(b"\x00" + row for row in rows)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6 if alpha else 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as out:
        out.write(png)


# Roku's sizes (the channel manifest): focus icons and splash screens.
IMAGES = [
    ("icon_focus_fhd.png", 540, 405, 300, BACKGROUND, INK),
    ("icon_focus_hd.png", 290, 218, 162, BACKGROUND, INK),
    ("splash_fhd.png", 1920, 1080, 560, DARK, ACCENT),
    ("splash_hd.png", 1280, 720, 374, DARK, ACCENT),
    ("splash_sd.png", 720, 480, 240, DARK, ACCENT),
]

# The watched mark, drawn at this size on tiles (components/items/Tile.brs).
MARK_SIZE = 40


def main():
    folder = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "images")
    os.makedirs(folder, exist_ok=True)
    for name, width, height, word_width, background, ink in IMAGES:
        write_png(os.path.join(folder, name), width, height, render(width, height, word_width, background, ink))
        print("images/" + name, width, "x", height)
    write_png(os.path.join(folder, "watched.png"), MARK_SIZE, MARK_SIZE, render_mark(MARK_SIZE), alpha=True)
    print("images/watched.png", MARK_SIZE, "x", MARK_SIZE)


if __name__ == "__main__":
    main()
