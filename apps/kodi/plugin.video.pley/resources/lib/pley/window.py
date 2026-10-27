"""pley's own screens in Kodi, drawn as its Android TV app draws them
(apps/android/ui/tv): the rail, Home's hero and rows, an item's page,
libraries, search, playlists and collections. Kodi's player plays, with
the service's Skip Intro and Up Next.

One window (layout.WINDOW) holds every screen; this shows one at a time and
keeps the ones under it, so Back goes to them as they were. What the
screens show comes through the service's proxy, as the plugin's listings.
"""

from __future__ import annotations

import threading
import time
import unicodedata
from urllib.parse import quote_plus

import xbmc
import xbmcgui

from . import api, kodi, playback, tiles, words
from . import layout as L
from .listing import image

T = kodi.text

# Kodi's actions.
LEFT, RIGHT, UP, DOWN, PAGE_UP, PAGE_DOWN = 1, 2, 3, 4, 5, 6
PREVIOUS_MENU, NAV_BACK, CONTEXT_MENU, MENU, SHOW_INFO = 10, 92, 117, 163, 11
BACK = (PREVIOUS_MENU, NAV_BACK)
MOVES = (LEFT, RIGHT, UP, DOWN, PAGE_UP, PAGE_DOWN)

# Focus rests this long on a tile before the backdrop and the info block
# follow it (TvHome's rememberRested).
REST = 0.4
UI = kodi.WINDOW_OPEN
# Room for a pill's icon before its label (pill-*.png keep it unstretched).
ICON_ROOM = " " * 8

SORTS = ("title", "added", "year", "release", "rating", "duration", "last_played")
SORT_NAMES = {
    "title": "sort_title",
    "added": "sort_added",
    "year": "sort_year",
    "release": "sort_release",
    "rating": "sort_rating",
    "duration": "sort_duration",
    "last_played": "sort_last_played",
}
LIBRARY_ICONS = {"movies": "icon-clapperboard.png", "shows": "icon-tv.png", "music": "icon-music.png"}
PAGE = 120
# What a playlist takes (shows, seasons, artists and albums add their
# episodes or tracks), and of which kind.
PLAYLIST_KINDS = {"movie": "video", "show": "video", "season": "video", "episode": "video", "artist": "audio", "album": "audio", "track": "audio"}
# A library's sort as a smart filter says it (SmartFilter.kt's, the web's).
PLEX_SORTS = {
    "title": "titleSort",
    "year": "year",
    "added": "addedAt",
    "rating": "rating",
    "release": "originallyAvailableAt",
    "duration": "duration",
    "last_played": "lastViewedAt",
}
HERO_BUTTONS = (
    L.ITEM_PLAY,
    L.ITEM_FROM_START,
    L.ITEM_WATCHED,
    L.ITEM_UNWATCHED,
    L.ITEM_ADD_TO_LIST,
    L.ITEM_IN_LIST,
    L.ITEM_TRAILER,
    L.ITEM_ADD_TO_PLAYLIST,
    L.ITEM_MORE,
)


def color(text: str, argb: str) -> str:
    return "[COLOR %s]%s[/COLOR]" % (argb, text)


# Noto Sans's advance widths (Estuary's font) from " " to "~", in 2048ths of
# an em. Kodi draws them at 0.943 of that, and its bold 0.075 em wider a
# character: so its screenshots measure.
ADVANCES = (
    532, 550, 835, 1323, 1171, 1701, 1499, 460, 614, 614, 1128, 1171, 548, 659, 548, 761,
    1171, 1171, 1171, 1171, 1171, 1171, 1171, 1171, 1171, 1171, 548, 548, 1171, 1171, 1171, 888,
    1841, 1308, 1331, 1294, 1495, 1138, 1062, 1490, 1517, 694, 559, 1267, 1073, 1857, 1556, 1599,
    1239, 1599, 1273, 1124, 1138, 1497, 1228, 1904, 1200, 1159, 1171, 673, 761, 673, 1171, 909,
    575, 1148, 1259, 983, 1259, 1155, 704, 1259, 1265, 528, 528, 1093, 528, 1914, 1265, 1239,
    1259, 1259, 845, 980, 739, 1265, 1040, 1609, 1083, 1044, 962, 778, 1128, 778, 1171,
)
DRAWN, BOLDER = 0.943, 0.075


def advance(ch: str) -> int:
    """How far `ch` moves the pen in Noto Sans, in 2048ths of an em."""
    code = ord(ch)
    if 32 <= code < 127:
        return ADVANCES[code - 32]
    if ch == "…":
        return 1619
    if code >= 0x2E80:  # CJK, kana, hangul: square
        return 2048
    # An accented letter as wide as its letter, anything else as a digit.
    base = ord(unicodedata.normalize("NFD", ch)[0])
    return ADVANCES[base - 32] if 32 <= base < 127 else 1171


def text_width(text: str, size: int, bold: bool = False) -> float:
    """How wide Kodi draws `text` in Estuary's Noto Sans at `size` px."""
    em = sum(advance(ch) for ch in text) / 2048 * DRAWN
    if bold:
        em += BOLDER * len(text)
    return em * size


def wrap(text: str, size: int, width: int, bold: bool = False) -> list:
    """`text`'s words, line by line, as Kodi wraps them in a column `width` wide."""
    space = text_width(" ", size, bold)
    rows, line, used = [], [], 0.0
    for word in text.split():
        w = text_width(word, size, bold)
        if line and used + space + w > width:
            rows.append(line)
            line, used = [], 0.0
        used += (space if line else 0) + w
        line.append(word)
    if line:
        rows.append(line)
    return rows


def lines(text: str, size: int, width: int, most: int, bold: bool = False) -> int:
    """How many lines `text` takes in a column `width` wide, up to `most`."""
    return min(len(wrap(text or "", size, width, bold)), most)


def clamp(text: str, size: int, width: int, most: int) -> str:
    """`text` cut at a word with "…" where it runs past `most` lines (Kodi's
    text boxes stop at a line without saying more was there)."""
    text = " ".join((text or "").split())
    rows = wrap(text, size, width)
    if len(rows) <= most:
        return text
    last = " ".join(rows[most - 1])
    while last and text_width(last + "…", size) > width:
        last = last.rsplit(" ", 1)[0] if " " in last else last[:-1]
    kept = [" ".join(row) for row in rows[: most - 1]] + [last]
    return " ".join(kept).rstrip(" ,.;:") + "…"


# Work posted to the window's thread (Window.post) is announced by this.
POSTED = "pley.posted"


class Monitor(xbmc.Monitor):
    """Hears the service say what the server changed, and the window's own
    threads hand it work: Kodi calls this on the window's thread."""

    def __init__(self, window):
        super().__init__()
        self.window = window

    def onNotification(self, sender, method, data):
        if sender != kodi.ADDON_ID:
            return
        if method.endswith(POSTED):
            self.window.run_posted()
        elif method.endswith("changed"):
            self.window.changed()


class Session:
    """The server, through the service's proxy, and the details asked of it."""

    def __init__(self, base: str):
        self.base = base
        self.client = api.Client(base, kodi.user_agent(), timeout=60, local=True)
        self.details = {}
        self.lock = threading.Lock()

    def get(self, path: str, **query):
        return self.client.get(path, **query)

    def detail(self, item_id: int, fresh: bool = False) -> dict:
        with self.lock:
            found = None if fresh else self.details.get(item_id)
        if found is None:
            found = self.client.get("/api/items/%d" % item_id)
            with self.lock:
                if len(self.details) > 80:
                    self.details.clear()
                self.details[item_id] = found
        return found

    def cached(self, item_id: int):
        with self.lock:
            return self.details.get(item_id)

    def forget(self) -> None:
        with self.lock:
            self.details.clear()


