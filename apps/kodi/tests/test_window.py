import unittest

import support  # noqa: F401
from pley import layout as L
from pley import window

BASE = "http://127.0.0.1:5000/s"
EPISODE = {
    "id": 31,
    "kind": "episode",
    "title": "Glasshouse",
    "index": 5,
    "parent_index": 2,
    "grandparent_title": "Severance",
    "duration_ms": 56 * 60_000,
    "year": 2025,
    "summary": "Mark gets a lead.",
    "user_state": {"position_ms": 30 * 60_000, "watched": False},
}


class Control:
    def __init__(self):
        self.label = None
        self.visible = True
        self.position = None
        self.items = []

    def setLabel(self, text):
        self.label = text

    def setVisible(self, shown):
        self.visible = shown

    def setPosition(self, x, y):
        self.position = (x, y)

    def reset(self):
        self.items = []

    def addItems(self, items):
        self.items += items

    def getSelectedPosition(self):
        return 0


class FakeWindow(window.Window):
    """The window's drawing, on controls that only remember."""

    def __init__(self):
        super().__init__()
        self.session = window.Session.__new__(window.Session)
        self.session.base = BASE
        self.made = {}

    def getControl(self, control_id):
        return self.made.setdefault(control_id, Control())


class WindowTest(unittest.TestCase):
    def test_text_as_wide_as_kodi_draws_it(self):
        # As the lab Kodi's screenshots measure them (Estuary at 1080p).
        line = "A burnt-out chef inherits a failing flower farm and a stubborn goat,"
        self.assertAlmostEqual(window.text_width(line, 30), 887, delta=9)
        self.assertAlmostEqual(window.text_width("Continue Watching", 36, bold=True), 350, delta=8)
        self.assertEqual(window.wrap(line + " and finds out she's better at one of them.", 30, L.TEXT_WIDTH)[0][-1], "goat,")
        self.assertEqual(window.lines("A storm keeps the fleet ashore, and old grudges come out at the pub.", 30, L.TEXT_WIDTH, 3), 1)
        self.assertEqual(window.advance("é"), window.advance("e"))
        self.assertEqual(window.lines("", 30, L.TEXT_WIDTH, 3), 0)

    def test_summaries_are_cut_at_a_word(self):
        text = " ".join(["lighthouse"] * 60)
        cut = window.clamp(text, 30, L.TEXT_WIDTH, 2)
        self.assertTrue(cut.endswith("…"))
        self.assertLessEqual(window.lines(cut, 30, L.TEXT_WIDTH, 9), 2)
        self.assertEqual(window.clamp("Short.", 30, L.TEXT_WIDTH, 2), "Short.")

    def test_what_a_hero_says_of_an_episode(self):
        info = window.about(EPISODE)
        self.assertEqual(info["title"], "Severance")
        self.assertEqual(info["context"], "S2 · E5 · Glasshouse")
        self.assertEqual(info["left"], "26 min left")
        self.assertTrue(info["progress"])
        self.assertEqual(info["summary"], "Mark gets a lead.")

    def test_the_metadata_line(self):
        detail = {"id": 31, "content_rating": "TV-MA", "files": [{"width": 1920, "height": 1080, "streams": []}]}
        self.assertEqual(
            window.meta_parts(EPISODE, detail, lead="S2 · E5"),
            [("lead", "S2 · E5"), ("text", "2025"), ("badge", "TV-MA"), ("text", "56 min"), ("quality", "1080p")],
        )
        win = FakeWindow()
        win.meta(L.HOME_INFO_META, [("text", "2025"), ("rating", "7.4"), ("quality", "4K"), ("quality", "HDR10")])
        dots = [win.made[L.meta_piece(L.HOME_INFO_META, i)[0]].visible for i in range(4)]
        # No dot first, nor between quality badges.
        self.assertEqual(dots, [False, True, True, False])
        self.assertTrue(win.made[L.meta_piece(L.HOME_INFO_META, 1)[1]].visible)
        self.assertEqual(win.made[L.meta_piece(L.HOME_INFO_META, 2)[3]].label, "4K")
        self.assertFalse(win.made[L.meta_piece(L.HOME_INFO_META, 5)[2]].visible)

    def test_credits(self):
        detail = {
            "cast": [{"id": 1, "name": "Idris Vale", "character": "Ash"}, {"id": 2, "name": "Maren Holt"}],
            "crew": [{"id": 3, "name": "Lena Okafor", "role": "director"}, {"id": 4, "name": "A. Producer", "role": "producer"}],
        }
        people = window.cast_and_crew(detail)
        self.assertEqual([p["name"] for p in people], ["Idris Vale", "Maren Holt", "Lena Okafor"])
        self.assertEqual(people[-1]["job"], "Director")
        line = window.credits_line(detail)
        self.assertIn("Lena Okafor", line)
        self.assertIn("Idris Vale, Maren Holt", line)

    def test_rows_fill_and_place(self):
        win = FakeWindow()
        rows = window.Rows(win, L.HOME_ROWS, L.HOME_SLOTS, ("poster", "landscape", "square"))
        movie = {"id": 5, "kind": "movie", "title": "Heat", "user_state": {}}
        rows.fill([
            {"key": "continue_watching", "title": "Continue Watching", "shape": "landscape", "items": [EPISODE]},
            {"key": "recently_added:1", "title": "Recently Added", "shape": "poster", "items": [movie], "see_all": 1},
        ])
        self.assertEqual(win.made[L.slot_header(L.HOME_ROWS, 0)].label, "Continue Watching")
        self.assertEqual(len(win.made[L.slot_list(L.HOME_ROWS, 0, "landscape")].items), 1)
        self.assertFalse(win.made[L.slot_list(L.HOME_ROWS, 0, "poster")].visible)
        # A library's row ends with See all.
        self.assertEqual(len(win.made[L.slot_list(L.HOME_ROWS, 1, "poster")].items), 2)
        self.assertFalse(win.made[L.slot(L.HOME_ROWS, 2)].visible)
        # Under the hero, the rows go down from where it ends…
        rows.place(None, 768)
        self.assertEqual(win.made[L.slot(L.HOME_ROWS, 0)].position, (0, 768))
        landscape = L.row_height(L.SHAPES["landscape"]) + L.ROW_GAP
        self.assertEqual(win.made[L.slot(L.HOME_ROWS, 1)].position, (0, 768 + landscape))
        # …and with a row focused, it sits under the info block, the ones above out of sight.
        rows.place(1, 0)
        self.assertEqual(win.made[L.slot(L.HOME_ROWS, 1)].position, (0, L.FOCUSED_ROW_Y))
        self.assertLess(win.made[L.slot(L.HOME_ROWS, 0)].position[1], 0)
        self.assertEqual(rows.find(L.slot_list(L.HOME_ROWS, 1, "poster"))[0], 1)
        self.assertIsNone(rows.find(L.slot_header(L.HOME_ROWS, 1)))


if __name__ == "__main__":
    unittest.main()
