"""Draws the watched mark of tiles into images/watched.png, and the focus
of buttons (images/button-focus.9.png, and where it was as a button group
lost it, images/button-footprint.9.png): nine-patch pictures, a rounded
box in the app's accent or its raised surface, as its lists have. The
Live TV guide's programmes sit on images/guide-cell.9.png, and what has
aired on images/guide-past.9.png; images/spinner.png turns while a channel
tunes, images/round.png makes a person's photo round (and, tinted,
draws a profile's disc), and images/globe.png is the globe that picks
the language before signing in, drawn in white for the screens to tint.

The shapes of Android TV's look, in white for a Poster's blendColor to
tint: images/pill.9.png, a button's or a rail entry's rounded box (its
corners Android's 6 dp, 12 pixels), and images/pill-edge.9.png its edge
at rest; images/tile.9.png a tile's well (4 dp corners), and
images/tile-corners.9.png the page's colour outside those corners, over
a poster, which rounds it; images/tile-ring.9.png the focus ring around
a tile (3 dp wide, as Android's PleyFocus, and 2 dp out).
images/dash.png and images/dash-long.png are the dashes beside Home's
hero's buttons (Android TV's 14 and 36 dp by 4, rounded), one per title.
images/scrim-left.png and images/scrim-bottom.png are the gradients over
a backdrop (Android TV's TvSurfaces.kt: from the left 100 %, 92 %, 85 %
then nothing; from the top nothing to all), and images/fade-right.png a
plain fade, each stretched to its place. images/poster-mask.png and
images/cover-mask.png round a title page's poster and an album's cover
(a MaskGroup's mask).

The icons and splash screens are pley's logo, rendered with the other
apps' icons by scripts/render-icons.mjs at the repository's root; the
Lucide icons by scripts/icons.mjs. Standard library only.

    python scripts/make_images.py
"""

import math
import os
import struct
import zlib

DARK = (0x16, 0x19, 0x1E)  # the app's background (components/shared/Ui.brs)
ACCENT = (0x38, 0xBD, 0xF8)  # the app's accent
SURFACE3 = (0x2E, 0x34, 0x40)  # its raised surface
SURFACE2 = (0x26, 0x2B, 0x35)  # a guide's programme
PAST = (0x1E, 0x22, 0x2A, 0x99)  # the guide's time gone, over the screen

# A button's box: its size inside the nine-patch's border, and its corners
# (Android's 6 dp for a control).
BOX = 32
RADIUS = 12

# The watched mark, drawn at this size on tiles (components/items/Tile.brs).
MARK_SIZE = 40

# The spinner a BusySpinner turns (components/screens/LivePlayerScreen).
SPINNER_SIZE = 64

# The mask of round pictures, a person's photo (components/screens/
# PersonScreen), drawn at this size.
ROUND_SIZE = 160

WHITE = (0xFF, 0xFF, 0xFF)

# Android TV's corners at 1080p: a control's 6 dp, a tile's 4 dp.
PILL_RADIUS = 12
TILE_RADIUS = 8

# The focus ring: 3 dp wide, as Android's, but 2 dp out from the tile
# where Android leaves 3, so that a tile grown with it fits the room a Roku
# list gives it (components/lib/Content.brs, contentTileCell).
RING_STROKE = 6
RING_REACH = 10
RING_BOX = 48

# The language's globe, drawn at this size beside its two letters
# (components/shared/LanguageGlobe).
GLOBE_SIZE = 28


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


def render_nine(box, edge, cover):
    """RGBA rows of a nine-patch (Roku's .9.png): box pixels square in a
    border of one pixel whose black marks say what stretches (top and
    left: all but edge pixels at either end). cover(x, y) is whether the
    point (x, y) of the box is drawn; color is drawn there."""
    size = box + 2
    samples = 4
    rows = []
    for row in range(size):
        line = bytearray()
        for col in range(size):
            if row in (0, size - 1) or col in (0, size - 1):
                stretch = max(edge, 1)
                marked = (row == 0 and stretch < col < size - 1 - stretch) or (col == 0 and stretch < row < size - 1 - stretch)
                line.extend((0, 0, 0, 255) if marked else (0, 0, 0, 0))
                continue
            inside = 0
            for sy in range(samples):
                for sx in range(samples):
                    if cover(col - 1 + (sx + 0.5) / samples, row - 1 + (sy + 0.5) / samples):
                        inside += 1
            line.extend(cover.color)
            line.append(round(cover.alpha * inside / (samples * samples)))
        rows.append(bytes(line))
    return rows