class Window(xbmcgui.WindowXML):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.session = None
        self.plugin = None
        self.lock = threading.RLock()
        self.controls = {}
        self.stack = []
        self.screen = None
        self.ready = False
        self.libraries = []
        self.me = {}
        self.rest_token = 0
        self.rest_timer = None
        self.toast_timer = None
        self.posted = []
        self.posted_lock = threading.Lock()
        self.owner = {}
        self.monitor = None
        self.closing = False
        self.focus = None
        # What to do once the window is closed: "choose_server", "sign_in".
        self.after = None

    # ------------------------------------------------------------ controls

    def c(self, control_id: int):
        """A control, its Python object kept (Kodi makes a new one each ask)."""
        found = self.controls.get(control_id)
        if found is None:
            found = self.controls[control_id] = self.getControl(control_id)
        return found

    def visible(self, control_id: int, shown: bool) -> None:
        self.c(control_id).setVisible(shown)

    def put(self, control_id: int, text: str) -> None:
        self.c(control_id).setLabel(text or "")

    def place(self, control_id: int, x: int, y: int) -> None:
        self.c(control_id).setPosition(int(x), int(y))

    def text(self, control_id: int, text: str) -> None:
        box = self.c(control_id)
        box.reset()
        box.setText(text or "")

    def meta(self, line: int, parts: list) -> None:
        """Fills a metadata line with `parts`: ("text"|"lead"|"rating"|"badge"|
        "quality", text). Quality badges sit together, without dots."""
        for i in range(L.META_PIECES):
            dot, star, text, badge = L.meta_piece(line, i)
            if i < len(parts):
                kind, value = parts[i]
                together = kind == "quality" and parts[i - 1][0] == "quality"
                self.visible(dot, i > 0 and not together)
                self.visible(star, kind == "rating")
                if kind in ("badge", "quality"):
                    self.visible(text, False)
                    self.put(badge, value)
                    self.visible(badge, True)
                else:
                    self.put(text, color(value, L.FG) if kind in ("lead", "rating") else value)
                    self.visible(text, True)
                    self.visible(badge, False)
            else:
                for c in (dot, star, text, badge):
                    self.visible(c, False)

    def prop(self, name: str, value: str) -> None:
        self.setProperty(name, value or "")

    # ----------------------------------------------------------- lifecycle

    def onInit(self) -> None:
        with self.lock:
            if self.ready:
                # Back from playing, or a dialog: what changed shows.
                if self.screen is not None:
                    self.screen.resume()
                return
            self.ready = True
            kodi.set_property(UI, "1")
            self.monitor = Monitor(self)
            started = time.perf_counter()
            self.start()
            kodi.log("pley's window ready in %d ms" % ((time.perf_counter() - started) * 1000))

    def start(self) -> None:
        try:
            self.me, self.libraries = api.together(lambda: self.session.get("/api/auth/me"), lambda: self.session.get("/api/libraries"))
        except api.ApiError as e:
            return self.failed(e, Home(self))
        self.setup_rail()
        self.go(Home(self))

    def close(self) -> None:
        self.closing = True
        for timer in (self.rest_timer, self.toast_timer):
            if timer is not None:
                timer.cancel()
        kodi.clear_property(UI)
        super().close()

    def changed(self) -> None:
        """The server said something changed (the watch state, My List, a
        library): the screen shows it."""
        with self.lock:
            if self.closing or self.screen is None:
                return
            self.session.forget()
            try:
                self.screen.refresh()
            except api.ApiError as e:
                kodi.log("refreshing %s: %s" % (self.screen.name, e.message), xbmc.LOGWARNING)

    # ---------------------------------------------------------- navigation

    def go(self, screen) -> None:
        """Opens a page of the rail's: what was open goes."""
        self.stack = []
        self.screen = None
        self.open(screen)

    def open(self, screen) -> None:
        """Opens `screen` over the one showing."""
        try:
            screen.load()
        except api.ApiError as e:
            return self.failed(e, screen)
        instead = screen.instead()
        if instead is not None:
            self.stack = []
            self.screen = None
            screen = instead
        if self.screen is not None:
            self.screen.leave()
            self.stack.append(self.screen)
        self.screen = screen
        self.show(screen)

    def show(self, screen, back: bool = False) -> None:
        for name, group in L.SCREENS.items():
            if name != screen.name:
                self.visible(group, False)
        self.prop("pley.screen", screen.name)
        self.visible(L.SCREENS[screen.name], True)
        self.prop("pley.rail", screen.rail or (self.stack[0].rail if self.stack else "home"))
        self.prop("pley.backdrop.on", "1" if screen.backdrop else "")
        if not screen.backdrop:
            self.prop("pley.backdrop", "")
        filled = self.owner.get(screen.name) is screen
        self.owner[screen.name] = screen
        screen.show(back and filled)

    def back(self) -> None:
        if self.screen is not None and self.screen.back():
            return
        if self.stack:
            self.screen.leave()
            self.screen = self.stack.pop()
            self.show(self.screen, back=True)
            return
        if self.focus_in_rail():
            self.close()
        else:
            self.focus_rail()

    def failed(self, error: api.ApiError, screen) -> None:
        """What a page says when the server didn't answer it."""
        if error.code == "unreachable":
            name = kodi.setting("server_name") or kodi.setting("server_url")
            notice = Notice(self, T("cant_reach") % name, T("cant_reach_hint"), retry=screen)
        elif error.code == "signed_out":
            notice = Notice(self, T("signed_out"), "", retry=screen, sign_in=True)
        else:
            notice = Notice(self, T("page_failed") if not isinstance(screen, Home) else T("home_failed"), error.message, retry=screen)
        if self.screen is None or isinstance(screen, Home):
            self.stack = []
            self.screen = None
        self.open(notice)

    # ---------------------------------------------------------------- rail

    def setup_rail(self) -> None:
        name = self.me.get("name") or self.me.get("username") or ""
        self.prop("pley.avatar.name", name)
        self.prop("pley.avatar.initial", name[:1].upper())
        self.prop("pley.avatar.server", kodi.setting("server_name") or "")
        tint = (self.me.get("avatar_color") or "").lstrip("#")
        self.prop("pley.avatar.color", "FF" + tint.upper() if len(tint) == 6 else L.SURFACE3)
        # Text on a light colour is dark, as the Android app's avatar draws it.
        light = len(tint) == 6 and luminance(tint) > 0.45
        self.prop("pley.avatar.ink", L.ON_ACCENT if light else L.FG)
        if self.me.get("avatar_url"):
            self.prop("pley.avatar.picture", image(self.session.base, self.me["avatar_url"], 128))
        y = L.RAIL_TOP + 2 * L.RAIL_PITCH
        for i, entry in enumerate(L.RAIL_LIBRARIES):
            group = L.rail_group(entry)
            if i < len(self.libraries):
                library = self.libraries[i]
                icon = LIBRARY_ICONS.get(library.get("kind"), "icon-folder.png")
                for control in L.rail_icons(entry):
                    self.c(control).setImage(icon, False)
                for control in L.rail_labels(entry):
                    self.put(control, library.get("name"))
                self.place(group, 0, y - 50)
                self.visible(group, True)
                y += L.RAIL_PITCH
            else:
                self.visible(group, False)
        for entry in (L.RAIL_PLAYLISTS, L.RAIL_COLLECTIONS):
            self.place(L.rail_group(entry), 0, y - 50)
            y += L.RAIL_PITCH

    def focus_in_rail(self) -> bool:
        return self.getFocusId() in L.RAIL_ENTRIES

    def focus_rail(self) -> None:
        """To the rail's current entry (D-pad rule 3)."""
        self.setFocusId(self.rail_entry(self.getProperty("pley.rail")))

    def rail_entry(self, key: str) -> int:
        if key.startswith("lib"):
            return L.RAIL_LIBRARIES[int(key[3:])]
        return {
            "profile": L.RAIL_AVATAR,
            "search": L.RAIL_SEARCH,
            "playlists": L.RAIL_PLAYLISTS,
            "collections": L.RAIL_COLLECTIONS,
            "settings": L.RAIL_SETTINGS,
        }.get(key, L.RAIL_HOME)

    def rail_click(self, control_id: int) -> None:
        if control_id == L.RAIL_AVATAR:
            return self.account()
        if control_id == L.RAIL_SETTINGS:
            kodi.addon().openSettings()
            return
        if control_id == L.RAIL_SEARCH:
            return self.go(Search(self))
        if control_id == L.RAIL_HOME:
            return self.go(Home(self))
        if control_id == L.RAIL_PLAYLISTS:
            return self.go(Grid.playlists(self))
        if control_id == L.RAIL_COLLECTIONS:
            return self.go(Grid.collections(self))
        if control_id in L.RAIL_LIBRARIES:
            i = L.RAIL_LIBRARIES.index(control_id)
            if i < len(self.libraries):
                self.go(Library(self, self.libraries[i], "lib%d" % i))

    def account(self) -> None:
        chosen = xbmcgui.Dialog().contextmenu([T("choose_server"), T("sign_out")])
        if chosen == 0:
            self.leave_for("choose_server")
        elif chosen == 1:
            self.leave_for("sign_out")

    def leave_for(self, after: str) -> None:
        """Closes the window, and does `after` (run())."""
        self.after = after
        self.close()

    # -------------------------------------------------------------- events

    def onAction(self, action) -> None:
        with self.lock:
            if self.closing:
                return
            code = action.getId()
            if code in BACK:
                return self.back()
            focus = self.getFocusId()
            if code in (CONTEXT_MENU, MENU) and self.screen is not None:
                return self.screen.menu(focus)
            if code in MOVES and self.screen is not None:
                if focus in L.RAIL_ENTRIES:
                    if code == RIGHT:
                        self.screen.enter()
                    return
                self.screen.moved(focus, code)

    def onClick(self, control_id: int) -> None:
        with self.lock:
            if self.closing:
                return
            if control_id in L.RAIL_ENTRIES:
                return self.rail_click(control_id)
            if self.screen is not None:
                self.screen.click(control_id)

    def onFocus(self, control_id: int) -> None:
        with self.lock:
            previous, self.focus = self.focus, control_id
            if self.closing or self.screen is None:
                return
            if control_id in L.RAIL_ENTRIES:
                current = self.rail_entry(self.getProperty("pley.rail"))
                if previous not in L.RAIL_ENTRIES and control_id != current:
                    # Into the rail from the page: to the page's entry.
                    self.setFocusId(current)
                return
            self.screen.focused(control_id)

    # ---------------------------------------------------- rest and toasts

    def post(self, work) -> None:
        """Runs `work` on the window's thread, soon: what other threads
        (timers, answers from the server) change on screen goes through here,
        as Kodi's controls are changed partly at once and partly by messages
        to its GUI thread, which threads racing each other could reorder."""
        with self.posted_lock:
            self.posted.append(work)
        xbmc.executebuiltin("NotifyAll(%s,%s)" % (kodi.ADDON_ID, POSTED))

    def run_posted(self) -> None:
        with self.posted_lock:
            work, self.posted = self.posted, []
        with self.lock:
            for job in work:
                if self.closing:
                    return
                try:
                    job()
                except api.ApiError as e:
                    kodi.log("posted work: %s" % e.message, xbmc.LOGWARNING)

    def rest(self, then) -> None:
        """Calls `then` once focus rested REST seconds where it is now."""
        self.rest_token += 1
        token = self.rest_token
        if self.rest_timer is not None:
            self.rest_timer.cancel()

        def fire():
            if token == self.rest_token and not self.closing:
                self.post(lambda: token == self.rest_token and then())

        self.rest_timer = threading.Timer(REST, fire)
        self.rest_timer.daemon = True
        self.rest_timer.start()

    def later(self, work, then) -> None:
        """Runs `work` away from the window's thread, then `then(answer)`
        on it (not once it closed)."""

        def run():
            try:
                answer = work()
            except api.ApiError as e:
                kodi.log("in the background: %s" % e.message, xbmc.LOGWARNING)
                return
            self.post(lambda: then(answer))

        threading.Thread(target=run, daemon=True).start()

    def toast(self, text: str, seconds: float = 3.5) -> None:
        self.put(L.TOAST_LABEL, text)
        self.prop("pley.toast", "1")
        if self.toast_timer is not None:
            self.toast_timer.cancel()
        self.toast_timer = threading.Timer(seconds, lambda: self.post(lambda: self.prop("pley.toast", "")))
        self.toast_timer.daemon = True
        self.toast_timer.start()

    # ---------------------------------------------------------- things to do

    def play(self, item: dict, from_start: bool = False) -> None:
        """Plays `item` (a show or season: the episode to watch next)."""
        try:
            if item.get("kind") in ("show", "season"):
                found = self.session.get("/api/items/%d/on-deck" % item["id"]).get("item")
                if not found:
                    return self.toast(T("nothing_found"))
                item = found
            start = 0 if from_start else (words.state(item).get("position_ms") or 0) if words.in_progress(item) else 0
            li = playback.prepare(self.session.client, self.session.base, item["id"], start, self.session.cached(item["id"]))
        except api.ApiError as e:
            return self.toast(T("server_error") % e.message, 6)
        if start:
            li.setProperty("StartOffset", "%.3f" % (start / 1000.0))
        xbmc.Player().play(li.getPath(), li)

    def set_watched(self, item: dict, watched: bool) -> None:
        self.session.client.post("/api/items/%d/%s" % (item["id"], "watched" if watched else "unwatched"))
        self.changed()

    def set_listed(self, item: dict, listed: bool) -> None:
        path = "/api/watchlist/%d" % item["id"]
        if listed:
            self.session.client.put(path)
        else:
            self.session.client.delete(path)
        self.changed()

    def tile_menu(self, item: dict, continue_watching: bool = False) -> None:
        """A tile's actions (TvItemActionsScreen), in Kodi's context menu."""
        kind = item.get("kind")
        actions = []
        if kind in ("movie", "show", "season", "episode", "extra"):
            watched = words.state(item).get("watched")
            actions.append((T("mark_unwatched") if watched else T("mark_watched"), lambda: self.set_watched(item, not watched)))
        if kind in ("movie", "show"):
            try:
                listed = self.session.detail(item["id"]).get("in_watchlist")
            except api.ApiError:
                listed = False
            actions.append((T("remove_from_list") if listed else T("add_to_list"), lambda: self.set_listed(item, not listed)))
        if kind in PLAYLIST_KINDS:
            actions.append((T("add_to_playlist"), lambda: self.add_to_playlist(item)))
        if continue_watching:
            actions.append((T("remove_from_continue"), lambda: self.forget_progress(item)))
        if kind == "episode" and item.get("grandparent_id"):
            actions.append((T("go_to_show"), lambda: self.open(Item(self, item["grandparent_id"]))))
        if not actions:
            return
        chosen = xbmcgui.Dialog().contextmenu([name for name, _ in actions])
        if chosen >= 0:
            try:
                actions[chosen][1]()
            except api.ApiError as e:
                self.toast(T("server_error") % e.message, 6)

    def forget_progress(self, item: dict) -> None:
        self.session.client.delete("/api/continue-watching/%d" % item["id"])
        self.changed()

    def add_to_playlist(self, item: dict) -> None:
        """Add to playlist (TvAddToPlaylist): into a new playlist named
        here, or one of the profile's of the item's kind (smart ones fill
        themselves)."""
        kind = PLAYLIST_KINDS.get(item.get("kind"), "video")
        playlists = self.session.get("/api/playlists").get("playlists", [])
        mine = [p for p in playlists if not p.get("smart_filter") and p.get("kind", kind) == kind]
        new = T("new_music_playlist" if kind == "audio" else "new_video_playlist")
        entries = [xbmcgui.ListItem(new, offscreen=True)]
        entries += [xbmcgui.ListItem(p.get("title") or "", T("playlist_items") % p.get("item_count", 0), offscreen=True) for p in mine]
        chosen = xbmcgui.Dialog().select(T("add_to_playlist"), entries)
        if chosen < 0:
            return
        if chosen == 0:
            title = xbmcgui.Dialog().input(new).strip()
            if not title:
                return
            self.session.client.post("/api/playlists", {"title": title, "kind": kind, "item_ids": [item["id"]]})
        else:
            playlist = mine[chosen - 1]
            title = playlist.get("title") or ""
            self.session.client.post("/api/playlists/%d/items" % playlist["id"], {"item_ids": [item["id"]]})
        self.toast(T("added_to") % title)

    def open_item(self, item: dict) -> None:
        """What a tile opens: the item's page; a person's, a library's."""
        kind = item.get("kind")
        if kind == "person":
            return self.open(Grid.person(self, item))
        if kind == "seeall":
            library = next((lib for lib in self.libraries if lib["id"] == item.get("library_id")), None)
            if library is not None:
                self.open(Library(self, library, None))
            return
        if kind == "extra":
            return self.play(item, from_start=True)
        if kind == "track":
            return self.play(item, from_start=True)
        self.open(Item(self, item["id"], item))


