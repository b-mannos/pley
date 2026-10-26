"""The plugin: pley's screens as Kodi listings, and what their items do.

Every call to the server goes through the service's proxy, which holds the
sign-in. Only signing in talks to the server directly, before there are
tokens.
"""

from __future__ import annotations

import json
import os
import sys
import threading
import time
import urllib.parse

import xbmc
import xbmcgui
import xbmcplugin

from . import api, auth, caps, discovery, kodi, listing
from .api import path_in

PAGE = 300
# The quality setting's choices, as the web player's (apps/web/src/player/quality.ts).
QUALITIES = [(None, None), (30000, 2160), (20000, 1080), (15000, 2160), (12000, 1080), (8000, 1080), (4000, 720), (2000, 480)]


class Plugin:
    def __init__(self, argv):
        self.base_url = argv[0]
        self.handle = int(argv[1]) if len(argv) > 1 and argv[1].lstrip("-").isdigit() else -1
        query = argv[2][1:] if len(argv) > 2 and argv[2].startswith("?") else ""
        self.params = dict(urllib.parse.parse_qsl(query))
        self.resume = len(argv) > 3 and argv[3] == "resume:true"
        self.proxy_base = None

    # ---------------------------------------------------------------- helpers

    def url(self, **params) -> str:
        return self.base_url + "?" + urllib.parse.urlencode(params)

    def proxy(self) -> str:
        """The proxy's address, waiting a moment for a service that is just
        starting."""
        for _ in range(30):
            base = kodi.get_property(kodi.PROXY)
            if base:
                return base
            xbmc.sleep(100)
        raise api.ApiError(0, "service_off", kodi.text("service_off"))

    def client(self) -> api.Client:
        if not self.proxy_base:
            self.proxy_base = self.proxy()
        return api.Client(self.proxy_base, kodi.user_agent(), timeout=60)

    def status(self) -> dict:
        client = self.client()
        try:
            return client.get("/status")
        except api.ApiError:
            return {"signed_in": False, "server": None}

    def folder(self, title: str, params: dict, icon: str = "DefaultFolder.png") -> None:
        li = xbmcgui.ListItem(title, offscreen=True)
        li.setArt({"icon": icon, "thumb": icon})
        xbmcplugin.addDirectoryItem(self.handle, self.url(**params), li, isFolder=True)

    def action(self, title: str, params: dict, icon: str = "DefaultAddonService.png") -> None:
        li = xbmcgui.ListItem(title, offscreen=True)
        li.setArt({"icon": icon, "thumb": icon})
        xbmcplugin.addDirectoryItem(self.handle, self.url(**params), li, isFolder=False)

    def items(self, items: list, content: str | None = None) -> None:
        base = self.proxy_base
        entries = []
        for item in items:
            li = listing.list_item(base, item)
            li.addContextMenuItems(self.menu(item))
            if item["kind"] in listing.PLAYABLE:
                # The resume point goes along, so playing needs no details first.
                position = (item.get("user_state") or {}).get("position_ms") or 0
                entries.append((self.url(route="play", id=item["id"], pos=position), li, False))
            else:
                entries.append((self.url(route="item", id=item["id"]), li, True))
        xbmcplugin.addDirectoryItems(self.handle, entries, len(entries))
        kinds = {item["kind"] for item in items}
        if content is None and len(kinds) == 1:
            content = listing.CONTENT.get(kinds.pop())
        if content:
            xbmcplugin.setContent(self.handle, content)

    def menu(self, item: dict) -> list:
        entries = []
        if item["kind"] not in listing.MUSIC:
            watched = (item.get("user_state") or {}).get("watched")
            name, value = ("mark_unwatched", "0") if watched else ("mark_watched", "1")
            entries.append((kodi.text(name), "RunPlugin(%s)" % self.url(route="watched", id=item["id"], value=value)))
        if item["kind"] in ("movie", "show"):
            if self.params.get("key") == "my_list":
                entries.append((kodi.text("remove_from_list"), "RunPlugin(%s)" % self.url(route="my_list", id=item["id"], value="0")))
            else:
                entries.append((kodi.text("add_to_list"), "RunPlugin(%s)" % self.url(route="my_list", id=item["id"], value="1")))
        return entries

    def end(self, ok: bool = True, cache: bool = False) -> None:
        xbmcplugin.endOfDirectory(self.handle, succeeded=ok, cacheToDisc=cache)

    # ----------------------------------------------------------------- routes

    def run(self) -> None:
        route = self.params.get("route", "root")
        handler = getattr(self, "route_" + route, None)
        if handler is None:
            kodi.log("unknown route " + route, xbmc.LOGWARNING)
            return
        try:
            handler()
        except api.ApiError as e:
            kodi.log("%s: %s %s" % (route, e.code, e.message), xbmc.LOGWARNING)
            listing_route = self.handle >= 0 and route not in ACTIONS
            if e.code == "unreachable" and listing_route:
                # The server is off or out of reach (the internet needn't be
                # there: pley works on the home network alone).
                return self.unreachable()
            if e.code == "unreachable":
                message = kodi.text("cant_reach") % self.server_name()
            elif e.code == "signed_out":
                message = kodi.text("signed_out")
            elif e.code == "service_off":
                message = e.message
            else:
                message = kodi.text("server_error") % e.message
            kodi.notify(message, error=True)
            if route == "play":
                xbmcplugin.setResolvedUrl(self.handle, False, xbmcgui.ListItem())
            elif listing_route:
                self.end(False)

    def server_name(self) -> str:
        return kodi.setting("server_name") or server_setting()

    def unreachable(self) -> None:
        """What a listing shows when the server doesn't answer."""
        li = xbmcgui.ListItem(kodi.text("cant_reach") % self.server_name(), offscreen=True)
        li.setArt({"icon": "DefaultIconError.png", "thumb": "DefaultIconError.png"})
        li.getVideoInfoTag().setPlot(kodi.text("cant_reach_hint"))
        xbmcplugin.addDirectoryItem(self.handle, self.url(route="retry"), li, isFolder=False)
        self.action(kodi.text("try_again"), {"route": "retry"}, "DefaultAddonsUpdates.png")
        self.action(kodi.text("choose_server"), {"route": "choose_server"}, "DefaultNetwork.png")
        self.end()

    def route_retry(self) -> None:
        xbmc.executebuiltin("Container.Refresh")

    def route_root(self) -> None:
        server = server_setting()
        name = kodi.setting("server_name") or server
        if not server:
            self.action(kodi.text("choose_server"), {"route": "choose_server"}, "DefaultNetwork.png")
            return self.end()
        status = self.status()
        if not status.get("signed_in") or status.get("server") != server:
            self.action(kodi.text("sign_in_to") % name, {"route": "sign_in"}, "DefaultUser.png")
            self.action(kodi.text("choose_server"), {"route": "choose_server"}, "DefaultNetwork.png")
            return self.end()
        client = self.client()
        for hub in client.get("/api/hubs/home").get("hubs", []):
            self.folder(hub["title"], {"route": "hub", "key": hub["key"]}, "DefaultRecentlyAddedMovies.png")
        icons = {"movies": "DefaultMovies.png", "shows": "DefaultTVShows.png", "music": "DefaultMusicAlbums.png"}
        for library in client.get("/api/libraries"):
            params = {"route": "library", "id": library["id"], "kind": library["kind"]}
            self.folder(library["name"], params, icons.get(library["kind"], "DefaultFolder.png"))
        self.folder(kodi.text("playlists"), {"route": "playlists"}, "DefaultPlaylist.png")
        self.folder(kodi.text("search"), {"route": "search"}, "DefaultAddonsSearch.png")
        self.action(kodi.text("settings"), {"route": "settings"})
        self.end()

    def route_hub(self) -> None:
        key = self.params["key"]
        hubs = self.client().get("/api/hubs/home").get("hubs", [])
        hub = next((h for h in hubs if h["key"] == key), None)
        self.items(hub["items"] if hub else [])
        self.end()

    def route_library(self) -> None:
        library, kind = self.params["id"], self.params.get("kind")
        offset = int(self.params.get("offset", 0))
        client = self.client()
        if offset == 0 and kind in ("movies", "shows"):
            collections = client.get("/api/libraries/%s/collections" % library).get("collections", [])
            if collections:
                self.folder(kodi.text("collections"), {"route": "collections", "id": library}, "DefaultSets.png")
        page = client.get("/api/libraries/%s/items" % library, sort="title", offset=offset, limit=PAGE)
        self.items(page.get("items", []))
        if offset + PAGE < page.get("total", 0):
            params = {"route": "library", "id": library, "kind": kind, "offset": offset + PAGE}
            self.folder(kodi.text("next_page"), params, "DefaultFolder.png")
        xbmcplugin.addSortMethod(self.handle, xbmcplugin.SORT_METHOD_UNSORTED)
        xbmcplugin.addSortMethod(self.handle, xbmcplugin.SORT_METHOD_VIDEO_YEAR)
        self.end()

    def route_item(self) -> None:
        children = self.client().get("/api/items/%s/children" % self.params["id"]).get("items", [])
        self.items(children)
        if children and children[0]["kind"] in ("episode", "track"):
            xbmcplugin.addSortMethod(self.handle, xbmcplugin.SORT_METHOD_UNSORTED)
            xbmcplugin.addSortMethod(self.handle, xbmcplugin.SORT_METHOD_LABEL)
        self.end()

    def route_collections(self) -> None:
        client = self.client()
        for collection in client.get("/api/libraries/%s/collections" % self.params["id"]).get("collections", []):
            li = xbmcgui.ListItem(collection["title"], offscreen=True)
            li.setArt(
                {
                    "poster": listing.image(self.proxy_base, collection.get("poster_url"), 500),
                    "fanart": listing.image(self.proxy_base, collection.get("backdrop_url"), 1920),
                }
            )
            li.getVideoInfoTag().setPlot(collection.get("summary") or "")
            xbmcplugin.addDirectoryItem(self.handle, self.url(route="collection", id=collection["id"]), li, isFolder=True)
        xbmcplugin.setContent(self.handle, "sets")
        self.end()

    def route_collection(self) -> None:
        self.items(self.client().get("/api/collections/%s" % self.params["id"]).get("items", []))
        self.end()

    def route_playlists(self) -> None:
        for playlist in self.client().get("/api/playlists").get("playlists", []):
            li = xbmcgui.ListItem("%s (%d)" % (playlist["title"], playlist.get("item_count", 0)), offscreen=True)
            artwork = playlist.get("artwork") or []
            if artwork:
                li.setArt({"thumb": listing.image(self.proxy_base, artwork[0], 500)})
            xbmcplugin.addDirectoryItem(self.handle, self.url(route="playlist", id=playlist["id"]), li, isFolder=True)
        self.end()

    def route_playlist(self) -> None:
        entries = self.client().get("/api/playlists/%s" % self.params["id"]).get("entries", [])
        self.items([entry["item"] for entry in entries])
        self.end()

    def route_search(self) -> None:
        query = self.params.get("q")
        if not query:
            query = xbmcgui.Dialog().input(kodi.text("search"))
            if not query:
                return self.end(False)
        found = self.client().get("/api/search", q=query, limit=100)
        items = found.get("items", [])
        if not items:
            kodi.notify(kodi.text("nothing_found"))
        self.items(items, content="videos")
        self.end()

    def route_play(self) -> None:
        started = time.monotonic()
        client = self.client()
        item_id = int(self.params["id"])
        listed = self.params.get("pos")
        details = {}
        loader = None
        if self.resume and listed is None:
            # Not from a listing: the resume point is in the details.
            details["item"] = client.get("/api/items/%d" % item_id)
            start_ms = (details["item"].get("user_state") or {}).get("position_ms", 0)
        else:
            # The details (for Kodi's info) come while the server decides.
            start_ms = int(listed or 0) if self.resume else 0
            loader = threading.Thread(target=load_details, args=(client, item_id, details), daemon=True)
            loader.start()
        max_bitrate, max_height = QUALITIES[quality_index()]
        request = {
            "item_id": item_id,
            "start_ms": start_ms,
            "client_id": install_id(),
            "caps": report(),
            "max_bitrate": max_bitrate,
            "max_height": max_height,
        }
        info = client.post("/api/playback", request)
        if loader:
            loader.join(10)
        detail = details.get("item")
        url = path_in(self.proxy_base, info["url"])
        if detail:
            li = listing.list_item(self.proxy_base, detail)
            listing.video_details(li, detail, self.proxy_base)
        else:
            li = xbmcgui.ListItem(offscreen=True)
        li.setPath(url)
        if info.get("hls"):
            li.setMimeType("application/vnd.apple.mpegurl")
            li.setContentLookup(False)
            # Text subtitles come beside a stream that carries none.
            subtitles = [path_in(self.proxy_base, s["url"]) for s in info.get("subtitles", []) if s.get("url")]
            if subtitles:
                li.setSubtitles(subtitles)
        else:
            # Said here, Kodi needn't ask the server for the file's type first.
            mime = file_mime(detail, info.get("file_id"))
            if mime:
                li.setMimeType(mime)
                li.setContentLookup(False)
        playing = {
            "item_id": item_id,
            "file_id": info.get("file_id"),
            "session_id": info["session_id"],
            "duration_ms": info.get("duration_ms") or (detail or {}).get("duration_ms") or 0,
            "path": url,
        }
        kodi.set_property(kodi.PLAYING, json.dumps(playing))
        reason = info.get("reason")
        elapsed = (time.monotonic() - started) * 1000
        kodi.log("playing %d by %s, ready in %d ms%s" % (item_id, info.get("method"), elapsed, ": " + reason if reason else ""))
        xbmcplugin.setResolvedUrl(self.handle, True, li)

    # ---------------------------------------------------------------- actions

    def route_watched(self) -> None:
        verb = "watched" if self.params.get("value") == "1" else "unwatched"
        self.client().post("/api/items/%s/%s" % (self.params["id"], verb))
        xbmc.executebuiltin("Container.Refresh")

    def route_my_list(self) -> None:
        path = "/api/watchlist/%s" % self.params["id"]
        if self.params.get("value") == "1":
            self.client().put(path)
        else:
            self.client().delete(path)
        xbmc.executebuiltin("Container.Refresh")

    def route_settings(self) -> None:
        kodi.addon().openSettings()

    def route_choose_server(self) -> None:
        dialog = xbmcgui.Dialog()
        progress = xbmcgui.DialogProgressBG()
        progress.create("pley", kodi.text("looking"))
        try:
            servers = discovery.find()
        finally:
            progress.close()
        names = ["%s (%s)" % (s["name"], s["url"].split("://", 1)[1]) for s in servers]
        chosen = dialog.select(kodi.text("choose_server"), names + [kodi.text("enter_address")])
        if chosen < 0:
            return
        if chosen < len(servers):
            server, name, server_id = servers[chosen]["url"], servers[chosen]["name"], servers[chosen]["id"]
        else:
            typed = dialog.input(kodi.text("enter_address"), defaultt=server_setting())
            if not typed:
                return
            try:
                server = api.normalize(typed)
            except ValueError:
                return dialog.ok("pley", kodi.text("not_pley") % typed)
            name, server_id = server.split("://", 1)[1], ""
        try:
            auth.metadata(server, kodi.user_agent())
        except api.ApiError as e:
            kodi.log("%s isn't a pley server: %s" % (server, e.message), xbmc.LOGWARNING)
            message = kodi.text("not_reachable") if e.status == 0 and e.code == "unreachable" else kodi.text("not_pley")
            return dialog.ok("pley", message % server)
        if server != server_setting():
            self.sign_out_quietly()
        kodi.set_setting("server_url", server)
        kodi.set_setting("server_name", name)
        kodi.set_setting("server_id", server_id)
        self.sign_in(server, name, server_id)

    def route_sign_in(self) -> None:
        server = server_setting()
        if not server:
            return self.route_choose_server()
        self.sign_in(server, kodi.setting("server_name") or server, kodi.setting("server_id"))

    def route_sign_out(self) -> None:
        self.sign_out_quietly()
        kodi.notify(kodi.text("signed_out"))
        xbmc.executebuiltin("Container.Refresh")

    def sign_out_quietly(self) -> None:
        try:
            self.client().request("POST", "/sign-out")
        except api.ApiError:
            pass

    def sign_in(self, server: str, name: str, server_id: str) -> None:
        dialog = xbmcgui.Dialog()
        flow = auth.DeviceSignIn(server, kodi.user_agent())
        started = flow.start()
        progress = xbmcgui.DialogProgress()
        steps = kodi.text("sign_in_steps") % (started["verification_uri"], started["user_code"])
        progress.create(kodi.text("sign_in_to") % name, steps)
        expires = time.time() + int(started.get("expires_in") or 600)
        try:
            while True:
                waited = 0.0
                while waited < flow.interval:
                    if progress.iscanceled():
                        return
                    xbmc.sleep(250)
                    waited += 0.25
                left = max(expires - time.time(), 0)
                progress.update(int(100 * left / max(int(started.get("expires_in") or 600), 1)), steps)
                state, tokens = flow.poll(started["device_code"])
                if state == "allowed":
                    store = auth.Store(os.path.join(kodi.profile_dir(), "tokens.json"))
                    store.save(flow.record(tokens, name, server_id))
                    me = api.Client(server, kodi.user_agent(), tokens["access_token"]).get("/api/auth/me")
                    kodi.notify(kodi.text("signed_in_as") % me.get("name", me.get("username", "")))
                    xbmc.executebuiltin("Container.Refresh")
                    return
                if state == "denied":
                    progress.close()
                    return dialog.ok("pley", kodi.text("code_refused"))
                if state == "expired" or left <= 0:
                    progress.close()
                    return dialog.ok("pley", kodi.text("code_expired"))
        finally:
            progress.close()


