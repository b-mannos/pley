"""The service: it holds the sign-in, runs the proxy, and tells the server
where playback is (docs/API.md, Playback): every 10 seconds while playing,
on pause, and when it stops, after which the session ends."""

from __future__ import annotations

import json
import os
import threading
import time

import xbmc

from . import api, auth, kodi, overlay, prompts
from .proxy import Proxy, new_secret

REPORT_EVERY = 10


class Reports:
    """Progress reports for one playback, by the rules above. `send(path,
    body)` and `stop(path)` reach the server."""

    def __init__(self, playing: dict, send, end, clock=time.monotonic):
        self.playing = playing
        self.send = send
        self.end = end
        self.clock = clock
        self.position_ms = 0
        self.last = None
        self.paused = False

    def body(self, state: str) -> dict:
        return {
            "position_ms": int(self.position_ms),
            "duration_ms": int(self.playing.get("duration_ms") or 0),
            "state": state,
            "session_id": self.playing["session_id"],
        }

    def report(self, state: str) -> None:
        self.last = self.clock()
        self.send("/api/items/%d/progress" % self.playing["item_id"], self.body(state))

    def at(self, position_ms: float) -> None:
        """Playback is at `position_ms`: reported once 10 seconds went by."""
        self.position_ms = position_ms
        if not self.paused and (self.last is None or self.clock() - self.last >= REPORT_EVERY):
            self.report("playing")

    def pause(self) -> None:
        self.paused = True
        self.report("paused")

    def resume(self) -> None:
        self.paused = False
        self.report("playing")

    def finish(self, ended: bool) -> None:
        if ended and self.playing.get("duration_ms"):
            self.position_ms = self.playing["duration_ms"]
        try:
            self.report("stopped")
        finally:
            self.end("/api/playback/%s" % self.playing["session_id"])


