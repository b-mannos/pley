"""Asking the server to play something, for the plugin's play route (which
hands Kodi the answer) and pley's window (which plays it itself): what this
Kodi plays, the address to play, Kodi's item for it, and what the service
needs to report progress and offer Skip and Up Next."""

from __future__ import annotations

import json
import os
import threading
import time

import xbmc
import xbmcgui

from . import api, kodi, listing
from .api import path_in

# The quality setting's choices, as the web player's (apps/web/src/player/quality.ts).
QUALITIES = [(None, None), (30000, 2160), (20000, 1080), (15000, 2160), (12000, 1080), (8000, 1080), (4000, 720), (2000, 480)]

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


def prepare(client: api.Client, base: str, item_id: int, start_ms: int, detail=None) -> xbmcgui.ListItem:
    """Kodi's item for playing `item_id` from `start_ms`, its address set,
    once the server said how. The item's details (for Kodi's info) are
    asked for while the server decides, unless given. Tells the service
    what plays (kodi.PLAYING)."""
    started = time.perf_counter()
    details = {"item": detail} if detail else {}
    loader = None
    if not detail:
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
    url = path_in(base, info["url"])
    duration_ms = info.get("duration_ms") or (detail or {}).get("duration_ms") or 0
    if detail:
        li = listing.list_item(base, detail)
        listing.video_details(li, detail, base)
        if detail["kind"] not in listing.MUSIC:
            # Kodi starts where this says, whatever it was asked: where
            # the server was told to start (none for "from the start").
            li.getVideoInfoTag().setResumePoint(start_ms / 1000.0, duration_ms / 1000.0)
    else:
        li = xbmcgui.ListItem(offscreen=True)
    li.setPath(url)
    if info.get("hls"):
        li.setMimeType("application/vnd.apple.mpegurl")
        li.setContentLookup(False)
        # Text subtitles come beside a stream that carries none, named so
        # Kodi reads their language and whether they are forced.
        subtitles = [
            path_in(base, api.subtitle_name(s["url"], s.get("language"), s.get("forced")))
            for s in info.get("subtitles", [])
            if s.get("url")
        ]
        if subtitles:
            li.setSubtitles(subtitles)
    else:
        # Said here, Kodi needn't ask the server for the file's type first.
        mime = file_mime(detail, info.get("file_id"))
        if mime:
            li.setMimeType(mime)
            li.setContentLookup(False)
    markers = [m for m in (detail or {}).get("markers") or [] if m.get("file_id") == info.get("file_id")]
    playing = {
        "item_id": item_id,
        "file_id": info.get("file_id"),
        "session_id": info["session_id"],
        "duration_ms": duration_ms,
        "path": url,
        # What the service needs for Skip and Up Next, so it asks for none of it again.
        "kind": (detail or {}).get("kind"),
        "markers": markers,
    }
    kodi.set_property(kodi.PLAYING, json.dumps(playing))
    reason = info.get("reason")
    elapsed = (time.perf_counter() - started) * 1000
    kodi.log("playing %d by %s, ready in %d ms%s" % (item_id, info.get("method"), elapsed, ": " + reason if reason else ""))
    return li


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
    from . import caps

    settings = {
        name: kodi.setting_value(name)
        for name in ["audiooutput.passthrough", "audiooutput.channels"] + [n for n, _ in caps.PASSTHROUGH]
    }
    screen = (int(xbmc.getInfoLabel("System.ScreenWidth") or 0), int(xbmc.getInfoLabel("System.ScreenHeight") or 0))
    model = "Kodi %s on %s" % (kodi.kodi_version(), kodi.platform())
    return caps.build(xbmc.getInfoLabel("System.SupportedHDRTypes"), settings, screen, model, kodi.is_coreelec())