# Routes that run as actions, with no listing to end.
ACTIONS = ("play", "watched", "my_list", "settings", "choose_server", "sign_in", "sign_out", "retry")


MIME = {
    "mkv": "video/x-matroska",
    "matroska": "video/x-matroska",
    "webm": "video/webm",
    "mp4": "video/mp4",
    "m4v": "video/mp4",
    "mov": "video/quicktime",
    "avi": "video/x-msvideo",
    "ts": "video/mp2t",
    "m2ts": "video/mp2t",
    "mpegts": "video/mp2t",
    "mp3": "audio/mpeg",
    "flac": "audio/flac",
    "m4a": "audio/mp4",
    "ogg": "audio/ogg",
    "opus": "audio/ogg",
    "wav": "audio/wav",
}


def file_mime(detail, file_id):
    """The type of a file played as it is, from its container (None when
    unknown, and Kodi asks)."""
    for file in (detail or {}).get("files") or []:
        if file.get("id") == file_id:
            return MIME.get((file.get("container") or "").lower())
    return None


def load_details(client: api.Client, item_id: int, into: dict) -> None:
    try:
        into["item"] = client.get("/api/items/%d" % item_id)
    except api.ApiError as e:
        kodi.log("details of %d: %s" % (item_id, e.message), xbmc.LOGWARNING)


