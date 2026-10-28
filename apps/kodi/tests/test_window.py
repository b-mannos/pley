import json
import os
import shutil
import tempfile
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

    def setLabel(self, text, font=None, textColor=None):
        self.label = text
        self.text_color = textColor

    def setColorDiffuse(self, color):
        self.diffuse = color

    def setVisible(self, shown):
        self.visible = shown

    def setPosition(self, x, y):
        self.position = (x, y)

    def setHeight(self, height):
        self.height = height

    def reset(self):
        self.items = []

    def addItems(self, items):
        self.items += items

    def getSelectedPosition(self):
        return getattr(self, "selected", 0)

    def selectItem(self, at):
        self.selected = at

    def setImage(self, path, use_cache=True):
        self.image = path


class FakeWindow(window.Window):
    """The window's drawing, on controls that only remember."""

    def __init__(self):
        super().__init__()
        self.session = window.Session.__new__(window.Session)
        self.session.base = BASE
        self.made = {}
        self.toasts = []
        self.focused = None
        self.opened = []

    def getControl(self, control_id):
        return self.made.setdefault(control_id, Control())

    def toast(self, text, seconds=3.5):
        self.toasts.append(text)

    def setFocusId(self, control_id):
        self.focused = control_id

    def getFocusId(self):
        return self.focused or 0

    def setProperty(self, name, value):
        pass

    def open(self, screen):
        self.opened.append(screen)


class Client:
    """The server, as far as these tests ask it: posts are noted."""

    def __init__(self, answers=None):
        self.answers = answers or {}
        self.posts = []

    def get(self, path, **query):
        return self.answers[path]

    def post(self, path, body=None):
        self.posts.append((path, body))
        return {"id": 77, "title": (body or {}).get("title")}


