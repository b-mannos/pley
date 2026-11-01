"""Requests' news, in the service. The summary is read as
Kodi starts, when the sign-in or its profile changes, when the event stream
opens, when the plugin did something to requests (requests.ASKING), soon
after request events, and every minute while the stream is down; it is
kept for the plugin (requests.SUMMARY). The listings that show its number,
or show requests, are shown again when they change (Your requests every 10
seconds while something on it downloads). A TV pops up ready-to-watch
notices only, never over playing video: they wait for it to stop."""

from __future__ import annotations

import json
import threading
import time

import xbmc

from . import kodi, listing, requests

# Events come in bursts (a request's events and its requests_changed): the
# summary is read once, this long after the first.
GATHER = 1.0
# Without the event stream: the summary read this often, and Your requests
# on screen asked for again this often.
POLL = 60
LISTED_POLL = 30
# Your requests while something on it downloads or imports, stream or not.
LIVE_POLL = 10
# Notices that wait for a video to stop: the last ones.
KEEP = 3
# The catch-up's look at the news (GET /api/requests?unseen=1).
CATCH_UP = 20
# remembered.json's newest updated_at announced, per server and account.
ANNOUNCED = "requests.announced."
# Events that may bring news (unseen rises) or change a request on screen.
REQUEST_EVENTS = ("requests_changed", "request_available", "request_decided", "request_created", "request_failed")
# What keeps a listing from being shown again under the person: a dialog.
DIALOG = "System.HasVisibleModalDialog"
POSTER = 185


