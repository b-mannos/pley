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

from . import api, autoquality, kodi, listing, tracks
from .api import path_in

# The quality setting (docs/QUALITY.md): a quality's id. Before Automatic
# it was an index into another order, under another name.
QUALITY = "video_quality"
OLD_QUALITY = "quality"
# Where Automatic keeps the ceiling of a network found slow, for each server.
CEILING = "auto_ceiling:"

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


def beside(base: str, info: dict) -> list:
    """The text subtitles Kodi is given beside what it plays, named so it
    reads their language and whether they are forced (flagged, or only
    named so: "English (Forced)", "Signs & Songs"): all of them beside a
    converted stream, which carries none; beside a file played as it is,
    the files next to it (Kodi reads the file's own tracks itself, and an
    address over HTTP has no folder for it to look in). Kodi lists them
    first, in this order (tracks.py)."""
    return [
        path_in(base, api.subtitle_name(s["url"], s.get("language"), tracks.is_forced(s)))
        for s in tracks.beside_tracks(info)
    ]


def prepare(client: api.Client, base: str, item_id: int, start_ms: int, detail=None, restart=None) -> xbmcgui.ListItem:
    """Kodi's item for playing `item_id` from `start_ms`, its address set,
    once the server said how. The item's details (for Kodi's info) are
    asked for while the server decides, unless given. Tells the service
    what plays (kodi.PLAYING). `restart` is Automatic's quality when the
    service plays the title again at another one."""
    started = time.perf_counter()
    details = {"item": detail} if detail else {}
    loader = None
    if not detail:
        loader = threading.Thread(target=load_details, args=(client, item_id, details), daemon=True)
        loader.start()
    asked = ask(restart)
    request = {
        "item_id": item_id,
        "start_ms": start_ms,
        "client_id": install_id(),
        "caps": report(),
        "max_bitrate": asked["max_bitrate"],
        "max_height": asked["max_height"],
        "mode": asked["mode"],
    }
    # The account's subtitles, when they are pictures: drawn onto the video
    # if the file is converted (the server draws nothing when Kodi shows
    # them itself).
    burn = tracks.burn_index(tracks.file_of(detail))
    if burn is not None:
        request.update(file_id=tracks.file_of(detail)["id"], subtitle_index=burn)
    info = client.post("/api/playback", request)
    if loader:
        loader.join(10)
    detail = details.get("item")
    file = tracks.file_of(detail, info.get("file_id")) if detail else None
    if file is not None and "subtitle_index" not in request and tracks.needs_burn(info, file):
        # The details came with the answer: asked again, with them drawn on.
        request.update(file_id=file["id"], subtitle_index=tracks.chosen_subtitle(file)["index"])
        info = client.post("/api/playback", request)
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
    else:
        # Said here, Kodi needn't ask the server for the file's type first.
        mime = file_mime(detail, info.get("file_id"))
        if mime:
            li.setMimeType(mime)
            li.setContentLookup(False)
    subtitles = beside(base, info)
    if subtitles:
        li.setSubtitles(subtitles)
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
    if (detail or {}).get("kind") not in listing.MUSIC:
        # The account's audio and subtitles, which the service shows in Kodi.
        playing["tracks"] = tracks.layout(info, file)
        if asked["automatic"]:
            # What the service's Automatic watches the playback with.
            playing["auto"] = automatic(asked, info, file)
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


def chosen_quality() -> str:
    """The quality setting's choice. One chosen before Automatic came is
    carried over, once: Original can't be told from the setting never
    changed, and becomes Automatic."""
    old = kodi.setting_int(OLD_QUALITY)
    if old:
        carried = autoquality.from_old_index(old)
        if carried:
            kodi.set_setting(QUALITY, carried)
        kodi.set_setting_int(OLD_QUALITY, 0)
        kodi.log("quality setting carried over: %s" % carried)
    return autoquality.from_setting(kodi.setting(QUALITY))


def ask(restart=None) -> dict:
    """How the playback is asked for. Original and Automatic let the server
    choose (the file as it is where it plays); a quality from the settings
    caps it, and the server converts a file that is over it (one under it
    plays as it is, as the other apps play a file the quality wouldn't make
    smaller). Automatic under a ceiling remembered for a slow network
    starts at the quality under it, the same way; the service's restart
    at a quality converts."""
    if restart is not None:
        max_bitrate, max_height = autoquality.limits(restart)
        mode = "auto" if restart == autoquality.ORIGINAL else "transcode"
        return dict(automatic=True, quality=restart, lowered=False, mode=mode, max_bitrate=max_bitrate, max_height=max_height)
    chosen = chosen_quality()
    if chosen != autoquality.AUTO:
        max_bitrate, max_height = autoquality.limits(chosen)
        return dict(automatic=False, quality=chosen, lowered=False, mode="auto", max_bitrate=max_bitrate, max_height=max_height)
    start = autoquality.start_quality(ceiling())
    max_bitrate, max_height = autoquality.limits(start) if start else (None, None)
    return dict(
        automatic=True,
        quality=start or autoquality.ORIGINAL,
        lowered=start is not None,
        mode="auto",
        max_bitrate=max_bitrate,
        max_height=max_height,
    )


def automatic(asked: dict, info: dict, file) -> dict:
    """What the service's Automatic needs of a playback: the quality it
    plays at (the original when the server played the file as it is,
    under a ceiling it fits), the bitrate that is, the file's bitrate and
    lines, and the server's own quality for the file."""
    file = file or {}
    video = next((s for s in file.get("streams") or [] if s.get("kind") == "video"), None) or {}
    file_kbps = (file.get("bitrate") or 0) / 1000.0 or None
    file_lines = file.get("height") or video.get("height") or None
    quality = asked["quality"]
    converted = info.get("method") == "transcode"
    if not converted:
        quality = autoquality.ORIGINAL
    cap = info.get("auto_quality")
    if quality == autoquality.ORIGINAL:
        playing_kbps = autoquality.kbps_of(cap) if cap and converted else file_kbps
    else:
        playing_kbps = autoquality.kbps_of(quality)
        if file_kbps:
            playing_kbps = min(playing_kbps, file_kbps)
    return {
        "quality": quality,
        "playing_kbps": playing_kbps,
        "file_kbps": file_kbps,
        "file_lines": file_lines,
        "cap": cap,
        "lowered": asked["lowered"] and quality != autoquality.ORIGINAL,
        "server": server_key(),
        "prefix": autoquality.media_prefix(info["url"]),
    }


def server_key() -> str:
    return kodi.setting("server_id") or kodi.setting("server_url")


def ceiling():
    """The ceiling Automatic remembers for this server, None past 30 minutes."""
    return autoquality.remembered_ceiling(kodi.remembered(CEILING + server_key()), time.time())


def remember_ceiling(server: str, kbps) -> None:
    """Remembered for the next titles in Automatic (None forgets it)."""
    kodi.remember(CEILING + server, autoquality.remember_ceiling(kbps, time.time()) if kbps else "")


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