class Dialogs:
    """xbmcgui.Dialog, answering as the test says: `picks` for select,
    `typed` for input, `yes` for yesno; what select offered is kept in
    `shown`, what yesno asked in `asked`."""

    def __init__(self, picks=(), typed=(), yes=()):
        self.picks, self.typed, self.yes, self.shown, self.asked = list(picks), list(typed), list(yes), [], []

    def __enter__(self):
        self.real = window.xbmcgui.Dialog
        outer = self

        class Dialog:
            def select(self, heading, entries, preselect=-1):
                outer.shown.append(([getattr(e, "label", e) for e in entries], preselect))
                return outer.picks.pop(0)

            def input(self, heading, *args, **kwargs):
                return outer.typed.pop(0)

            def contextmenu(self, entries):
                outer.shown.append((list(entries), -1))
                return outer.picks.pop(0)

            def yesno(self, heading, message, nolabel="", yeslabel="", autoclose=0):
                outer.asked.append((heading, message, nolabel, yeslabel))
                return outer.yes.pop(0)

        window.xbmcgui.Dialog = Dialog
        return self

    def __exit__(self, *exc):
        window.xbmcgui.Dialog = self.real


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

    def test_adding_to_a_playlist(self):
        win = FakeWindow()
        playlists = [
            {"id": 1, "title": "Road trip", "kind": "video", "item_count": 3},
            {"id": 2, "title": "Comedies", "kind": "video", "smart_filter": "genre=Comedy"},
            {"id": 3, "title": "Running", "kind": "audio", "item_count": 9},
        ]
        win.session.client = Client({"/api/playlists": {"playlists": playlists}})
        win.session.get = win.session.client.get
        with Dialogs(picks=[1]) as asked:
            win.add_to_playlist({"id": 5, "kind": "movie"})
        # A new one first, then the profile's own of the kind: smart ones fill themselves.
        self.assertEqual(asked.shown[0][0], [window.T("new_video_playlist"), "Road trip"])
        self.assertEqual(win.session.client.posts, [("/api/playlists/1/items", {"item_ids": [5]})])
        self.assertEqual(win.toasts, [window.T("added_to") % "Road trip"])
        with Dialogs(picks=[0], typed=[" Late albums "]) as asked:
            win.add_to_playlist({"id": 9, "kind": "album"})
        self.assertEqual(asked.shown[0][0], [window.T("new_music_playlist"), "Running"])
        self.assertEqual(win.session.client.posts[-1], ("/api/playlists", {"title": "Late albums", "kind": "audio", "item_ids": [9]}))
        # Backing out of naming it adds nothing.
        with Dialogs(picks=[0], typed=[""]):
            win.add_to_playlist({"id": 9, "kind": "album"})
        self.assertEqual(len(win.session.client.posts), 2)

    def test_library_filters(self):
        win = FakeWindow()
        win.libraries = [{"id": 4, "kind": "movies", "name": "Movies", "item_count": 342}]
        library = window.Library(win, win.libraries[0], None)
        library.items, library.total = [], 23
        library.genre, library.watched = "Science Fiction", False
        library.sort, library.descending = "year", True
        self.assertEqual(library.query(0)["watched"], "false")
        # Save as…: Plex's syntax, as SmartFilter.of writes it.
        self.assertEqual(library.smart_filter(), "genre=Science+Fiction&unwatched=1&sort=year%3Adesc")
        library.draw()
        # A filter that is on shows its twin, which leads with a check.
        self.assertFalse(win.made[L.LIBRARY_GENRE].visible)
        self.assertTrue(win.made[L.LIBRARY_GENRE_ON].visible)
        self.assertTrue(win.made[L.LIBRARY_GENRE_ON].label.endswith("Science Fiction"))
        self.assertTrue(win.made[L.LIBRARY_YEAR].visible)
        self.assertFalse(win.made[L.LIBRARY_YEAR_ON].visible)
        self.assertEqual(win.made[L.LIBRARY_COUNT].label, window.T("count_of") % ("23", "342 movies"))
        # Clear filters, from the Filter menu (All, Unwatched, Watched, Genre, Year, Clear, Save as).
        library.reload = lambda: None
        with Dialogs(picks=[5]) as asked:
            library.filter_menu()
        self.assertEqual(asked.shown[0][1], 1)
        self.assertEqual(len(asked.shown[0][0]), 7)
        self.assertFalse(library.filtered)
        self.assertEqual(library.smart_filter(), "sort=year%3Adesc")
        # Watched, then Save as… makes the smart playlist and opens it.
        with Dialogs(picks=[2]):
            library.filter_menu()
        self.assertIs(library.watched, True)
        win.session.client = Client()
        with Dialogs(picks=[6], typed=["Seen"]):
            library.filter_menu()
        self.assertEqual(win.session.client.posts, [("/api/playlists", {"title": "Seen", "smart_filter": "unwatched=0&sort=year%3Adesc", "library_id": 4})])
        self.assertEqual(len(win.opened), 1)

    def test_a_library_opens_on_the_tab_it_was_left_on(self):
        settings = window.kodi.xbmcaddon.settings
        profile = tempfile.mkdtemp()
        self.addCleanup(shutil.rmtree, profile, True)
        before = dict(settings)
        self.addCleanup(lambda: (settings.clear(), settings.update(before)))
        settings.update({"_profile": profile, "server_id": "srv-1"})
        movies, shows = {"id": 4, "kind": "movies", "name": "Movies"}, {"id": 5, "kind": "shows", "name": "TV Shows"}
        music = {"id": 6, "kind": "music", "name": "Music"}

        def opened(library, me=7):
            win = FakeWindow()
            win.libraries, win.me = [movies, shows, music], {} if me is None else {"id": me}
            page = window.Library(win, library, None)
            page.load = lambda: None
            page.show = lambda filled: None
            return page

        # Library the first time.
        self.assertEqual(opened(movies).tab, "all")
        opened(movies).switch("recommended")
        self.assertEqual(opened(movies).tab, "recommended")
        # Each library keeps its own, and each profile and server theirs.
        self.assertEqual(opened(shows).tab, "all")
        self.assertEqual(opened(movies, me=8).tab, "all")
        settings["server_id"] = "srv-2"
        self.assertEqual(opened(movies).tab, "all")
        settings["server_id"] = "srv-1"
        opened(movies).switch("collections")
        self.assertEqual(opened(movies).tab, "collections")
        # Music has no Collections: Library instead.
        opened(music).switch("recommended")
        self.assertEqual(opened(music).tab, "recommended")
        with open(os.path.join(profile, window.kodi.REMEMBERED), encoding="utf-8") as f:
            kept = json.load(f)
        kept["library_tab/srv-1/7/6"] = "collections"
        with open(os.path.join(profile, window.kodi.REMEMBERED), "w", encoding="utf-8") as f:
            json.dump(kept, f)
        self.assertEqual(opened(music).tab, "all")
        # Without a profile, nothing is read or kept.
        self.assertEqual(opened(movies, me=None).tab, "all")
        opened(movies, me=None).switch("recommended")
        opened(movies).switch("all")
        self.assertEqual(opened(movies).tab, "all")
        # Kept in the add-on's data folder, so a restarted Kodi has it.
        with open(os.path.join(profile, window.kodi.REMEMBERED), encoding="utf-8") as f:
            self.assertEqual(json.load(f), {"library_tab/srv-1/7/4": "all", "library_tab/srv-1/7/6": "collections"})
        # A file that isn't one opens on Library.
        with open(os.path.join(profile, window.kodi.REMEMBERED), "w", encoding="utf-8") as f:
            f.write("{")
        self.assertEqual(opened(movies).tab, "all")
        opened(movies).switch("recommended")
        self.assertEqual(opened(movies).tab, "recommended")

    def test_sorting_the_libraries(self):
        movies, shows, music = ({"id": 1, "name": "Movies", "kind": "movies"}, {"id": 2, "name": "TV Shows", "kind": "shows"}, {"id": 3, "name": "Music", "kind": "music"})
        every = {lib["id"]: lib for lib in (movies, shows, music)}

        class Server:
            """PUT and DELETE /api/me/libraries/order, noted; the answer is
            the libraries in their new order."""

            def __init__(self, missing=False):
                self.missing, self.calls = missing, []

            def put(self, path, body=None):
                self.calls.append(("PUT", path, body))
                if self.missing:
                    raise window.api.ApiError(404, "not_found", "no such route")
                return [every[i] for i in body["ids"]]

            def delete(self, path):
                self.calls.append(("DELETE", path, None))
                return [movies, shows, music]

        win = FakeWindow()
        win.libraries = [movies, shows, music]
        win.session.client = Server()
        shows_page = window.Library(win, shows, None)
        self.assertEqual(shows_page.rail, "lib1")
        win.screen = shows_page
        # Sort libraries: TV Shows, then First; Back when done.
        with Dialogs(picks=[1, 0, -1]) as asked:
            win.sort_libraries()
        self.assertEqual(asked.shown[0][0], ["Movies", "TV Shows", "Music", window.T("server_order")])
        self.assertEqual(asked.shown[1], ([window.T("first"), window.T("after") % "Movies", window.T("after") % "Music"], 1))
        self.assertEqual(win.session.client.calls, [("PUT", "/api/me/libraries/order", {"ids": [2, 1, 3]})])
        self.assertEqual([lib["id"] for lib in win.libraries], [2, 1, 3])
        self.assertEqual(win.made[L.rail_labels(L.RAIL_LIBRARIES[0])[0]].label, "TV Shows")
        # The rail's mark stays on the library open.
        self.assertEqual(shows_page.rail, "lib0")
        # A rail library's menu: the first has no Move up.
        with Dialogs(picks=[0]) as asked:
            win.library_menu(L.RAIL_LIBRARIES[0])
        self.assertEqual(asked.shown[0][0], [window.T("move_down"), window.T("server_order")])
        self.assertEqual(win.session.client.calls[-1][2], {"ids": [1, 2, 3]})
        self.assertEqual(win.focused, L.RAIL_LIBRARIES[1])
        with Dialogs(picks=[2]):
            win.library_menu(L.RAIL_LIBRARIES[1])
        self.assertEqual(win.session.client.calls[-1], ("DELETE", "/api/me/libraries/order", None))
        # An older server has no such route: it says so.
        win.session.client = Server(missing=True)
        with Dialogs(picks=[0]):
            win.library_menu(L.RAIL_LIBRARIES[0])
        self.assertEqual(win.toasts[-1], window.T("sort_needs_newer_server"))
        self.assertEqual([lib["id"] for lib in win.libraries], [1, 2, 3])
        # Sorted on another device: libraries_changed refreshes the window,
        # and the rail follows.
        win.session.get = lambda path, **query: [music, movies, shows]
        win.session.details = {}
        win.session.lock = window.threading.Lock()
        win.screen = shows_page
        # The page redrawn takes focus; someone in the rail stays there.
        shows_page.refresh = lambda: win.setFocusId(L.LIBRARY_SORT)
        win.setFocusId(L.RAIL_SEARCH)
        win.changed()
        self.assertEqual(win.made[L.rail_labels(L.RAIL_LIBRARIES[0])[0]].label, "Music")
        self.assertEqual(shows_page.rail, "lib2")
        self.assertEqual(win.focused, L.RAIL_SEARCH)

    def test_signing_out_asks_first(self):
        win = FakeWindow()
        win.libraries = []
        left = []
        win.leave_for = left.append
        window.kodi.xbmcaddon.settings["server_name"] = "Tokyo"
        self.addCleanup(window.kodi.xbmcaddon.settings.pop, "server_name", None)
        # The profile's menu: Choose a server, Audio and subtitles, Sign out.
        with Dialogs(picks=[2], yes=[False]) as asked:
            win.account()
        # Short: the question, the server, and Sign out to say yes.
        self.assertEqual(asked.asked, [("Sign out?", "Kodi signs out of Tokyo.", window.T("cancel"), window.T("sign_out"))])
        self.assertEqual(left, [])
        with Dialogs(picks=[2], yes=[True]):
            win.account()
        self.assertEqual(left, ["sign_out"])

    def test_the_rail_fits_every_library_above_settings(self):
        for count in range(8):
            win = FakeWindow()
            win.libraries = [{"id": i, "name": "Library %d" % i, "kind": "movies"} for i in range(count)]
            win.setup_rail()
            # As on Android TV (RailItems.kt): three libraries, then Libraries in Collections'
            # place; no Collections without a library.
            more = count > 3
            if more:
                entries = [L.RAIL_SEARCH, L.RAIL_HOME, *L.RAIL_LIBRARIES, L.RAIL_ALL_LIBRARIES, L.RAIL_PLAYLISTS, L.RAIL_SETTINGS]
            else:
                entries = [L.RAIL_SEARCH, L.RAIL_HOME, *L.RAIL_LIBRARIES[:count], L.RAIL_PLAYLISTS] + ([L.RAIL_COLLECTIONS] if count else []) + [L.RAIL_SETTINGS]
            for i, entry in enumerate(L.RAIL_LIBRARIES):
                self.assertEqual(win.made[L.rail_group(entry)].visible, i < count)
            self.assertEqual(win.made[L.rail_group(L.RAIL_ALL_LIBRARIES)].visible, more)
            self.assertEqual(win.made[L.rail_group(L.RAIL_COLLECTIONS)].visible, 0 < count <= 3)
            # Each pill's top and bottom on the screen.
            pills = []
            for entry in entries:
                group, pill = win.made[L.rail_group(entry)], win.made[entry]
                top = group.position[1] + pill.position[1]
                pills.append((top, top + pill.height))
                self.assertEqual(win.made[L.rail_pill(entry)].height, pill.height)
            for (_, above), (below, _) in zip(pills, pills[1:]):
                self.assertGreaterEqual(below - above, L.RAIL_GAP, "%d libraries: %s" % (count, pills))
            # Settings stays where skin.py puts it, its pill as tall as the others' (a 100 one
            # under pills of 64 looked out of place).
            self.assertEqual(win.made[L.rail_group(L.RAIL_SETTINGS)].position, (0, L.RAIL_SETTINGS_Y - 50))
            self.assertEqual(len({bottom - top for top, bottom in pills}), 1, pills)
            # The icons (40 tall) still fit their pills.
            self.assertGreaterEqual(min(bottom - top for top, bottom in pills), 60, count)
            # Few libraries keep the rail as it was.
            if count <= 2:
                self.assertEqual(win.made[L.rail_group(L.RAIL_PLAYLISTS)].position[1], L.RAIL_TOP + (2 + count) * L.RAIL_PITCH - 50)

    def test_the_libraries_page(self):
        # Five libraries: the rail has three, and Libraries all of them (TvLibrariesScreen).
        listed = [
            {"id": 1, "name": "Movies", "kind": "movies", "item_count": 1234},
            {"id": 2, "name": "TV Shows", "kind": "shows", "item_count": 1},
            {"id": 3, "name": "Music", "kind": "music", "item_count": 40},
            {"id": 4, "name": "Documentaries", "kind": "movies", "item_count": 12, "scanning": True},
            {"id": 5, "name": "Anime", "kind": "shows"},
        ]

        class Server:
            """GET /api/libraries, PUT and DELETE /api/me/libraries/order."""

            def __init__(self):
                self.listed, self.orders = list(listed), []

            def get(self, path, **query):
                return list(self.listed)

            def put(self, path, body=None):
                self.orders.append(body["ids"])
                self.listed = [next(lib for lib in listed if lib["id"] == i) for i in body["ids"]]
                return list(self.listed)

            def delete(self, path):
                self.orders.append(None)
                self.listed = list(listed)
                return list(self.listed)

        server = Server()
        win = FakeWindow()
        win.session.client = server
        win.session.get = server.get
        win.libraries = []
        page = window.Libraries(win)
        page.load()
        self.assertEqual(page.rail, "libraries")
        self.assertTrue(win.made[L.rail_group(L.RAIL_ALL_LIBRARIES)].visible)
        win.screen = page
        page.show(False)
        cards = win.made[L.LIBRARIES_LIST]
        self.assertEqual([(c.label, c.label2) for c in cards.items], [
            ("Movies", "1,234 movies"),
            ("TV Shows", "1 show"),
            ("Music", "40 artists"),
            ("Documentaries", "12 movies · " + window.T("scanning_now")),
            ("Anime", ""),
        ])
        self.assertEqual(cards.items[2].properties["icon"], "icon-music.png")
        self.assertEqual(win.focused, L.LIBRARIES_LIST)
        # A card opens its library; past the rail's three, its page marks Libraries.
        cards.selectItem(3)
        page.click(L.LIBRARIES_LIST)
        self.assertEqual((win.opened[-1].library["name"], win.opened[-1].rail), ("Documentaries", "libraries"))
        cards.selectItem(1)
        page.click(L.LIBRARIES_LIST)
        self.assertEqual(win.opened[-1].rail, "lib1")
        # A card's menu: Open, Move up, Move down, the server's order. Focus goes with the card.
        with Dialogs(picks=[2]) as asked:
            page.menu(L.LIBRARIES_LIST)
        self.assertEqual(asked.shown[0][0], [window.T("open"), window.T("move_up"), window.T("move_down"), window.T("server_order")])
        self.assertEqual(server.orders[-1], [1, 3, 2, 4, 5])
        self.assertEqual([c.label for c in cards.items[:3]], ["Movies", "Music", "TV Shows"])
        self.assertEqual(cards.getSelectedPosition(), 2)
        self.assertEqual(win.made[L.rail_labels(L.RAIL_LIBRARIES[1])[0]].label, "Music")
        # The first card has no Move up.
        cards.selectItem(0)
        with Dialogs(picks=[2]) as asked:
            page.menu(L.LIBRARIES_LIST)
        self.assertEqual(asked.shown[0][0], [window.T("open"), window.T("move_down"), window.T("server_order")])
        self.assertIsNone(server.orders[-1])
        # Sort libraries, as in the profile's menu.
        with Dialogs(picks=[-1]) as asked:
            page.click(L.LIBRARIES_SORT)
        self.assertEqual(asked.shown[0][0], [lib["name"] for lib in listed] + [window.T("server_order")])
        # The rail's third library moved down goes to Libraries, and focus with it.
        with Dialogs(picks=[1]):
            win.library_menu(L.RAIL_LIBRARIES[2])
        self.assertEqual(server.orders[-1], [1, 2, 4, 3, 5])
        self.assertEqual(win.focused, L.RAIL_ALL_LIBRARIES)
        self.assertEqual(cards.items[3].label, "Music")
        # A scan's counts reach the page; the rail stays as it is.
        win.session.details = {}
        win.session.lock = window.threading.Lock()
        server.listed = [dict(server.listed[0], item_count=1235)] + server.listed[1:]
        win.changed()
        self.assertEqual(cards.items[0].label2, "1,235 movies")
        # Down to three libraries: Collections is back, Libraries gone, and the page marks Home.
        server.listed = server.listed[:3]
        win.changed()
        self.assertEqual(len(cards.items), 3)
        self.assertEqual(page.rail, "home")
        self.assertFalse(win.made[L.rail_group(L.RAIL_ALL_LIBRARIES)].visible)
        self.assertTrue(win.made[L.rail_group(L.RAIL_COLLECTIONS)].visible)
        # Focus never goes to the entry the rail doesn't show.
        self.assertEqual((win.rail_entry("libraries"), win.rail_entry("collections")), (L.RAIL_HOME, L.RAIL_COLLECTIONS))
        win.libraries = listed
        self.assertEqual((win.rail_entry("libraries"), win.rail_entry("collections")), (L.RAIL_ALL_LIBRARIES, L.RAIL_HOME))

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