def past_corners(x, y, box, radius):
    """How far (x, y) is outside a rounded box box wide, its corners of
    radius: 0 or less inside it."""
    dx = max(radius - x, 0, x - (box - radius))
    dy = max(radius - y, 0, y - (box - radius))
    return math.hypot(dx, dy) - radius


def shape(test, color=(0xFF, 0xFF, 0xFF, 255)):
    """test as a cover for render_nine, of color (with its alpha)."""
    test.color = color[:3]
    test.alpha = color[3] if len(color) > 3 else 255
    return test


def render_box(color, radius=RADIUS, box=BOX):
    """A rounded box of color (with its alpha, if it has one)."""
    return render_nine(box, radius, shape(lambda x, y: past_corners(x, y, box, radius) <= 0, color))


def render_outline(radius, stroke, box):
    """The edge of a rounded box, stroke wide, in white."""
    inner = radius - stroke

    def test(x, y):
        if past_corners(x, y, box, radius) > 0:
            return False
        return past_corners(x - stroke, y - stroke, box - 2 * stroke, inner) > 0

    return render_nine(box, radius, shape(test))


def render_corners(radius, box):
    """White outside a rounded box's corners, clear within it: drawn over
    a picture in the colour around it, it rounds the picture's corners."""
    return render_nine(box, radius, shape(lambda x, y: past_corners(x, y, box, radius) > 0))


def render_rounded(width, height, radius):
    """RGBA rows of a white box width × height with corners of radius, its
    edge smoothed: a MaskGroup's mask that rounds a picture's corners, or a
    shape drawn at its own size."""
    samples = 4
    rows = []
    for row in range(height):
        line = bytearray()
        for col in range(width):
            inside = 0
            for sy in range(samples):
                for sx in range(samples):
                    x = col + (sx + 0.5) / samples
                    y = row + (sy + 0.5) / samples
                    dx = max(radius - x, 0, x - (width - radius))
                    dy = max(radius - y, 0, y - (height - radius))
                    if math.hypot(dx, dy) <= radius:
                        inside += 1
            line.extend(WHITE)
            line.append(round(255 * inside / (samples * samples)))
        rows.append(bytes(line))
    return rows


def render_gradient(length, stops, across):
    """RGBA rows of a white gradient length long and 4 wide, its alpha
    going through stops ([(at, alpha)], at from 0 to 1): left to right
    when across, else top to bottom."""

    def alpha_at(t):
        for (a, va), (b, vb) in zip(stops, stops[1:]):
            if a <= t <= b:
                return va + (vb - va) * (t - a) / (b - a)
        return stops[-1][1]

    values = [round(255 * alpha_at((i + 0.5) / length)) for i in range(length)]
    if across:
        line = bytearray()
        for value in values:
            line.extend(WHITE + (value,))
        return [bytes(line)] * 4
    return [bytes(WHITE + (value,)) * 4 for value in values]


def render_spinner(size):
    """RGBA rows of a ring in the accent, three quarters drawn, fading
    from its head to its tail."""
    samples = 4
    outer = 0.5
    inner = 0.38
    rows = []
    for row in range(size):
        line = bytearray()
        for col in range(size):
            cover = 0.0
            for sy in range(samples):
                for sx in range(samples):
                    x = (col + (sx + 0.5) / samples) / size - 0.5
                    y = (row + (sy + 0.5) / samples) / size - 0.5
                    r = math.hypot(x, y)
                    if inner <= r <= outer:
                        turn = (math.atan2(y, x) / (2 * math.pi)) % 1.0
                        if turn <= 0.75:
                            cover += 0.25 + 0.75 * turn / 0.75
            line.extend(ACCENT)
            line.append(round(255 * cover / (samples * samples)))
        rows.append(bytes(line))
    return rows


def render_disc(size):
    """RGBA rows of a white disc as wide as the picture, its edge smoothed:
    the mask that makes a picture round (a MaskGroup's maskUri)."""
    samples = 4
    rows = []
    for row in range(size):
        line = bytearray()
        for col in range(size):
            inside = 0
            for sy in range(samples):
                for sx in range(samples):
                    x = (col + (sx + 0.5) / samples) / size - 0.5
                    y = (row + (sy + 0.5) / samples) / size - 0.5
                    if math.hypot(x, y) <= 0.5:
                        inside += 1
            line.extend((255, 255, 255))
            line.append(round(255 * inside / (samples * samples)))
        rows.append(bytes(line))
    return rows