def luminance(hex_rgb: str) -> float:
    r, g, b = (int(hex_rgb[i : i + 2], 16) / 255.0 for i in (0, 2, 4))
    lin = [c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4 for c in (r, g, b)]
    return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2]


# ----------------------------------------------------------------- rows


class Rows:
    """A screen's rows of tiles: slots the skin holds, filled and placed
    here. A row: {"key", "title", "shape", "items", "see_all"}."""

    def __init__(self, win: Window, base: int, slots: int, shapes: tuple):
        self.win = win
        self.base = base
        self.slots = slots
        self.shapes = shapes
        self.rows = []

    def fill(self, rows: list) -> None:
        self.rows = rows[: self.slots]
        win = self.win
        base = win.session.base
        for i in range(self.slots):
            group = L.slot(self.base, i)
            if i >= len(self.rows):
                win.visible(group, False)
                continue
            row = self.rows[i]
            win.put(L.slot_header(self.base, i), row["title"])
            for shape in self.shapes:
                found = win.c(L.slot_list(self.base, i, shape))
                found.reset()
                if shape == row["shape"]:
                    if shape == "person":
                        items = [tiles.person(base, credit) for credit in row["items"]]
                    else:
                        items = [tiles.tile(base, item, shape) for item in row["items"]]
                    if row.get("see_all") is not None:
                        items.append(tiles.see_all(T("see_all")))
                    found.addItems(items)
                    found.setVisible(True)
                else:
                    found.setVisible(False)
            win.visible(group, True)

    def place(self, focused, top: int) -> None:
        """Places the rows: from `top`, or with row `focused` under the info
        block and the rows above it out of sight."""
        y = top if focused is None else L.FOCUSED_ROW_Y
        for i, row in enumerate(self.rows):
            group = L.slot(self.base, i)
            if focused is not None and i < focused:
                self.win.place(group, 0, -2 * L.HEIGHT)
                continue
            self.win.place(group, 0, y)
            y += L.row_height(L.SHAPES[row["shape"]]) + L.ROW_GAP

    def find(self, control_id: int):
        """(row index, row) of a row list's id, or None."""
        found = L.find_slot(self.base, self.slots, control_id)
        if found is None or found[0] >= len(self.rows):
            return None
        return found[0], self.rows[found[0]]

    def selected(self, control_id: int):
        """(row index, row, item) under focus in a row list, or None. A row's
        "See all" is {"kind": "seeall", "library_id": …}."""
        found = self.find(control_id)
        if found is None:
            return None
        i, row = found
        at = self.win.c(control_id).getSelectedPosition()
        if at < 0:
            return None
        if at >= len(row["items"]):
            return i, row, {"kind": "seeall", "library_id": row.get("see_all")}
        return i, row, row["items"][at]

    def focus(self, i: int, at: int = 0) -> None:
        row = self.rows[i]
        list_id = L.slot_list(self.base, i, row["shape"])
        self.win.setFocusId(list_id)
        if at:
            self.win.c(list_id).selectItem(at)


# --------------------------------------------------------------- screens


class Screen:
    """A page of pley's window."""

    name = ""
    rail = None
    backdrop = False

    def __init__(self, win: Window):
        self.win = win
        self.session = win.session
        self.last_focus = None

    def load(self) -> None:
        """Asks the server for what the page shows (may raise ApiError)."""

    def instead(self):
        """A page to show instead of this one, once loaded (None: this one)."""
        return None

    def show(self, filled: bool) -> None:
        """Draws the page; `filled`: its controls still hold it (Back)."""

    def leave(self) -> None:
        self.last_focus = self.win.getFocusId()

    def resume(self) -> None:
        """The window shows again (after playing): what changed shows."""
        self.refresh()

    def refresh(self) -> None:
        """What the page shows changed on the server."""

    def back(self) -> bool:
        return False

    def enter(self) -> None:
        """Right from the rail: back to where focus was on the page."""
        self.win.setFocusId(self.last_focus or self.first())

    def first(self) -> int:
        return 0

    def focused(self, control_id: int) -> None:
        pass

    def moved(self, control_id: int, action: int) -> None:
        pass

    def click(self, control_id: int) -> None:
        pass

    def menu(self, control_id: int) -> None:
        pass


def hero_lines(win: Window, ids: dict, info: dict, gaps: dict) -> int:
    """Places a hero's pieces down its column as the Android app stacks
    them (a missing one closes its gap), and returns the buttons' top.
    `ids`/`gaps`: by piece; `info`: the texts."""
    title = info.get("title") or ""
    count = lines(title, 60, L.TEXT_WIDTH, 2, bold=True)
    # The title sits on the foot of its box, which ends at 248.
    win.place(ids["title"], L.GUTTER, 248 - max(count, 1) * 82)
    win.put(ids["title"], title)
    y = 248
    for piece in ("context", "meta", "left", "genres", "summary", "credits"):
        if piece not in ids:
            continue
        value = info.get(piece)
        shown = bool(value)
        if piece == "summary":
            win.visible(ids[piece], shown)
            if shown:
                count = lines(value, 30, L.TEXT_WIDTH, info.get("summary_lines", 3))
                y += gaps[piece]
                win.place(ids[piece], L.GUTTER, y)
                win.c(ids[piece]).setHeight(count * 42)
                win.text(ids[piece], clamp(value, 30, L.TEXT_WIDTH, info.get("summary_lines", 3)))
                y += count * 42
            continue
        if piece == "meta":
            win.meta(ids[piece], value or [])
            if not value:
                win.visible(ids[piece], False)
                continue
            win.visible(ids[piece], True)
            y += gaps[piece]
            win.place(ids[piece], L.GUTTER, y)
            y += 36
            continue
        win.visible(ids[piece], shown)
        if shown:
            height = {"context": 44}.get(piece, 34)
            y += gaps[piece]
            win.place(ids[piece], L.GUTTER, y)
            win.put(ids[piece], value)
            y += height
    return y + gaps["buttons"]


def meta_parts(item: dict, detail=None, lead: str = "") -> list:
    """The metadata line's pieces (TvMetaLine)."""
    found = words.meta(item, detail)
    parts = []
    if lead:
        parts.append(("lead", lead))
    if found["year"]:
        parts.append(("text", found["year"]))
    if found["content_rating"]:
        parts.append(("badge", found["content_rating"]))
    if found["runtime"]:
        parts.append(("text", found["runtime"]))
    if found["rating"]:
        parts.append(("rating", found["rating"]))
    parts += [("quality", b) for b in found["badges"]]
    return parts[: L.META_PIECES]


