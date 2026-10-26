"""The service: it holds the sign-in, runs the proxy, and tells the server
where playback is (docs/API.md, Playback): every 10 seconds while playing,
on pause, and when it stops, after which the session ends."""

from __future__ import annotations

import json
import os
import threading
import time

import xbmc

from . import api, auth, kodi
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

    def call(self, method: str, path: str, body=None) -> None:
        """One request to the server, the token renewed once if refused."""
        server = self.tokens.server()
        if not server:
            return
        token = self.tokens.access()
        for attempt in range(2):
            client = api.Client(server, self.user_agent, token)
            response = client.request(method, path, json_body=body)
            if response.status != 401 or attempt:
                if response.status >= 400 and response.status != 410:
                    kodi.log("%s %s: %d" % (method, path, response.status), xbmc.LOGWARNING)
                return
            token = self.tokens.refreshed(token)

    def safely(self, method: str, path: str, body=None) -> None:
        try:
            self.call(method, path, body)
        except (api.ApiError, auth.SignedOut) as e:
            kodi.log("%s %s failed: %s" % (method, path, e), xbmc.LOGWARNING)

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
            self.reports = Reports(
                playing,
                lambda path, body: self.safely("POST", path, body),
                lambda path: self.safely("DELETE", path),
            )
            self.reports.at(self.position())

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
            reports, self.reports = self.reports, None
        if reports:
            reports.finish(ended)
            # Listings show the new resume point and watched marks.
            xbmc.executebuiltin("Container.Refresh")

    def tick(self) -> None:
        with self.lock:
            if self.reports and self.isPlaying():
                self.reports.at(self.position())


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
    kodi.clear_property(kodi.PROXY)
    proxy.stop()