def quality_index() -> int:
    return min(max(kodi.setting_int("quality"), 0), len(QUALITIES) - 1)


def server_setting() -> str:
    """The chosen server's address, as pley writes it ("" when there is
    none, or it was typed into the settings as something else)."""
    try:
        return api.normalize(kodi.setting("server_url")) if kodi.setting("server_url").strip() else ""
    except ValueError:
        return ""


def install_id() -> str:
    """This Kodi, as pley's sessions know it: the same player from one
    playback to the next."""
    path = os.path.join(kodi.profile_dir(), "install_id")
    try:
        with open(path, encoding="utf-8") as f:
            found = f.read().strip()
            if found:
                return found
    except OSError:
        pass
    made = "kodi-" + os.urandom(16).hex()
    with open(path, "w", encoding="utf-8") as f:
        f.write(made)
    return made


def report() -> dict:
    settings = {
        name: kodi.setting_value(name)
        for name in ["audiooutput.passthrough", "audiooutput.channels"] + [n for n, _ in caps.PASSTHROUGH]
    }
    screen = (int(xbmc.getInfoLabel("System.ScreenWidth") or 0), int(xbmc.getInfoLabel("System.ScreenHeight") or 0))
    model = "Kodi %s on %s" % (kodi.kodi_version(), kodi.platform())
    return caps.build(xbmc.getInfoLabel("System.SupportedHDRTypes"), settings, screen, model, kodi.is_coreelec())


def run(argv=None) -> None:
    Plugin(argv or sys.argv).run()