def about(item: dict, detail=None) -> dict:
    """What a hero or an info block says of `item` (HomeInfo)."""
    detail = detail if detail and detail.get("id") == item.get("id") else None
    episode = item.get("kind") == "episode"
    show = item.get("grandparent_title") if episode else None
    context = ""
    if episode:
        context = " · ".join(p for p in (words.episode_label(item.get("parent_index"), item.get("index")), item.get("title")) if p)
    progress = words.in_progress(item)
    return {
        "title": show or item.get("title"),
        "context": context,
        "meta": meta_parts(item, detail),
        "left": words.time_left(words.state(item).get("position_ms"), item.get("duration_ms")) if progress else "",
        "summary": ((detail or {}).get("summary") or item.get("summary") or "").strip(),
        "progress": progress,
        "watched": bool(words.state(item).get("watched")),
    }


class Home(Screen):
    """Home (TvHome): the hero for what to watch next over its backdrop,
    then the server's rows; in the rows, an info block about the tile focus
    rests on, and its backdrop."""

    name = "home"
    rail = "home"
    backdrop = True

    def __init__(self, win: Window):
        super().__init__(win)
        self.rows = Rows(win, L.HOME_ROWS, L.HOME_SLOTS, ("poster", "landscape", "square"))
        self.hero = None
        self.model = []
        self.in_rows = False
        self.about_id = None

    def load(self) -> None:
        hubs = self.session.get("/api/hubs/home", summaries=1).get("hubs", [])
        kinds = {lib["id"]: lib.get("kind") for lib in self.win.libraries}
        self.model = [
            {
                "key": hub["key"],
                "title": hub["title"],
                "shape": tiles.row_shape(hub, kinds.get(hub.get("library_id"))),
                "items": hub["items"],
                "see_all": tiles.see_all_of(hub),
            }
            for hub in hubs
            if hub.get("items")
        ]
        self.hero = tiles.hero_of(self.model)

    def instead(self):
        if self.model:
            return None
        win = self.win
        if not win.libraries:
            return Notice(win, T("no_libraries"), T("no_libraries_body"), retry=self)
        scanning = any(lib.get("scanning") for lib in win.libraries)
        return Notice(win, T("scanning") if scanning else T("nothing_yet"), T("nothing_yet_body"), retry=self)

    def show(self, filled: bool) -> None:
        win = self.win
        if not filled:
            self.rows.fill(self.model)
            self.draw_hero()
        if self.in_rows and filled and self.last_focus:
            found = self.rows.find(self.last_focus)
            if found:
                self.rows_mode(found[0])
                win.setFocusId(self.last_focus)
                return
        self.hero_mode()

    def draw_hero(self) -> None:
        if self.hero is None:
            return
        win = self.win
        detail = self.session.cached(self.hero["id"])
        info = about(self.hero, detail)
        ids = {"title": L.HOME_TITLE, "context": L.HOME_CONTEXT, "meta": L.HOME_META, "left": L.HOME_LEFT, "summary": L.HOME_SUMMARY}
        gaps = {"context": 20, "meta": 16, "left": 20, "summary": 20, "buttons": 40}
        top = hero_lines(win, ids, info, gaps)
        win.place(L.HOME_BUTTONS_BOX, L.GUTTER, top)
        win.put(L.HOME_PLAY, ICON_ROOM + (T("resume") if info["progress"] else T("play")))
        win.visible(L.button_box(L.HOME_FROM_START), info["progress"])
        win.visible(L.button_box(L.HOME_WATCHED), not info["watched"])
        win.visible(L.button_box(L.HOME_UNWATCHED), info["watched"])
        if detail is None:
            hero = self.hero
            win.later(lambda: self.session.detail(hero["id"]), lambda _: self.hero is hero and self.draw_hero())

    def hero_mode(self) -> None:
        win = self.win
        self.in_rows = False
        win.prop("pley.mode", "hero")
        win.prop("pley.scrim", "hero")
        win.visible(L.HOME_HERO, True)
        if self.hero is not None:
            win.prop("pley.backdrop", tiles.backdrop(self.session.base, self.hero))
        self.rows.place(None, 768)
        win.setFocusId(L.HOME_PLAY)

    def rows_mode(self, i: int) -> None:
        self.in_rows = True
        self.win.prop("pley.mode", "rows")
        self.win.prop("pley.scrim", "rows")
        self.rows.place(i, 0)

    def first(self) -> int:
        return L.HOME_PLAY

    def focused(self, control_id: int) -> None:
        found = self.rows.find(control_id)
        if found is not None:
            entering = not self.in_rows
            self.rows_mode(found[0])
            if entering:
                # From the hero, the info block says at once what focus is on.
                self.rested(control_id)
            else:
                self.rested_later(control_id)
        elif control_id in (L.HOME_PLAY, L.HOME_FROM_START, L.HOME_INFO, L.HOME_WATCHED, L.HOME_UNWATCHED) and self.in_rows:
            self.hero_mode()

    def moved(self, control_id: int, action: int) -> None:
        found = self.rows.find(control_id)
        if found is None:
            return
        if action == UP and found[0] == 0 and self.win.getFocusId() == control_id:
            # Up from the first row: back to the hero (its group was hidden,
            # so Kodi couldn't move there itself).
            self.win.visible(L.HOME_HERO, True)
            return self.hero_mode()
        self.rested_later(control_id)

    def rested_later(self, control_id: int) -> None:
        self.win.rest(lambda: self.rested(control_id))

    def rested(self, control_id: int) -> None:
        found = self.rows.selected(control_id)
        if found is None or self.win.getFocusId() != control_id:
            return
        _, _, item = found
        if item.get("kind") == "seeall":
            return
        self.about_id = item["id"]
        self.draw_info(item, self.session.cached(item["id"]))
        self.win.prop("pley.backdrop", tiles.backdrop(self.session.base, item))
        if self.session.cached(item["id"]) is None:
            self.win.later(lambda: self.session.detail(item["id"]), lambda detail: self.about_id == item["id"] and self.draw_info(item, detail))

    def draw_info(self, item: dict, detail) -> None:
        """The info block over the rows (TvInfoBlock)."""
        win = self.win
        info = about(item, detail)
        win.put(L.HOME_INFO_TITLE, info["title"])
        win.meta(L.HOME_INFO_META, meta_parts(item, detail, lead=info["context"]))
        win.text(L.HOME_INFO_SUMMARY, clamp(info["summary"], 27, L.TEXT_WIDTH, 2))

    def back(self) -> bool:
        if self.in_rows:
            # Back from the rows: to the hero (TvHome's BackHandler).
            self.win.visible(L.HOME_HERO, True)
            self.hero_mode()
            return True
        return False

    def click(self, control_id: int) -> None:
        win = self.win
        if control_id == L.HOME_PLAY and self.hero:
            return win.play(self.hero)
        if control_id == L.HOME_FROM_START and self.hero:
            return win.play(self.hero, from_start=True)
        if control_id == L.HOME_INFO and self.hero:
            return win.open_item(self.hero)
        if control_id in (L.HOME_WATCHED, L.HOME_UNWATCHED) and self.hero:
            try:
                win.set_watched(self.hero, control_id == L.HOME_WATCHED)
            except api.ApiError as e:
                win.toast(T("server_error") % e.message, 6)
            return
        found = self.rows.selected(control_id)
        if found is not None:
            win.open_item(found[2])

    def menu(self, control_id: int) -> None:
        found = self.rows.selected(control_id)
        if found is not None and found[2].get("kind") != "seeall":
            self.win.tile_menu(found[2], continue_watching=found[1]["key"].startswith("continue"))

    def refresh(self) -> None:
        focus = self.win.getFocusId()
        found = self.rows.selected(focus)
        self.load()
        if not self.model:
            return
        self.rows.fill(self.model)
        self.draw_hero()
        if self.in_rows and found is not None:
            # Focus stays on the tile, or its neighbour when it went.
            i, row, item = found
            at = next((n for n, r in enumerate(self.model) if r["key"] == row["key"]), None)
            if at is None:
                return self.hero_mode()
            ids = [x.get("id") for x in self.model[at]["items"]]
            position = ids.index(item.get("id")) if item.get("id") in ids else min(self.win.c(focus).getSelectedPosition(), len(ids) - 1)
            self.rows_mode(at)
            self.rows.focus(at, max(position, 0))
        elif not self.in_rows:
            self.hero_mode()


