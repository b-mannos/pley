"""Find subtitles in Kodi's player (docs/API.md, "Added subtitles"): the
add-on is a subtitle service of Kodi's (`xbmc.subtitle.module`), so the
player's Subtitles → Download lists "pley" beside any other service. For
a title pley plays, Kodi asks the plugin to search (`action=search`) and
lists what the server found on OpenSubtitles for the file, in the
profile's subtitle language; the one picked (`action=download`) is added
on the server, saved as the profile's choice for the show or the movie,
and handed to Kodi as a file, which Kodi loads into the player and shows.
Added subtitles are ordinary text tracks: every later playback, here and
on every other device, lists them with the file's own."""

from __future__ import annotations

import json
import os
import re

from . import api

# What the plugin hears from Kodi's subtitle dialog.
ACTIONS = ("search", "manualsearch", "download")
# Told to the service when a subtitle was added to what plays: Kodi lists
# it among the playback's streams where the service's layout doesn't know
# it, so the service stops following changes there (session id).
ADDED = "pley.subtitle_added"


def playing(raw: str):
    """What pley plays (the plugin's word to the service, kodi.PLAYING):
    {item_id, file_id, session_id}, None when Kodi plays something else
    or music."""
    try:
        found = json.loads(raw) if raw else None
    except ValueError:
        return None
    if not isinstance(found, dict) or not isinstance(found.get("item_id"), int) or "tracks" not in found:
        return None
    return found


def search(client, item_id: int, file_id) -> list:
    """GET /api/items/{id}/subtitles/search for the file playing, in the
    profile's subtitle language (no `language`: the server takes the
    profile's, English without one). Raises api.ApiError."""
    query = {"file_id": file_id} if file_id else {}
    answer = client.get("/api/items/%d/subtitles/search" % item_id, **query) or {}
    results = answer.get("results") if isinstance(answer, dict) else None
    return [r for r in results or [] if isinstance(r, dict) and r.get("id")]


def problem(error: api.ApiError, text) -> str:
    """What to say when the search can't be made: not set up on the server
    (503, the server's own words aside), a server from before it (404),
    or what went wrong. `text` is kodi.text."""
    if error.status == 503 and error.code == "unavailable":
        return text("subtitles_not_set_up")
    if error.status == 404:
        return text("subtitles_need_newer")
    return text("server_error") % error.message


def language_name(code: str, convert=None) -> str:
    """A result's language as Kodi names it in English ("English" for
    "en"), which Kodi's dialog shows and reads back for the file's name."""
    code = (code or "").split("-")[0].lower()
    if convert is not None:
        try:
            name = convert(code)
            if name:
                return name
        except Exception:  # noqa: BLE001 - Kodi's conversion knows most, not all
            pass
    return code or "Unknown"


def rating(result: dict) -> str:
    """Kodi's 0 to 5 stars, from how often it was downloaded (5 for 10,000
    or more), and 5 for one made for this very file."""
    if result.get("matched") == "hash":
        return "5"
    downloads = result.get("downloads") or 0
    for stars, least in ((5, 10_000), (4, 1_000), (3, 100), (2, 10), (1, 1)):
        if downloads >= least:
            return str(stars)
    return "0"


def entries(results: list, plugin_url: str, item_id: int, file_id, convert=None) -> list:
    """What Kodi's dialog lists of the results: (address, label, label2,
    art, properties), the address the plugin's download action."""
    import urllib.parse

    out = []
    for result in results:
        query = {"action": "download", "result": result["id"], "item": item_id}
        if file_id:
            query["file"] = file_id
        language = (result.get("language") or "").split("-")[0].lower()
        out.append(
            (
                plugin_url + "?" + urllib.parse.urlencode(query),
                language_name(language, convert),
                result.get("name") or result["id"],
                {"icon": rating(result), "thumb": language},
                {
                    "sync": "true" if result.get("matched") == "hash" else "false",
                    "hearing_imp": "true" if result.get("hearing_impaired") else "false",
                },
            )
        )
    return out


def add(client, item_id: int, file_id, result: str) -> dict:
    """POST /api/items/{id}/subtitles: the track the server made of the
    result (SubtitleTrack, `added: true`). Raises api.ApiError, or
    ValueError for an answer that isn't a track."""
    body = {"result": result}
    if file_id:
        body["file_id"] = int(file_id)
    track = client.post("/api/items/%d/subtitles" % item_id, body)
    if not isinstance(track, dict) or not isinstance(track.get("index"), int) or not track.get("url"):
        raise ValueError("not a subtitle track: %r" % (track,))
    return track


def save(client, track: dict, folder: str) -> str:
    """The track's WebVTT, fetched and written to `folder` as Kodi names a
    subtitle file (10001.eng.vtt): its path. Raises api.ApiError."""
    response = client.request("GET", track["url"], headers={"Accept": "text/vtt"})
    if response.status >= 400:
        raise api.error_of(response)
    os.makedirs(folder, exist_ok=True)
    name = api.subtitle_name(track["url"], track.get("language"), bool(track.get("forced"))).rsplit("/", 1)[-1]
    name = re.sub(r"[^A-Za-z0-9._-]", "_", name)
    path = os.path.join(folder, name)
    with open(path, "wb") as f:
        f.write(response.body)
    return path


def chosen(file_id, index: int) -> dict:
    """PUT /api/items/{id}/streams's body: the added track as the
    profile's choice, the audio as it was."""
    return {"file_id": file_id, "subtitle_index": index}