class Player(xbmc.Player):
    def __init__(self, tokens: auth.Tokens, user_agent: str):
        super().__init__()
        self.tokens = tokens
        self.user_agent = user_agent
        self.lock = threading.Lock()
        self.reports = None
        self.moments = None
        # Up Next after an episode that ended on its own.
        self.after_end = None
        self.overlay = None
        self.shown = None
        # The button over the video, which the loop and a press both change.
        self.ui = threading.RLock()

    def call(self, method: str, path: str, body=None):
        """One request to the server, the token renewed once if refused: its
        answer, or None when signed out."""
        server = self.tokens.server()
        if not server:
            return None
        token = self.tokens.access()
        for attempt in range(2):
            client = api.Client(server, self.user_agent, token)
            response = client.request(method, path, json_body=body)
            if response.status != 401 or attempt:
                if response.status >= 400 and response.status != 410:
                    kodi.log("%s %s: %d" % (method, path, response.status), xbmc.LOGWARNING)
                return response
            token = self.tokens.refreshed(token)
        return None

    def safely(self, method: str, path: str, body=None):
        try:
            return self.call(method, path, body)
        except (api.ApiError, auth.SignedOut) as e:
            kodi.log("%s %s failed: %s" % (method, path, e), xbmc.LOGWARNING)
            return None

    def fetch(self, path: str) -> dict:
        response = self.safely("GET", path)
        if response is None or response.status != 200:
            return {}
        return response.json() or {}

    def onAVStarted(self) -> None:
        raw = kodi.get_property(kodi.PLAYING)
        if not raw:
            return
        try:
            playing = json.loads(raw)
            file = self.getPlayingFile()
        except (ValueError, RuntimeError):
            return
        if file.split("|")[0] != playing.get("path"):
            return
        kodi.clear_property(kodi.PLAYING)
        with self.lock:
            self.after_end = None
            self.reports = Reports(
                playing,
                lambda path, body: self.safely("POST", path, body),
                lambda path: self.safely("DELETE", path),
            )
            self.reports.at(self.position())
        # The markers of the file playing, and the episode after it.
        detail = self.fetch("/api/items/%d" % playing["item_id"])
        markers = [m for m in detail.get("markers", []) if m.get("file_id") == playing.get("file_id")]
        following = None
        if detail.get("kind") == "episode":
            following = self.fetch("/api/items/%d/next" % playing["item_id"]).get("item")
        with self.lock:
            if self.reports and self.reports.playing is playing:
                self.moments = prompts.Moments(markers, following)

    def position(self) -> float:
        try:
            return self.getTime() * 1000
        except RuntimeError:
            return 0

    def onPlayBackPaused(self) -> None:
        with self.lock:
            if self.reports:
                self.reports.position_ms = self.position()
                self.reports.pause()

    def onPlayBackResumed(self) -> None:
        with self.lock:
            if self.reports:
                self.reports.resume()

    def onPlayBackStopped(self) -> None:
        self.finish(ended=False)

    def onPlayBackEnded(self) -> None:
        self.finish(ended=True)

    def onPlayBackError(self) -> None:
        self.finish(ended=False)

    def finish(self, ended: bool) -> None:
        with self.lock:
            if self.reports is None or self.playing_other(self.reports):
                return
            reports, self.reports = self.reports, None
            moments, self.moments = self.moments, None
            if ended and moments and moments.next_item and not moments.next_cancelled:
                self.after_end = moments
        reports.finish(ended)
        # Listings show the new resume point and watched marks.
        xbmc.executebuiltin("Container.Refresh")

    def playing_other(self, reports: Reports) -> bool:
        """Whether something else plays already: a stop Kodi tells of late,
        about the item before."""
        try:
            return self.isPlaying() and self.getPlayingFile().split("|")[0] != reports.playing["path"]
        except RuntimeError:
            return False

    def tick(self) -> None:
        """Once a second, from the service's loop: progress, and the button
        over the video."""
        with self.lock:
            playing = self.reports is not None and self.isPlaying()
            position = self.position() if playing else 0
            if playing:
                self.reports.at(position)
            moments = self.moments if playing else self.after_end
        with self.ui:
            self.offer(moments, playing, position)

    def offer(self, moments, playing: bool, position: float) -> None:
        if moments is None:
            return self.close_overlay()
        if self.overlay is not None and self.overlay.choice:
            return self.chosen(moments)
        offer = moments.offer(position) if playing else ("next", None)
        if offer is None:
            moments.next_since = None
            return self.close_overlay()
        kind, marker = offer
        if kind == "skip":
            name = "skip_intro" if marker["kind"] == "intro" else "skip_credits"
            return self.show(("skip", prompts.key(marker), marker), "", kodi.text(name))
        left = moments.counting()
        if left == 0:
            return self.play_next(moments.next_item)
        title = kodi.text("up_next") % prompts.episode_line(moments.next_item)
        self.show(("next",), title, kodi.text("play_now") % left, kodi.text("cancel"))

    def pressed(self, which: overlay.Overlay) -> None:
        """A button was pressed: acted on now, not on the loop's next turn."""
        with self.ui:
            if which is not self.overlay:
                return
            with self.lock:
                moments = self.moments if self.reports is not None else self.after_end
            if moments is not None:
                self.chosen(moments)

    def chosen(self, moments: prompts.Moments) -> None:
        choice, shown = self.overlay.choice, self.shown
        self.close_overlay()
        if shown[0] == "skip":
            moments.dismiss(shown[2])
            if choice == "accept":
                self.seekTime(prompts.target_ms(shown[2]) / 1000.0)
        elif choice == "accept":
            self.play_next(moments.next_item)
        else:
            moments.cancel_next()
            self.after_end = None

    def show(self, what: tuple, title: str, accept: str, cancel: str = "") -> None:
        if self.overlay is not None and self.shown[:2] != what[:2]:
            self.close_overlay()
        if self.overlay is None:
            self.overlay = overlay.Overlay("pley-overlay.xml", kodi.addon_dir(), "default", "1080i")
            self.overlay.on_choice = self.pressed
            self.overlay.set_texts(title, accept, cancel)
            self.overlay.show()
        else:
            self.overlay.set_texts(title, accept, cancel)
        self.shown = what

    def close_overlay(self) -> None:
        if self.overlay is not None:
            self.overlay.close()
            self.overlay = None
            self.shown = None

    def play_next(self, item: dict) -> None:
        """Ends this playback where it is, and plays `item`."""
        self.close_overlay()
        with self.lock:
            reports, self.reports = self.reports, None
            self.moments = None
            self.after_end = None
        if reports:
            reports.finish(ended=False)
        self.play("plugin://%s/?route=play&id=%d" % (kodi.ADDON_ID, item["id"]))


def proxy_config(profile: str) -> dict:
    """The proxy's port and secret, kept so that the addresses Kodi caches
    artwork by stay the same from one start to the next."""
    path = os.path.join(profile, "proxy.json")
    try:
        with open(path, encoding="utf-8") as f:
            config = json.load(f)
        if isinstance(config, dict) and config.get("secret") and isinstance(config.get("port"), int):
            return config
    except (OSError, ValueError):
        pass
    return {"port": 0, "secret": new_secret()}


def save_proxy_config(profile: str, config: dict) -> None:
    with open(os.path.join(profile, "proxy.json"), "w", encoding="utf-8") as f:
        json.dump(config, f)


def stopped_by_server(message) -> None:
    kodi.notify(message or kodi.text("stream_stopped"), error=True)
    xbmc.Player().stop()


def run() -> None:
    monitor = xbmc.Monitor()
    profile = kodi.profile_dir()
    user_agent = kodi.user_agent()
    tokens = auth.Tokens(auth.Store(os.path.join(profile, "tokens.json")), user_agent)
    config = proxy_config(profile)
    proxy = Proxy(tokens, user_agent, config["secret"], config["port"], on_stopped=stopped_by_server)
    port = proxy.start()
    if port != config["port"]:
        save_proxy_config(profile, {"port": port, "secret": config["secret"]})
    kodi.set_property(kodi.PROXY, proxy.base)
    kodi.log("proxy on 127.0.0.1:%d" % port)
    player = Player(tokens, user_agent)
    while not monitor.waitForAbort(1):
        player.tick()
    player.close_overlay()
    kodi.clear_property(kodi.PROXY)
    proxy.stop()