class Item(Screen):
    """An item's page (TvItemScreen): the title over its backdrop, its
    buttons, then a show's or season's episodes, Extras, Cast & Crew and
    More Like This; once focus leaves the buttons, an info block about the
    page, or the focused episode."""

    name = "item"
    backdrop = True

    def __init__(self, win: Window, item_id: int, summary=None):
        super().__init__(win)
        self.item_id = item_id
        self.summary = summary
        self.rows = Rows(win, L.ITEM_ROWS, L.ITEM_SLOTS, ("poster", "landscape", "episode", "person"))
        self.detail = {}
        self.seasons = []
        self.season = None
        self.episodes = []
        self.on_deck = None
        self.related = []
        self.area = "hero"
        self.episode_about = None
        self.playable = True
        self.model = []
        self.first_y = 800

    # -- loading

    def load(self) -> None:
        s = self.session
        known = (self.summary or {}).get("kind")
        if known:
            # The tile said what it is: its details come with the rest.
            detail = None
            kind = known
            parent = (self.summary or {}).get("parent_id")
        else:
            detail = s.detail(self.item_id, fresh=True)
            kind = detail["kind"]
            parent = detail.get("parent_id")
        calls = []
        if detail is None:
            calls.append(("detail", lambda: s.detail(self.item_id, fresh=True)))
        if kind == "show":
            calls.append(("seasons", lambda: s.get("/api/items/%d/children" % self.item_id, summaries=1).get("items", [])))
            calls.append(("on_deck", lambda: s.get("/api/items/%d/on-deck" % self.item_id).get("item")))
        elif kind == "season":
            calls.append(("episodes", lambda: s.get("/api/items/%d/children" % self.item_id, summaries=1).get("items", [])))
            calls.append(("on_deck", lambda: s.get("/api/items/%d/on-deck" % self.item_id).get("item")))
        elif kind == "episode" and parent:
            calls.append(("episodes", lambda: s.get("/api/items/%d/children" % parent, summaries=1).get("items", [])))
        if kind in ("movie", "show"):
            calls.append(("related", lambda: s.get("/api/items/%d/related" % self.item_id).get("items", [])))
        answers = dict(zip([name for name, _ in calls], api.together(*[call for _, call in calls]))) if calls else {}
        self.detail = detail or answers["detail"]
        self.related = answers.get("related") or []
        self.on_deck = answers.get("on_deck")
        if kind == "show":
            self.seasons = answers.get("seasons") or []
            wanted = (self.on_deck or {}).get("parent_id")
            self.season = next((x for x in self.seasons if x["id"] == wanted), self.seasons[0] if self.seasons else None)
            self.episodes = s.get("/api/items/%d/children" % self.season["id"], summaries=1).get("items", []) if self.season else []
        else:
            self.episodes = answers.get("episodes") or []
        self.model = self.rows_model()

    def rows_model(self) -> list:
        d = self.detail
        rows = []
        if self.episodes:
            title = words.season_title((self.season or {}).get("title"), (self.season or {}).get("index"))
            if d["kind"] in ("season", "episode"):
                title = words.season_title(None, self.episodes[0].get("parent_index"))
            rows.append({"key": "episodes", "title": "" if len(self.seasons) > 1 else title, "shape": "episode", "items": self.episodes})
        extras = d.get("extras") or []
        if extras:
            rows.append({"key": "extras", "title": T("extras"), "shape": "landscape", "items": extras})
        credits = cast_and_crew(d)
        if credits:
            crew = any(c.get("job") for c in credits)
            cast = any(not c.get("job") for c in credits)
            episode = d["kind"] == "episode"
            if not crew:
                title = T("guest_stars") if episode else T("cast")
            elif not cast:
                title = T("crew")
            else:
                title = T("cast_and_crew")
            rows.append({"key": "cast", "title": title, "shape": "person", "items": credits})
        if self.related:
            rows.append({"key": "related", "title": T("more_like_this"), "shape": "poster", "items": self.related})
        return rows

    # -- drawing

    def primary(self) -> tuple:
        """(label, item to play, resumes): Resume, Play, "Resume S1 · E3"."""
        d = self.detail
        if d["kind"] in ("movie", "episode", "extra"):
            return (T("resume") if words.in_progress(d) else T("play")), d, words.in_progress(d)
        target = self.on_deck
        if not target:
            return T("unavailable"), None, False
        label = words.episode_label(target.get("parent_index"), target.get("index"))
        if words.in_progress(target):
            return T("resume_episode") % label, target, True
        return T("play_episode") % label, target, False

    def show(self, filled: bool) -> None:
        if not filled:
            self.rows.fill(self.model)
            self.draw()
        self.win.prop("pley.backdrop", tiles.backdrop(self.session.base, self.detail))
        if filled and self.last_focus and self.area != "hero":
            self.restore()
        else:
            self.hero_mode()

    def draw(self) -> None:
        win, d = self.win, self.detail
        kind = d["kind"]
        show_title = d.get("grandparent_title") if kind == "episode" else d.get("parent_title") if kind == "season" else None
        context = ""
        if kind == "episode":
            context = " · ".join(p for p in (words.episode_label(d.get("parent_index"), d.get("index")), d.get("title")) if p)
        elif kind == "season" and show_title:
            context = d.get("title") or words.season_title(None, d.get("index"))
        label, target, resumes = self.primary()
        genres = list(d.get("genres") or [])
        info = {
            "title": show_title or d.get("title"),
            "context": context,
            "meta": words.item_meta(d, self.seasons)[: L.META_PIECES],
            "left": words.time_left(words.state(target).get("position_ms"), target.get("duration_ms")) if resumes and target else "",
            "genres": " · ".join(genres),
            "summary": (d.get("summary") or "").strip(),
            "credits": credits_line(d),
        }
        ids = {
            "title": L.ITEM_TITLE,
            "context": L.ITEM_CONTEXT,
            "meta": L.ITEM_META,
            "left": L.ITEM_LEFT,
            "genres": L.ITEM_GENRES,
            "summary": L.ITEM_SUMMARY,
            "credits": L.ITEM_CREDITS,
        }
        gaps = {"context": 16, "meta": 24, "left": 16, "genres": 12, "summary": 20, "credits": 16, "buttons": 44}
        top = hero_lines(win, ids, info, gaps)
        win.place(L.ITEM_BUTTONS_BOX, L.GUTTER, top)
        self.first_y = top + L.BUTTON + L.BUTTON_LABEL_ROOM + 16 + L.ROW_GAP
        win.put(L.ITEM_PLAY, ICON_ROOM + label)
        self.playable = target is not None
        win.c(L.ITEM_PLAY).setEnabled(self.playable)
        win.visible(L.button_box(L.ITEM_FROM_START), resumes)
        watched = bool(words.state(d).get("watched"))
        win.visible(L.button_box(L.ITEM_WATCHED), not watched)
        win.visible(L.button_box(L.ITEM_UNWATCHED), watched)
        listable = kind in ("movie", "show")
        win.visible(L.button_box(L.ITEM_ADD_TO_LIST), listable and not d.get("in_watchlist"))
        win.visible(L.button_box(L.ITEM_IN_LIST), listable and bool(d.get("in_watchlist")))
        win.visible(L.button_box(L.ITEM_TRAILER), self.trailer() is not None)
        win.visible(L.button_box(L.ITEM_ADD_TO_PLAYLIST), kind in PLAYLIST_KINDS)
        win.visible(L.button_box(L.ITEM_MORE), bool(self.more_actions()))
        # Season tabs, for a show of more than one season.
        tabs = win.c(L.ITEM_SEASONS)
        tabs.reset()
        if len(self.seasons) > 1:
            entries = []
            for season in self.seasons:
                li = xbmcgui.ListItem(words.season_title(season.get("title"), season.get("index")), offscreen=True)
                li.select(season is self.season)
                entries.append(li)
            tabs.addItems(entries)
            tabs.selectItem(self.seasons.index(self.season) if self.season in self.seasons else 0)
            tabs.setVisible(True)
        else:
            tabs.setVisible(False)
        # The episodes row starts on the one to watch (or this episode).
        if self.episodes:
            current = self.item_id if kind == "episode" else (self.on_deck or {}).get("id")
            ids_ = [e["id"] for e in self.episodes]
            if current in ids_:
                win.c(L.slot_list(L.ITEM_ROWS, 0, "episode")).selectItem(ids_.index(current))

    def trailer(self):
        return next((x for x in self.detail.get("extras") or [] if x.get("extra_type") == "trailer"), None)

    def more_actions(self) -> list:
        d = self.detail
        actions = []
        summary = (d.get("summary") or "").strip()
        if summary and lines(summary, 30, L.TEXT_WIDTH, 4) > 3 or d.get("tagline"):
            actions.append((T("full_summary"), self.full_summary))
        if d["kind"] == "episode" and d.get("grandparent_id"):
            actions.append((T("go_to_show"), lambda: self.win.open(Item(self.win, d["grandparent_id"]))))
        if d["kind"] == "episode" and d.get("parent_id"):
            actions.append((T("go_to_season"), lambda: self.win.open(Item(self.win, d["parent_id"]))))
        if d["kind"] == "season" and d.get("parent_id"):
            actions.append((T("go_to_show"), lambda: self.win.open(Item(self.win, d["parent_id"]))))
        return actions

    def full_summary(self) -> None:
        d = self.detail
        text = "\n\n".join(p for p in (d.get("tagline"), d.get("summary")) if p)
        xbmcgui.Dialog().textviewer(d.get("title") or "", text)

    def hero_mode(self) -> None:
        win = self.win
        self.area = "hero"
        win.prop("pley.mode", "hero")
        win.prop("pley.scrim", "item")
        win.visible(L.ITEM_HERO, True)
        self.rows.place(None, self.first_y)
        self.place_tabs(self.first_y)
        win.setFocusId(L.ITEM_PLAY if self.playable else L.ITEM_WATCHED)

    def place_tabs(self, episodes_y: int) -> None:
        if len(self.seasons) > 1 and self.rows.rows and self.rows.rows[0]["key"] == "episodes":
            self.win.place(L.ITEM_SEASONS, L.GUTTER, episodes_y - 32)

    def rows_mode(self, i: int) -> None:
        win = self.win
        win.prop("pley.mode", "rows")
        win.prop("pley.scrim", "rows")
        row = self.rows.rows[i]
        if row["key"] == "episodes":
            self.area = "episodes"
            # The episodes under the info block, as the spec puts them (y = 272 dp).
            self.rows.place(None, 468)
            self.place_tabs(468)
        else:
            self.area = "rows"
            self.rows.place(i, 0)
            self.win.place(L.ITEM_SEASONS, L.GUTTER, -2 * L.HEIGHT)
            self.draw_info(None)

    def draw_info(self, episode) -> None:
        """The info block: the page's title and metadata, then the focused
        episode's title, its line and summary, or the page's summary."""
        win, d = self.win, self.detail
        kind = d["kind"]
        show_title = d.get("grandparent_title") if kind == "episode" else d.get("parent_title") if kind == "season" else None
        win.put(L.ITEM_INFO_TITLE, show_title or d.get("title"))
        if episode is not None:
            win.meta(L.ITEM_INFO_META, words.item_meta(d, self.seasons)[: L.META_PIECES])
            win.put(L.ITEM_INFO_EPISODE, episode.get("title"))
            line = " · ".join(p for p in (words.episode_long_label(episode.get("parent_index"), episode.get("index")), words.runtime(episode.get("duration_ms"))) if p)
            win.put(L.ITEM_INFO_LINE, line)
            win.visible(L.ITEM_INFO_EPISODE, True)
            win.visible(L.ITEM_INFO_LINE, True)
            win.place(L.ITEM_INFO_SUMMARY, L.GUTTER, 326)
            win.text(L.ITEM_INFO_SUMMARY, clamp(episode.get("summary") or "", 30, L.TEXT_WIDTH, 2))
        else:
            win.meta(L.ITEM_INFO_META, words.item_meta(d, self.seasons)[: L.META_PIECES])
            win.visible(L.ITEM_INFO_EPISODE, False)
            win.visible(L.ITEM_INFO_LINE, False)
            win.place(L.ITEM_INFO_SUMMARY, L.GUTTER, 216)
            win.text(L.ITEM_INFO_SUMMARY, clamp(d.get("summary") or "", 30, L.TEXT_WIDTH, 2))

    def restore(self) -> None:
        found = self.rows.find(self.last_focus)
        if found is None:
            return self.hero_mode()
        self.rows_mode(found[0])
        self.win.setFocusId(self.last_focus)

    def first(self) -> int:
        return L.ITEM_PLAY

    def focused(self, control_id: int) -> None:
        if control_id == L.ITEM_SEASONS:
            self.rows_mode(0)
            self.draw_info(None)
            return
        found = self.rows.find(control_id)
        if found is not None:
            self.rows_mode(found[0])
            if found[1]["key"] == "episodes":
                self.win.rest(lambda: self.rested(control_id))
            return
        if control_id in HERO_BUTTONS:
            if self.area != "hero":
                self.hero_mode()
                self.win.setFocusId(control_id)

    def rested(self, control_id: int) -> None:
        found = self.rows.selected(control_id)
        if found is not None and found[1]["key"] == "episodes" and self.win.getFocusId() == control_id:
            self.draw_info(found[2])

    def moved(self, control_id: int, action: int) -> None:
        if control_id == L.ITEM_SEASONS and action in (LEFT, RIGHT):
            return self.win.rest(self.pick_season)
        found = self.rows.find(control_id)
        if found is None:
            if control_id == L.ITEM_SEASONS and action == UP:
                self.up_to_hero()
            return
        if action == UP and self.win.getFocusId() == control_id:
            # Up from the first row there is (the hero's group was hidden).
            if found[0] == 0 and len(self.seasons) <= 1:
                return self.up_to_hero()
        if found[1]["key"] == "episodes":
            self.win.rest(lambda: self.rested(control_id))

    def up_to_hero(self) -> None:
        self.win.visible(L.ITEM_HERO, True)
        self.hero_mode()

    def pick_season(self) -> None:
        tabs = self.win.c(L.ITEM_SEASONS)
        at = tabs.getSelectedPosition()
        if not (0 <= at < len(self.seasons)) or self.seasons[at] is self.season:
            return
        self.season = self.seasons[at]
        for n in range(tabs.size()):
            tabs.getListItem(n).select(n == at)
        self.episodes = self.session.get("/api/items/%d/children" % self.season["id"], summaries=1).get("items", [])
        self.model = self.rows_model()
        self.rows.fill(self.model)
        self.place_tabs(468)
        self.rows.place(None, 468)

    def back(self) -> bool:
        if self.area != "hero":
            self.up_to_hero()
            return True
        return False

    def click(self, control_id: int) -> None:
        win, d = self.win, self.detail
        try:
            if control_id == L.ITEM_PLAY:
                _, target, _ = self.primary()
                if target is not None:
                    win.play(target)
            elif control_id == L.ITEM_FROM_START:
                _, target, _ = self.primary()
                if target is not None:
                    win.play(target, from_start=True)
            elif control_id in (L.ITEM_WATCHED, L.ITEM_UNWATCHED):
                win.set_watched(d, control_id == L.ITEM_WATCHED)
            elif control_id in (L.ITEM_ADD_TO_LIST, L.ITEM_IN_LIST):
                win.set_listed(d, control_id == L.ITEM_ADD_TO_LIST)
            elif control_id == L.ITEM_TRAILER and self.trailer():
                win.play(self.trailer(), from_start=True)
            elif control_id == L.ITEM_ADD_TO_PLAYLIST:
                win.add_to_playlist(d)
            elif control_id == L.ITEM_MORE:
                actions = self.more_actions()
                chosen = xbmcgui.Dialog().contextmenu([name for name, _ in actions]) if actions else -1
                if chosen >= 0:
                    actions[chosen][1]()
            elif control_id == L.ITEM_SEASONS:
                self.pick_season()
            else:
                found = self.rows.selected(control_id)
                if found is not None:
                    _, row, item = found
                    if row["key"] == "episodes":
                        win.open(Item(win, item["id"], item))
                    else:
                        win.open_item(item)
        except api.ApiError as e:
            win.toast(T("server_error") % e.message, 6)

    def menu(self, control_id: int) -> None:
        found = self.rows.selected(control_id)
        if found is not None and found[1]["key"] != "cast":
            self.win.tile_menu(found[2])

    def refresh(self) -> None:
        focus = self.win.getFocusId()
        area = self.area
        self.load()
        self.rows.fill(self.model)
        self.draw()
        if area == "hero":
            self.hero_mode()
            if focus and focus != L.ITEM_PLAY:
                self.win.setFocusId(focus)
        else:
            self.last_focus = focus
            self.restore()