def render_globe(size):
    """RGBA rows of a plain globe in white lines: its outline, a meridian
    and the equator, as the other TV apps draw it."""
    samples = 4
    radius = 0.44
    meridian = 0.2
    half_stroke = 0.045
    rows = []
    for row in range(size):
        line = bytearray()
        for col in range(size):
            ink = 0
            for sy in range(samples):
                for sx in range(samples):
                    x = (col + (sx + 0.5) / samples) / size - 0.5
                    y = (row + (sy + 0.5) / samples) / size - 0.5
                    r = math.hypot(x, y)
                    outline = abs(r - radius) <= half_stroke
                    ellipse = abs(math.hypot(x / meridian, y / radius) - 1) * meridian <= half_stroke
                    equator = abs(y) <= half_stroke and r <= radius
                    if outline or (r <= radius and ellipse) or equator:
                        ink += 1
            line.extend((255, 255, 255))
            line.append(round(255 * ink / (samples * samples)))
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
    boxes = (
        ("button-focus.9.png", ACCENT, RADIUS),
        ("button-footprint.9.png", SURFACE3, RADIUS),
        ("guide-cell.9.png", SURFACE2, 6),
        ("guide-past.9.png", PAST, 0),
    )
    for name, color, radius in boxes:
        write_png(os.path.join(folder, name), BOX + 2, BOX + 2, render_box(color, radius))
        print("images/" + name, BOX + 2, "x", BOX + 2)
    write_png(os.path.join(folder, "spinner.png"), SPINNER_SIZE, SPINNER_SIZE, render_spinner(SPINNER_SIZE))
    print("images/spinner.png", SPINNER_SIZE, "x", SPINNER_SIZE)
    write_png(os.path.join(folder, "round.png"), ROUND_SIZE, ROUND_SIZE, render_disc(ROUND_SIZE))
    print("images/round.png", ROUND_SIZE, "x", ROUND_SIZE)
    shapes = (
        ("pill.9.png", BOX, render_box(WHITE, PILL_RADIUS)),
        ("pill-edge.9.png", BOX, render_outline(PILL_RADIUS, 2, BOX)),
        ("tile.9.png", BOX, render_box(WHITE, TILE_RADIUS)),
        ("tile-corners.9.png", BOX, render_corners(TILE_RADIUS, BOX)),
        ("tile-ring.9.png", RING_BOX, render_outline(TILE_RADIUS + RING_REACH, RING_STROKE, RING_BOX)),
    )
    for name, box, rows in shapes:
        write_png(os.path.join(folder, name), box + 2, box + 2, rows)
        print("images/" + name, box + 2, "x", box + 2)
    gradients = (
        # From the left over 800 dp: 100 % at 0, 92 % at 300, 85 % at 560, none at 800.
        ("scrim-left.png", 400, 4, render_gradient(400, [(0, 1), (0.375, 0.92), (0.7, 0.85), (1, 0)], True)),
        ("scrim-bottom.png", 4, 256, render_gradient(256, [(0, 0), (1, 1)], False)),
        ("fade-right.png", 256, 4, render_gradient(256, [(0, 1), (1, 0)], True)),
    )
    for name, width, height, rows in gradients:
        write_png(os.path.join(folder, name), width, height, rows)
        print("images/" + name, width, "x", height)
    # A title page's poster and an album's cover, with a tile's corners.
    for name, width, height in (("poster-mask.png", 420, 630), ("cover-mask.png", 420, 420)):
        write_png(os.path.join(folder, name), width, height, render_rounded(width, height, TILE_RADIUS))
        print("images/" + name, width, "x", height)
    # Home's hero's dashes, ends round.
    for name, width in (("dash.png", 28), ("dash-long.png", 72)):
        write_png(os.path.join(folder, name), width, 8, render_rounded(width, 8, 4))
        print("images/" + name, width, "x", 8)
    write_png(os.path.join(folder, "globe.png"), GLOBE_SIZE, GLOBE_SIZE, render_globe(GLOBE_SIZE))
    print("images/globe.png", GLOBE_SIZE, "x", GLOBE_SIZE)


if __name__ == "__main__":
    main()