class News:
    """`call(method, path, fine=…)`: the service's request to the server (an
    answer, or None when signed out or out of reach). `identity()`: who the
    sign-in acts as. `work(job)`: runs a job off the service's loop.
    `playing_video()`: whether a video plays. `proxy_base()`: the proxy's
    address, for the notices' posters."""

    def __init__(self, call, identity, work, playing_video, proxy_base, clock=time.monotonic):
        self.call = call
        self.identity = identity
        self.work = work
        self.playing_video = playing_video
        self.proxy_base = proxy_base
        self.clock = clock
        self.lock = threading.Lock()
        # Who the summary is of, and its account (for what was announced).
        self.who = None
        self.account = None
        self.summary = None
        # When the summary is read next (None: when something says so).
        self.due = None
        self.reading = False
        self.polled = float("-inf")
        # Off on a server without requests for apps: asked again only when
        # the stream opens again.
        self.off = False
        self.stream_open = False
        # A request event came since the summary was last read: its unseen
        # rising is explained.
        self.explained = False
        # Listings to show again: those with the summary's number, those
        # with requests.
        self.counted = False
        self.listed = False
        self.waiting = []
        # The listing on screen, since when, and whether it is to be shown
        # again once a video or a dialog stops holding it back.
        self.path = ""
        self.shown_at = float("-inf")
        self.held = False

    # ------------------------------------------------------- what says so

    def heard(self, event: dict) -> None:
        """An event of the server's stream, from its thread."""
        kind = event.get("type")
        if kind == "requests_settings_changed":
            self.read_soon()
        elif kind in REQUEST_EVENTS:
            with self.lock:
                self.explained = True
                self.listed = True
            self.read_soon()
            if kind == "request_available" and event.get("item_id"):
                with self.lock:
                    who = self.who
                self.work(lambda: self.available(event, who))

    def opened(self) -> None:
        """The event stream opened (again): nothing it missed is replayed."""
        with self.lock:
            self.off = False
            self.due = self.clock()

    def asked(self) -> None:
        """The plugin asked for something, did something to a request, saw
        them or switched profile (requests.ASKING)."""
        with self.lock:
            self.due = self.clock()

    def read_soon(self) -> None:
        with self.lock:
            soon = self.clock() + GATHER
            if self.due is None or self.due > soon:
                self.due = soon

    # ------------------------------------------------------------ the loop

    def tick(self, stream_open: bool) -> None:
        """Once a second, from the service's loop."""
        now = self.clock()
        who = self.identity()
        with self.lock:
            self.stream_open = stream_open
            if who != self.who:
                self.reset(who)
            due = self.due is not None and now >= self.due
            if not stream_open and not self.off and now - self.polled >= POLL:
                due = True
            if due and self.who is not None and not self.reading:
                self.reading = True
                self.due = None
                self.polled = now
                self.work(self.read)
        self.show_waiting()
        self.refresh(now, stream_open)

    def reset(self, who) -> None:
        """Another sign-in, or profile: its own summary and news (locked)."""
        if self.who is not None:
            # What the plugin was told of the one before.
            kodi.clear_property(requests.SUMMARY)
            kodi.clear_property(requests.LIVE)
        self.who = who
        self.account = None
        self.summary = None
        self.off = False
        self.waiting = []
        self.due = self.clock() if who is not None else None

    def show_waiting(self) -> None:
        """The notices that waited, once no video plays."""
        if not self.waiting or self.playing_video():
            return
        with self.lock:
            waiting, self.waiting = self.waiting, []
        for message, icon in waiting:
            kodi.notify(message, icon=icon, ms=8000)

    def refresh(self, now: float, stream_open: bool) -> None:
        """The pley listing on screen shown again where it changed: one with
        the summary's number, one with requests; Your requests every 10
        seconds while something on it downloads (every 30 without the
        stream). Never under a dialog or behind a video: once they close."""
        path = xbmc.getInfoLabel("Container.FolderPath")
        if path and path != self.path:
            # Another listing, read as it showed (none shows behind a video).
            self.path, self.shown_at, self.held = path, now, False
        with self.lock:
            # Off screen, they are asked for afresh when they show.
            counted, listed = self.counted, self.listed
            self.counted = self.listed = False
        wanted = self.held or (counted and requests.showing(path, requests.COUNTED)) or (listed and requests.showing(path, requests.LISTED))
        if not wanted and path:
            every = LIVE_POLL if path == kodi.get_property(requests.LIVE) else LISTED_POLL
            wanted = now - self.shown_at >= every and (every == LIVE_POLL or (not stream_open and requests.showing(path, ("requests",))))
        if not wanted:
            return
        if self.playing_video() or xbmc.getCondVisibility(DIALOG):
            self.held = True
            return
        self.held = False
        self.shown_at = now
        xbmc.executebuiltin("Container.Refresh")

    # -------------------------------------------------------- on the worker

    def read(self) -> None:
        """GET /api/requests/summary, kept; then what its change says."""
        try:
            self._read()
        finally:
            with self.lock:
                self.reading = False

    def _read(self) -> None:
        with self.lock:
            who = self.who
        answer = self.call("GET", "/api/requests/summary", fine=(403, 404))
        if answer is None:
            # Signed out or out of reach: as it was.
            return
        off = answer.status in (403, 404)
        if off:
            # An older server, or one from before apps could ask.
            found = {"enabled": False}
        elif answer.status >= 400:
            return
        else:
            try:
                found = answer.json()
            except ValueError:
                return
            if not isinstance(found, dict):
                return
        with self.lock:
            if self.who != who:
                return
            before, self.summary = self.summary, found
            explained, self.explained = self.explained, False
            self.off = off
            if before is not None and without(before, "changed_at") != without(found, "changed_at"):
                self.counted = True
            if before is not None and not self.stream_open and before.get("changed_at") != found.get("changed_at"):
                self.listed = True
        kodi.set_property(requests.SUMMARY, json.dumps(found))
        unseen = (found.get("unseen") or 0) if found.get("enabled") else 0
        if unseen and (before is None or (unseen > (before.get("unseen") or 0) and not explained)):
            self.catch_up(who)

    def catch_up(self, who) -> None:
        """News that came with no event (Kodi was off, the stream down): the
        unseen requests ready to watch that weren't announced, in one
        notice."""
        answer = self.call("GET", "/api/requests?unseen=1&limit=%d" % CATCH_UP)
        if answer is None or answer.status != 200:
            return
        try:
            found = [r for r in (answer.json() or {}).get("requests") or [] if isinstance(r, dict)]
        except (ValueError, AttributeError):
            return
        key = self.announced_key(who)
        if key is None:
            return
        mark = announced(key)
        fresh = [r for r in found if (r.get("updated_at") or 0) > mark]
        if not fresh:
            return
        ready = [r for r in fresh if requests.ready_now(r)]
        if len(ready) == 1:
            self.notice(who, requests.ready(requests.as_event(ready[0]), kodi.text), ready[0].get("poster_url"))
        elif ready:
            self.notice(who, requests.news(len(ready), kodi.text))
        announce(key, max(r.get("updated_at") or 0 for r in fresh))

    def available(self, event: dict, who) -> None:
        """request_available, heard as `who`: ready to watch, said once."""
        self.notice(who, requests.ready(event, kodi.text), event.get("poster_url"))
        # Never again at a catch-up: the request as it is now.
        answer = self.call("GET", "/api/requests/%d" % int(event.get("request_id") or 0), fine=(404,))
        key = self.announced_key(who)
        if answer is None or answer.status != 200 or key is None:
            return
        try:
            updated = (answer.json() or {}).get("updated_at")
        except (ValueError, AttributeError):
            return
        if isinstance(updated, (int, float)) and updated > announced(key):
            announce(key, updated)

    def announced_key(self, who):
        """remembered.json's key for what was announced to this account on
        this server (None once the sign-in changed)."""
        with self.lock:
            if self.who != who or who is None:
                return None
            account = self.account
        if account is None:
            answer = self.call("GET", "/api/auth/me")
            said = ""
            if answer is not None and answer.status == 200:
                try:
                    said = str((answer.json() or {}).get("id") or "")
                except (ValueError, AttributeError):
                    said = ""
            with self.lock:
                if self.who != who:
                    return None
                if said:
                    self.account = said
            # The profile switched to stands for the account where the
            # server didn't say (asked again next time).
            account = said or str(who[1] or "")
        return ANNOUNCED + who[0] + "|" + account

    def notice(self, who, message: str, poster=None) -> None:
        """A notice for `who`, now or once no video plays (the last few
        kept); none once another sign-in or profile took over."""
        base = self.proxy_base()
        icon = listing.image(base, poster, POSTER) if base else None
        with self.lock:
            if self.who != who:
                return
            self.waiting.append((message, icon))
            del self.waiting[:-KEEP]


def without(found: dict, name: str) -> dict:
    return {k: v for k, v in found.items() if k != name}


def announced(key: str) -> float:
    try:
        return float(kodi.remembered(key, "0"))
    except ValueError:
        return 0.0


def announce(key: str, updated_at) -> None:
    kodi.remember(key, "%d" % updated_at)