def cast_and_crew(detail: dict) -> list:
    """Cast, then the directors, creators and writers (as people with a job)."""
    jobs = {"director": "Director", "creator": "Creator", "writer": "Writer"}
    found = [dict(c) for c in detail.get("cast") or []]
    for credit in detail.get("crew") or []:
        if credit.get("role") in jobs:
            entry = dict(credit)
            entry["job"] = jobs[credit["role"]]
            found.append(entry)
    return found[:40]


def credits_line(detail: dict) -> str:
    """"Directed by Lena Okafor · Starring Idris Vale, Maren Holt"."""
    crew = detail.get("crew") or []
    makers = [c for c in crew if c.get("role") == "director"] or [c for c in crew if c.get("role") == "creator"]
    stars = (detail.get("cast") or [])[:3]
    parts = []
    if makers:
        made = T("directed_by") if makers[0].get("role") == "director" else T("created_by")
        parts.append("%s %s" % (color(made, L.MUTED), ", ".join(c["name"] for c in makers[:2])))
    if stars:
        parts.append("%s %s" % (color(T("starring"), L.MUTED), ", ".join(c["name"] for c in stars)))
    return color(" · ", L.SUBTLE).join(parts)


class Library(Screen):
    """A library (TvLibraryScreen): Recommended (its rows), Library (every
    title, sorted and filtered, in a grid) and Collections."""

    name = "library"
    backdrop = False

    def __init__(self, win: Window, library: dict, rail_key):
        super().__init__(win)
        self.library = library
        self.rail = rail_key or "lib%d" % next((i for i, lib in enumerate(win.libraries) if lib["id"] == library["id"]), 0)
        self.tab = "all"
        self.sort = "title"
        self.descending = False
        self.watched = None  # all, False: unwatched, True: watched
        self.genre = None
        self.year = None
        self.items = []
        self.total = 0
        self.collections = None
        self.hubs = None
        self.rows = Rows(win, L.LIBRARY_ROWS, L.LIBRARY_SLOTS, ("poster", "landscape", "square"))
        self.filters = None
        self.loading = False

    @property
    def shape(self) -> str:
        return "square" if self.library.get("kind") == "music" else "grid"

    def query(self, offset: int) -> dict:
        q = {"sort": self.sort, "order": "desc" if self.descending else "asc", "offset": offset, "limit": PAGE}
        if self.watched is not None:
            q["watched"] = "true" if self.watched else "false"
        if self.genre:
            q["genre"] = self.genre
        if self.year:
            q["year"] = self.year
        return q

    @property
    def filtered(self) -> bool:
        return self.watched is not None or bool(self.genre) or bool(self.year)

    def chips(self) -> list:
        """(chip, whether it is on, its label when on) for Unwatched, Genre
        and Year."""
        return [
            (L.LIBRARY_UNWATCHED, self.watched is False, T("unwatched")),
            (L.LIBRARY_GENRE, bool(self.genre), self.genre or ""),
            (L.LIBRARY_YEAR, bool(self.year), str(self.year or "")),
        ]

    def refocus(self, chip: int) -> None:
        """Focus back on [chip], or its twin where that is the one shown."""
        on = next(on for c, on, _ in self.chips() if c == chip)
        self.win.setFocusId(chip + 100 if on else chip)

    def smart_filter(self) -> str:
        """The filters and sort as a smart playlist's filter, in Plex's
        syntax (SmartFilter.of): what Save as… saves."""
        parts = []
        if self.genre:
            parts.append(("genre", self.genre))
        if self.year:
            parts.append(("year", str(self.year)))
        if self.watched is not None:
            parts.append(("unwatched", "0" if self.watched else "1"))
        field = PLEX_SORTS[self.sort]
        parts.append(("sort", field + ":desc" if self.descending else field))
        return "&".join("%s=%s" % (quote_plus(k), quote_plus(v)) for k, v in parts)

    def load(self) -> None:
        lib = self.library["id"]
        if self.tab == "all":
            page = self.session.get("/api/libraries/%d/items" % lib, **self.query(0))
            self.items = page.get("items", [])
            self.total = page.get("total", len(self.items))
        elif self.tab == "recommended":
            hubs = self.session.get("/api/libraries/%d/hubs" % lib).get("hubs", [])
            self.hubs = [
                {"key": h["key"], "title": h["title"], "shape": tiles.row_shape(h, self.library.get("kind")), "items": h["items"], "see_all": None}
                for h in hubs
                if h.get("items")
            ]
        else:
            self.collections = self.session.get("/api/libraries/%d/collections" % lib).get("collections", [])

    def show(self, filled: bool) -> None:
        win = self.win
        win.put(L.LIBRARY_TITLE, self.library.get("name"))
        win.prop("pley.library.tab", self.tab)
        music = self.library.get("kind") == "music"
        for tab, id_ in (("recommended", L.LIBRARY_RECOMMENDED), ("all", L.LIBRARY_ALL), ("collections", L.LIBRARY_COLLECTIONS)):
            offered = not (music and tab == "collections")
            win.visible(id_, offered and tab != self.tab)
            win.visible(id_ + 100, offered and tab == self.tab)
        if not filled:
            self.draw()
        if filled and self.last_focus:
            win.setFocusId(self.last_focus)
        else:
            win.setFocusId(self.first())

    def draw(self) -> None:
        win = self.win
        for shape in L.GRID_SHAPES:
            win.visible(L.grid_list(L.LIBRARY_GRID, shape), False)
        win.put(L.LIBRARY_EMPTY, "")
        if self.tab == "all":
            sort_name = T(SORT_NAMES[self.sort])
            win.visible(L.LIBRARY_SORT, not self.descending)
            win.visible(L.LIBRARY_SORT_DOWN, self.descending)
            for control in (L.LIBRARY_SORT, L.LIBRARY_SORT_DOWN):
                win.put(control, ICON_ROOM + sort_name)
            # Music only sorts (TvLibraryScreen's chips); a filter that is on
            # shows its twin, with a check.
            music = self.library.get("kind") == "music"
            win.visible(L.LIBRARY_FILTER, not music)
            win.put(L.LIBRARY_FILTER, ICON_ROOM + T("filter"))
            for chip, on, text in self.chips():
                win.visible(chip, not music and not on)
                win.visible(chip + 100, not music and on)
                win.put(chip + 100, ICON_ROOM + text)
            kind = self.library.get("kind")
            noun = {"movies": ("movie", "movies"), "shows": ("show", "shows"), "music": ("artist", "artists")}.get(kind, ("item", "items"))
            every = self.library.get("item_count")
            if self.filtered and every is not None and every >= self.total:
                # "23 of 342 movies"
                win.put(L.LIBRARY_COUNT, T("count_of") % ("{:,}".format(self.total), words.plural(every, *noun)))
            else:
                win.put(L.LIBRARY_COUNT, words.plural(self.total, *noun))
            grid = win.c(L.grid_list(L.LIBRARY_GRID, self.shape))
            grid.reset()
            grid.addItems([tiles.tile(self.session.base, item, self.shape) for item in self.items])
            win.visible(L.grid_list(L.LIBRARY_GRID, self.shape), bool(self.items))
            if not self.items:
                win.put(L.LIBRARY_EMPTY, T("no_matches") if self.filtered else T("library_empty"))
        elif self.tab == "recommended":
            self.rows.fill(self.hubs or [])
            self.rows.place(None, 200)
            if not self.hubs:
                win.put(L.LIBRARY_EMPTY, T("nothing_yet"))
        else:
            grid = win.c(L.grid_list(L.LIBRARY_GRID, "grid"))
            grid.reset()
            grid.addItems([collection_tile(self.session.base, c) for c in self.collections or []])
            win.visible(L.grid_list(L.LIBRARY_GRID, "grid"), bool(self.collections))
            if not self.collections:
                win.put(L.LIBRARY_EMPTY, T("no_collections"))

    def first(self) -> int:
        if self.tab == "recommended" and self.hubs:
            return L.slot_list(L.LIBRARY_ROWS, 0, self.hubs[0]["shape"])
        grid = L.grid_list(L.LIBRARY_GRID, self.shape if self.tab == "all" else "grid")
        if (self.tab == "all" and self.items) or (self.tab == "collections" and self.collections):
            return grid
        return L.LIBRARY_ALL + (100 if self.tab == "all" else 0)

    def switch(self, tab: str) -> None:
        if tab == self.tab:
            return
        self.tab = tab
        try:
            self.load()
        except api.ApiError as e:
            return self.win.toast(T("server_error") % e.message, 6)
        self.show(False)

    def reload(self) -> None:
        try:
            self.load()
        except api.ApiError as e:
            return self.win.toast(T("server_error") % e.message, 6)
        self.draw()

    def click(self, control_id: int) -> None:
        win = self.win
        tabs = {L.LIBRARY_RECOMMENDED: "recommended", L.LIBRARY_ALL: "all", L.LIBRARY_COLLECTIONS: "collections"}
        if control_id in tabs or control_id - 100 in tabs:
            return self.switch(tabs.get(control_id) or tabs[control_id - 100])
        if control_id in (L.LIBRARY_SORT, L.LIBRARY_SORT_DOWN):
            names = [T(SORT_NAMES[s]) for s in SORTS]
            chosen = xbmcgui.Dialog().select(T("sort_by"), names, preselect=SORTS.index(self.sort))
            if chosen < 0:
                return
            if SORTS[chosen] == self.sort:
                self.descending = not self.descending
            else:
                self.sort = SORTS[chosen]
                self.descending = self.sort not in ("title",)
            self.reload()
            return win.setFocusId(L.LIBRARY_SORT_DOWN if self.descending else L.LIBRARY_SORT)
        if control_id == L.LIBRARY_FILTER:
            return self.filter_menu()
        if control_id in (L.LIBRARY_UNWATCHED, L.LIBRARY_UNWATCHED_ON):
            self.watched = None if self.watched is False else False
            self.reload()
            return self.refocus(L.LIBRARY_UNWATCHED)
        if control_id in (L.LIBRARY_GENRE, L.LIBRARY_GENRE_ON):
            self.pick_genre()
            return self.refocus(L.LIBRARY_GENRE)
        if control_id in (L.LIBRARY_YEAR, L.LIBRARY_YEAR_ON):
            self.pick_year()
            return self.refocus(L.LIBRARY_YEAR)
        if self.tab == "recommended":
            found = self.rows.selected(control_id)
            if found is not None:
                win.open_item(found[2])
            return
        grid = win.c(control_id) if control_id in [L.grid_list(L.LIBRARY_GRID, s) for s in L.GRID_SHAPES] else None
        if grid is None:
            return
        at = grid.getSelectedPosition()
        if self.tab == "all" and 0 <= at < len(self.items):
            win.open_item(self.items[at])
        elif self.tab == "collections" and self.collections and 0 <= at < len(self.collections):
            win.open(Grid.collection(win, self.collections[at]))

    def filter_menu(self) -> None:
        """Filter (TvLibraryPanel's): which titles to show, the genre and
        the year; then Clear filters and Save as…"""
        shows = ((None, T("all")), (False, T("unwatched")), (True, T("mark_watched_short")))
        entries = []
        for watched, name in shows:
            li = xbmcgui.ListItem(name, offscreen=True)
            li.select(self.watched is watched)
            entries.append(li)
        # Kodi's select list draws one label: the value goes in it.
        entries.append(xbmcgui.ListItem("%s: %s" % (T("genre"), self.genre or T("any_genre")), offscreen=True))
        entries.append(xbmcgui.ListItem("%s: %s" % (T("year"), self.year or T("any_year")), offscreen=True))
        if self.filtered:
            entries.append(xbmcgui.ListItem(T("clear_filters"), offscreen=True))
            entries.append(xbmcgui.ListItem(T("save_as"), offscreen=True))
        current = next(i for i, (watched, _) in enumerate(shows) if self.watched is watched)
        chosen = xbmcgui.Dialog().select(T("filter"), entries, preselect=current)
        if chosen < 0:
            return
        if chosen < len(shows):
            self.watched = shows[chosen][0]
            return self.reload()
        if chosen == len(shows):
            return self.pick_genre()
        if chosen == len(shows) + 1:
            return self.pick_year()
        if chosen == len(shows) + 2:
            self.watched, self.genre, self.year = None, None, None
            return self.reload()
        self.save_as()

    def filter_options(self) -> dict:
        if self.filters is None:
            self.filters = self.session.get("/api/libraries/%d/filters" % self.library["id"])
        return self.filters

    def pick_genre(self) -> None:
        try:
            options = self.filter_options().get("genres") or []
        except api.ApiError as e:
            return self.win.toast(T("server_error") % e.message, 6)
        current = options.index(self.genre) + 1 if self.genre in options else 0
        chosen = xbmcgui.Dialog().select(T("genre"), [T("any_genre")] + options, preselect=current)
        if chosen >= 0:
            self.genre = options[chosen - 1] if chosen else None
            self.reload()

    def pick_year(self) -> None:
        try:
            options = [str(y) for y in self.filter_options().get("years") or []]
        except api.ApiError as e:
            return self.win.toast(T("server_error") % e.message, 6)
        current = options.index(str(self.year)) + 1 if str(self.year) in options else 0
        chosen = xbmcgui.Dialog().select(T("year"), [T("any_year")] + options, preselect=current)
        if chosen >= 0:
            self.year = int(options[chosen - 1]) if chosen else None
            self.reload()

    def save_as(self) -> None:
        """Save these filters: a smart playlist of this library that keeps
        up with it by itself; it opens once made."""
        title = xbmcgui.Dialog().input(T("save_filters")).strip()
        if not title:
            return
        body = {"title": title, "smart_filter": self.smart_filter(), "library_id": self.library["id"]}
        try:
            made = self.session.client.post("/api/playlists", body)
        except api.ApiError as e:
            return self.win.toast(T("server_error") % e.message, 6)
        self.win.open(Grid.playlist(self.win, made))

    def moved(self, control_id: int, action: int) -> None:
        if self.tab != "all" or control_id != L.grid_list(L.LIBRARY_GRID, self.shape) or self.loading:
            return
        at = self.win.c(control_id).getSelectedPosition()
        if at >= len(self.items) - 3 * 7 and len(self.items) < self.total:
            self.more()

    def more(self) -> None:
        """The next titles, before focus reaches the grid's end."""
        self.loading = True
        offset = len(self.items)
        lib = self.library["id"]

        def done(page):
            self.loading = False
            found = page.get("items", [])
            if not found or len(self.items) != offset:
                return
            self.items += found
            self.win.c(L.grid_list(L.LIBRARY_GRID, self.shape)).addItems([tiles.tile(self.session.base, item, self.shape) for item in found])

        self.win.later(lambda: self.session.get("/api/libraries/%d/items" % lib, **self.query(offset)), done)

    def menu(self, control_id: int) -> None:
        if self.tab == "all" and control_id == L.grid_list(L.LIBRARY_GRID, self.shape):
            at = self.win.c(control_id).getSelectedPosition()
            if 0 <= at < len(self.items):
                self.win.tile_menu(self.items[at])
        elif self.tab == "recommended":
            found = self.rows.selected(control_id)
            if found is not None:
                self.win.tile_menu(found[2], continue_watching=found[1]["key"].startswith("continue"))

    def refresh(self) -> None:
        focus = self.win.getFocusId()
        at = self.win.c(focus).getSelectedPosition() if focus in [L.grid_list(L.LIBRARY_GRID, s) for s in L.GRID_SHAPES] else -1
        self.load()
        self.draw()
        if at >= 0:
            self.win.setFocusId(focus)
            self.win.c(focus).selectItem(min(at, max(len(self.items) - 1, 0)))


def collection_tile(base: str, collection: dict) -> xbmcgui.ListItem:
    li = xbmcgui.ListItem(collection.get("title") or "", words.plural(collection["item_count"], "title") if collection.get("item_count") is not None else "", offscreen=True)
    # Without a poster of its own, the server sends its titles' (a redirect).
    url = collection.get("poster_url") or "/api/collections/%d/images/poster" % collection["id"]
    found = image(base, url, tiles.WIDTHS["grid"])
    if found:
        li.setArt({"tile": found})
    li.setProperty("ph", words.placeholder(collection.get("title") or ""))
    return li


class Search(Screen):
    """Search (TvSearchScreen): the field, then what was found, by kind."""

    name = "search"
    rail = "search"

    def __init__(self, win: Window):
        super().__init__(win)
        self.query = ""
        self.found = []
        self.rows = Rows(win, L.SEARCH_ROWS, L.SEARCH_SLOTS, ("poster", "episode", "person", "square"))

    def show(self, filled: bool) -> None:
        win = self.win
        win.put(L.SEARCH_FIELD, self.query or color(T("search_hint"), L.MUTED))
        if not filled:
            self.draw()
        win.setFocusId(self.last_focus if filled and self.last_focus else L.SEARCH_FIELD)

    def draw(self) -> None:
        win = self.win
        self.rows.fill(self.found)
        self.rows.place(None, 200)
        if not self.query:
            win.put(L.SEARCH_EMPTY_TITLE, T("search_empty"))
            win.put(L.SEARCH_EMPTY_BODY, T("search_empty_body"))
        elif not self.found:
            win.put(L.SEARCH_EMPTY_TITLE, T("nothing_found_for") % self.query)
            win.put(L.SEARCH_EMPTY_BODY, "")
        else:
            win.put(L.SEARCH_EMPTY_TITLE, "")
            win.put(L.SEARCH_EMPTY_BODY, "")

    def search(self, query: str) -> None:
        self.query = query.strip()
        self.found = []
        if self.query:
            answer = self.session.get("/api/search", q=self.query, limit=100, summaries=1)
            items = answer.get("items", [])
            by_kind = lambda kinds: [i for i in items if i.get("kind") in kinds]  # noqa: E731
            for title, kinds, shape in (
                (T("movies"), ("movie",), "poster"),
                (T("shows"), ("show", "season"), "poster"),
                (T("episodes"), ("episode",), "episode"),
                (T("music"), ("artist", "album", "track"), "square"),
            ):
                found = by_kind(kinds)
                if found:
                    self.found.append({"key": title, "title": title, "shape": shape, "items": found})
            people = answer.get("people") or []
            if people:
                self.found.insert(min(len(self.found), 3), {"key": "people", "title": T("people"), "shape": "person", "items": people})
        self.win.put(L.SEARCH_FIELD, self.query or color(T("search_hint"), L.MUTED))
        self.draw()

    def click(self, control_id: int) -> None:
        win = self.win
        if control_id == L.SEARCH_FIELD:
            typed = xbmcgui.Dialog().input(T("search"), defaultt=self.query)
            if typed is None:
                return
            try:
                self.search(typed)
            except api.ApiError as e:
                return win.toast(T("server_error") % e.message, 6)
            if self.found:
                self.rows.focus(0)
            return
        found = self.rows.selected(control_id)
        if found is not None:
            item = dict(found[2])
            if found[1]["key"] == "people":
                item["kind"] = "person"
            win.open_item(item)

    def focused(self, control_id: int) -> None:
        found = self.rows.find(control_id)
        if found is not None:
            # Under the field, not an info block: rows start at 200.
            y = 200
            for i, row in enumerate(self.rows.rows):
                group = L.slot(L.SEARCH_ROWS, i)
                if i < found[0]:
                    self.win.place(group, 0, -2 * L.HEIGHT)
                    continue
                self.win.place(group, 0, y)
                y += L.row_height(L.SHAPES[row["shape"]]) + L.ROW_GAP
        elif control_id == L.SEARCH_FIELD:
            self.rows.place(None, 200)

    def menu(self, control_id: int) -> None:
        found = self.rows.selected(control_id)
        if found is not None and found[1]["key"] != "people":
            self.win.tile_menu(found[2])

    def refresh(self) -> None:
        if self.query:
            self.search(self.query)


class Grid(Screen):
    """A page of tiles: playlists, a playlist, collections, a collection,
    a person's titles."""

    name = "grid"

    def __init__(self, win: Window, title: str, loader, shape: str = "grid", rail=None, opener=None, empty: str = "", subtitle: str = ""):
        super().__init__(win)
        self.title = title
        self.loader = loader
        self.shape = shape
        self.rail = rail
        self.opener = opener
        self.empty = empty
        self.subtitle = subtitle
        self.entries = []
        self.items = []

    @classmethod
    def playlists(cls, win: Window):
        def load():
            return win.session.get("/api/playlists").get("playlists", [])

        def tile(playlist):
            li = xbmcgui.ListItem(playlist.get("title") or "", T("playlist_items") % playlist.get("item_count", 0), offscreen=True)
            artwork = playlist.get("artwork") or []
            if artwork:
                li.setArt({"tile": image(win.session.base, artwork[0], tiles.WIDTHS["wide"])})
            li.setProperty("ph", words.placeholder(playlist.get("title") or ""))
            return li

        return cls(win, T("playlists"), (load, tile), "wide", "playlists", lambda p: win.open(Grid.playlist(win, p)), T("no_playlists"))

    @classmethod
    def playlist(cls, win: Window, playlist: dict):
        def load():
            entries = win.session.get("/api/playlists/%d" % playlist["id"], summaries=1).get("entries", [])
            return [e["item"] for e in entries]

        return cls(win, playlist.get("title") or "", (load, None), "wide", opener=win.open_item)

    @classmethod
    def collections(cls, win: Window):
        def load():
            found = []
            videos = [lib for lib in win.libraries if lib.get("kind") in ("movies", "shows")]
            answers = api.together(*[lambda lib=lib: win.session.get("/api/libraries/%d/collections" % lib["id"]) for lib in videos]) if videos else []
            for answer in answers:
                found += answer.get("collections", [])
            return sorted(found, key=lambda c: (c.get("title") or "").lower())

        return cls(win, T("collections"), (load, lambda c: collection_tile(win.session.base, c)), "grid", "collections", lambda c: win.open(Grid.collection(win, c)), T("no_collections"))

    @classmethod
    def collection(cls, win: Window, collection: dict):
        def load():
            return win.session.get("/api/collections/%d" % collection["id"], summaries=1).get("items", [])

        return cls(win, collection.get("title") or "", (load, None), "grid", opener=win.open_item, subtitle=collection.get("summary") or "")

    @classmethod
    def person(cls, win: Window, person: dict):
        def load():
            found = win.session.get("/api/people/%d" % int(person["id"]))
            return [credit["item"] for credit in found.get("credits", [])]

        return cls(win, person.get("name") or person.get("title") or "", (load, None), "grid", opener=win.open_item, subtitle=T("known_for"))

    def load(self) -> None:
        self.entries = self.loader[0]()
        if self.shape == "wide" and self.loader[1] is None and self.entries:
            kinds = {e.get("kind") for e in self.entries}
            self.shape = "wide" if kinds <= {"episode", "extra"} else "square" if kinds <= {"track", "album", "artist"} else "grid"

    def show(self, filled: bool) -> None:
        win = self.win
        win.put(L.GRID_TITLE, self.title)
        win.put(L.GRID_SUBTITLE, clamp(self.subtitle, 27, 1500, 1))
        if not filled:
            self.draw()
        win.setFocusId(self.last_focus if filled and self.last_focus else (L.grid_list(L.GRID_LISTS, self.shape) if self.entries else L.RAIL_HOME))

    def draw(self) -> None:
        win = self.win
        for shape in L.GRID_SHAPES:
            win.visible(L.grid_list(L.GRID_LISTS, shape), False)
        grid = win.c(L.grid_list(L.GRID_LISTS, self.shape))
        grid.reset()
        make = self.loader[1] or (lambda item: tiles.tile(self.session.base, item, self.shape))
        grid.addItems([make(entry) for entry in self.entries])
        grid.setVisible(bool(self.entries))
        win.put(L.GRID_EMPTY, "" if self.entries else self.empty)

    def click(self, control_id: int) -> None:
        if control_id != L.grid_list(L.GRID_LISTS, self.shape):
            return
        at = self.win.c(control_id).getSelectedPosition()
        if 0 <= at < len(self.entries) and self.opener:
            self.opener(self.entries[at])

    def menu(self, control_id: int) -> None:
        if control_id == L.grid_list(L.GRID_LISTS, self.shape) and self.loader[1] is None:
            at = self.win.c(control_id).getSelectedPosition()
            if 0 <= at < len(self.entries):
                self.win.tile_menu(self.entries[at])

    def refresh(self) -> None:
        focus = self.win.getFocusId()
        grid = L.grid_list(L.GRID_LISTS, self.shape)
        at = self.win.c(grid).getSelectedPosition() if focus == grid else -1
        self.load()
        self.draw()
        if at >= 0 and self.entries:
            self.win.setFocusId(grid)
            self.win.c(grid).selectItem(min(at, len(self.entries) - 1))


class Notice(Screen):
    """A sentence instead of a page (TvHomeNotice): the server out of
    reach, nothing in the libraries yet."""

    name = "notice"

    def __init__(self, win: Window, title: str, body: str, retry=None, sign_in: bool = False):
        super().__init__(win)
        self.title = title
        self.body = body
        self.retry = retry
        self.sign_in = sign_in

    def show(self, filled: bool) -> None:
        win = self.win
        win.put(L.NOTICE_TITLE, self.title)
        win.put(L.NOTICE_BODY, self.body)
        first = T("sign_in") if self.sign_in else T("try_again")
        win.put(L.NOTICE_FIRST, ICON_ROOM + first)
        win.put(L.NOTICE_SECOND, T("choose_server"))
        win.visible(L.NOTICE_FIRST, self.retry is not None or self.sign_in)
        win.setFocusId(L.NOTICE_FIRST if (self.retry is not None or self.sign_in) else L.NOTICE_SECOND)

    def first(self) -> int:
        return L.NOTICE_FIRST

    def click(self, control_id: int) -> None:
        win = self.win
        if control_id == L.NOTICE_FIRST and self.sign_in:
            return win.leave_for("sign_in")
        if control_id == L.NOTICE_FIRST and self.retry is not None:
            screen = type(self.retry)(win) if isinstance(self.retry, (Home, Search)) else self.retry
            if not win.libraries:
                try:
                    win.me, win.libraries = api.together(lambda: win.session.get("/api/auth/me"), lambda: win.session.get("/api/libraries"))
                except api.ApiError as e:
                    return win.toast(T("server_error") % e.message, 6)
                win.setup_rail()
            win.stack = []
            win.screen = None
            return win.open(screen)
        if control_id == L.NOTICE_SECOND:
            win.leave_for("choose_server")


def run(plugin, base: str) -> None:
    """Opens pley's window until it is closed. Choosing a server and signing
    in, asked from it, happen with it closed, and it opens again after
    when the add-on is signed in."""
    while True:
        window = Window(L.WINDOW, kodi.addon_dir(), "default", "1080i")
        window.session = Session(base)
        window.plugin = plugin
        window.doModal()
        after = window.after
        del window
        kodi.clear_property(UI)
        if after == "choose_server":
            plugin.route_choose_server()
        elif after == "sign_in":
            plugin.route_sign_in()
        elif after == "sign_out":
            plugin.route_sign_out()
            return
        else:
            return
        if not plugin.signed_in():
            return
