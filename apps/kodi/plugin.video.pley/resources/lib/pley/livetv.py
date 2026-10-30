"""Live TV (docs/API.md, "Live TV"): the server's IPTV channels and their
guide, as pley's window and Kodi's lists show them, and how one plays.

A channel plays as the provider sends it, through the server
(`GET …/stream.ts?client_id=`): Kodi reads MPEG-TS itself, deinterlaces
interlaced channels with its own deinterlacer, and passes surround sound
through, so the server converts nothing. The server's live HLS (`POST
…/watch`) is made for players that can't: it converts what isn't H.264,
makes the sound AAC stereo, and starts a few segments behind.

The plugin's play route first asks to watch (`POST …/watch`), which tunes
the channel, says when every connection is in use (409), and gives what is
on for the player's info; then hands Kodi the stream, whose reader ends the
`watch` (the same `client_id`). A device watches one channel at a time:
changing channel lets go of the one before.
"""

from __future__ import annotations

import time
from urllib.parse import urlencode

import xbmc
import xbmcgui

from . import api, kodi, words
from .listing import image

SUMMARY = "/api/live"
CHANNELS = "/api/live/channels"
GUIDE = "/api/live/guide"
# A channel's schedule, asked for this many hours at a time: the guide
# can only be asked for every channel at once.
SCHEDULE_HOURS = 6
LOGO_WIDTH = 320
PICTURE_WIDTH = 640
MIME = "video/mp2t"
# The plugin's play route for a channel, which Kodi asks as it comes to it
# in the queue of channels (Next and Previous change channel).
PLAY_ROUTE = "live_play"
# Channels queued either side of the one played.
QUEUED = 100

# Kodi's own strings: the short names of the days (Mon…Sun) and months
# (Jan…Dec), in Kodi's language.
SHORT_DAYS = 41
SHORT_MONTHS = 51
ENGLISH_DAYS = ("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")
ENGLISH_MONTHS = ("Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec")


# ------------------------------------------------------------ the server


def summary(client) -> dict:
    """GET /api/live: whether to show Live TV ({available, allowed,
    channels, sources, recent}). {} from a server without Live TV, or one
    that refuses it: no Live TV then. Out of reach, it raises, as the
    calls beside it do."""
    try:
        found = client.get(SUMMARY)
    except api.ApiError as e:
        if e.status == 0:
            raise
        return {}
    return found if isinstance(found, dict) else {}


def available(found: dict) -> bool:
    return bool((found or {}).get("available"))


def channels(client) -> list:
    """The shown channels by number, each with what's on now and next."""
    return client.get(CHANNELS) or []


def set_favorite(client, channel_id: int, favorite: bool) -> None:
    path = "%s/%d/favorite" % (CHANNELS, channel_id)
    if favorite:
        client.put(path)
    else:
        client.delete(path)


def watch(client, channel_id: int, client_id: str) -> dict:
    """Tunes the channel for this device (what it watched before ends):
    the channel, with what is on. Raises ApiError, 409 when every
    connection of its source is in use."""
    answer = client.post("%s/%d/watch" % (CHANNELS, channel_id), {"client_id": client_id}) or {}
    return answer.get("channel") or {"id": channel_id, "name": ""}


def stream_path(channel_id: int, client_id: str) -> str:
    """The channel's MPEG-TS as the provider sends it, on the server."""
    return "%s/%d/stream.ts?%s" % (CHANNELS, channel_id, urlencode({"client_id": client_id}))


def schedule(client, channel_id: int, start: float, hours: int = SCHEDULE_HOURS) -> list:
    """The channel's programmes from `start` (the one on then included) over
    `hours`."""
    begin = int(start)
    response = client.request("GET", GUIDE, query={"from": begin, "to": begin + hours * 3600})
    if response.status >= 400:
        raise api.error_of(response)
    return programs_of(response.body, channel_id)


def programs_of(body: bytes, channel_id: int) -> list:
    """One channel's programmes out of the guide's answer, found in it
    rather than the whole of it read: the guide is asked for every channel
    at once, and a lineup of tens of thousands sends tens of megabytes for
    a few hours. Its channels are its only keys made of digits."""
    import json
    import re

    text = body.decode("utf-8", "replace")
    key = re.search(r'"%d"\s*:\s*(?=\[)' % channel_id, text)
    if key is None:
        return []
    try:
        found, _ = json.JSONDecoder().raw_decode(text, key.end())
    except ValueError:
        return []
    return found if isinstance(found, list) else []


# ------------------------------------------------------------ what's on


def airing(program, now: float) -> bool:
    return bool(program) and program["start"] <= now < program["stop"]


def on_now(channel: dict, now: float):
    """What the channel shows at `now`, if the guide says: its `now`, or
    its `next` once that has begun (the channels were listed a while ago)."""
    for key in ("now", "next"):
        program = channel.get(key)
        if airing(program, now):
            return program
    return None


def up_next(channel: dict, now: float):
    """What comes after what's on now, if the guide says."""
    program = channel.get("next")
    if program and program["start"] >= now:
        return program
    return None


def progress(program: dict, now: float) -> float:
    """How far into `program` `now` is, 0 to 1."""
    length = max(1, program["stop"] - program["start"])
    return min(1.0, max(0.0, (now - program["start"]) / length))


def time_format() -> str:
    """Kodi's time format, without seconds ("%H:%M", "%I:%M %p")."""
    region = getattr(xbmc, "getRegion", None)
    found = region("time") if region else ""
    return (found or "%H:%M").replace(":%S", "").strip()


def clock(ts: float) -> str:
    """"20:00", or "8:00 PM", as Kodi writes times."""
    text = time.strftime(time_format(), time.localtime(ts))
    # Twelve-hour times without the leading zero, as the web writes them.
    return text[1:] if text.startswith("0") and "%I" in time_format() else text


def span(program: dict) -> str:
    """"20:00 – 21:00"."""
    return "%s – %s" % (clock(program["start"]), clock(program["stop"]))


def kodi_string(number: int, english: str) -> str:
    found = getattr(xbmc, "getLocalizedString", None)
    return (found(number) if found else "") or english


def day(ts: float, now: float) -> str:
    """"Today", "Tomorrow", "Yesterday", or "Fri 30 Oct"."""
    when = time.localtime(ts)
    for offset, name in ((0, "today"), (1, "tomorrow"), (-1, "yesterday")):
        other = time.localtime(now + offset * 86_400)
        if (when.tm_year, when.tm_yday) == (other.tm_year, other.tm_yday):
            return kodi.text(name)
    weekday = kodi_string(SHORT_DAYS + when.tm_wday, ENGLISH_DAYS[when.tm_wday])
    month = kodi_string(SHORT_MONTHS + when.tm_mon - 1, ENGLISH_MONTHS[when.tm_mon - 1])
    return "%s %d %s" % (weekday, when.tm_mday, month)


def left(program: dict, now: float) -> str:
    """"12 min left", "1h 5m left"."""
    return words.time_left((now - program["start"]) * 1000, (program["stop"] - program["start"]) * 1000)


def episode(program: dict) -> str:
    """"S1 · E7", when the guide numbers it."""
    return words.episode_label(program.get("season"), program.get("episode"))


def badges(program: dict) -> list:
    """Live, New and the rating, as the web's programme details show them."""
    found = []
    if program.get("is_live"):
        found.append(kodi.text("live"))
    if program.get("is_new"):
        found.append(kodi.text("new"))
    if program.get("rating"):
        found.append(program["rating"])
    return found


def name(channel: dict) -> str:
    """"101 Pley News HD", or the name alone."""
    number = channel.get("number")
    return "%s %s" % (number, channel.get("name") or "") if number else channel.get("name") or ""


def nothing_on(channel: dict) -> str:
    """What a channel without a programme now says (the web's words)."""
    return kodi.text("nothing_on_now") if channel.get("has_guide") else kodi.text("no_guide")


def headline(program: dict) -> str:
    """A programme's title, with its episode's own title: "Morning
    Briefing: Part 7"."""
    title = program.get("title") or ""
    subtitle = program.get("subtitle")
    return "%s: %s" % (title, subtitle) if subtitle and subtitle != title else title


def plot(channel: dict, now: float) -> str:
    """What's on now and next, for Kodi's info on a channel."""
    current = on_now(channel, now)
    later = up_next(channel, now)
    parts = []
    if current:
        # Its title is the item's tagline and second label.
        line = " · ".join(p for p in [left(current, now), episode(current)] + badges(current) if p)
        parts.append("[B]%s[/B]%s" % (span(current), "  " + line if line else ""))
        if current.get("description"):
            parts.append("\n" + current["description"].strip())
    else:
        parts.append(nothing_on(channel))
    if later:
        parts.append("\n[B]%s[/B]  %s  %s" % (kodi.text("next"), clock(later["start"]), headline(later)))
    return "\n".join(parts)


def groups(found: list) -> list:
    """(group, how many channels) in the order the channels have them."""
    counts = {}
    for channel in found:
        group = channel.get("group") or ""
        if group:
            counts[group] = counts.get(group, 0) + 1
    return list(counts.items())


def recent_channels(found: list, recent: list) -> list:
    """The channels watched last, newest first, that are still shown."""
    by_id = {c["id"]: c for c in found}
    return [by_id[i] for i in recent or [] if i in by_id]


def shown(found: list, which: str, recent=(), group=None) -> list:
    """The channels of a tab: "all", "favorites", "recent" or "group"."""
    if which == "favorites":
        return [c for c in found if c.get("favorite")]
    if which == "recent":
        return recent_channels(found, list(recent))
    if which == "group":
        return [c for c in found if (c.get("group") or "") == group]
    return list(found)


def by_number(found: list, typed: str):
    """The channel numbered `typed`, else the first whose number starts
    with it (the web's number keys); None when none does."""
    exact = next((c for c in found if (c.get("number") or "") == typed), None)
    return exact or next((c for c in found if (c.get("number") or "").startswith(typed)), None)


# ------------------------------------------------------------ Kodi's items


def play_url(channel_id: int) -> str:
    """The plugin's play route for a channel."""
    return "plugin://%s/?%s" % (kodi.ADDON_ID, urlencode({"route": PLAY_ROUTE, "id": channel_id}))


def logo(base: str, channel: dict, width: int = LOGO_WIDTH):
    return image(base, channel.get("logo"), width)


def list_item(base: str, channel: dict, now: float) -> xbmcgui.ListItem:
    """A channel in Kodi's lists, its queue of channels and its player:
    its number and name, what's on now beside it, now and next in its info,
    its logo, and the programme's picture as fanart. Kodi plays it as a
    stream without an end: no duration and no resume point, so nothing of
    it is kept in Kodi's library."""
    current = on_now(channel, now)
    li = xbmcgui.ListItem(name(channel), headline(current) if current else "", offscreen=True)
    art = {}
    found = logo(base, channel)
    if found:
        art.update(thumb=found, icon=found)
    picture = image(base, (current or {}).get("image"), PICTURE_WIDTH)
    if picture:
        art["fanart"] = picture
    if art:
        li.setArt(art)
    tag = li.getVideoInfoTag()
    tag.setMediaType("video")
    tag.setTitle(name(channel))
    tag.setPlot(plot(channel, now))
    if current:
        tag.setTagLine(headline(current))
        if current.get("categories"):
            tag.setGenres(list(current["categories"]))
    li.setProperty("IsPlayable", "true")
    return li


def play_item(base: str, channel: dict, client_id: str, now: float) -> xbmcgui.ListItem:
    """Kodi's item for playing a channel: its stream, said to be MPEG-TS,
    so Kodi asks the server nothing before it plays."""
    li = list_item(base, channel, now)
    li.setPath(api.path_in(base, stream_path(channel["id"], client_id)))
    li.setMimeType(MIME)
    li.setContentLookup(False)
    return li


def queue(base: str, found: list, at: int, now: float) -> tuple:
    """Kodi's video queue, holding the channels of `found` around the one at
    `at` (QUEUED either side: a lineup can have tens of thousands), and
    where that one is in it: Next and Previous in the player change
    channel."""
    first = max(0, at - QUEUED)
    playlist = xbmc.PlayList(xbmc.PLAYLIST_VIDEO)
    playlist.clear()
    for channel in found[first : at + QUEUED + 1]:
        playlist.add(play_url(channel["id"]), list_item(base, channel, now))
    playlist.unshuffle()
    return playlist, at - first


def is_ours(playlist) -> bool:
    """Whether Kodi's video queue is a queue of pley's channels."""
    try:
        return playlist.size() > 0 and "route=%s" % PLAY_ROUTE in playlist[0].getPath() and kodi.ADDON_ID in playlist[0].getPath()
    except (RuntimeError, IndexError, AttributeError, TypeError):
        return False


def forget_queue() -> None:
    """Empties Kodi's video queue when it is pley's channels: after a
    channel that can't be watched, Kodi would try each one after it."""
    playlist = xbmc.PlayList(xbmc.PLAYLIST_VIDEO)
    if is_ours(playlist):
        playlist.clear()


def busy(message: str) -> None:
    """Every connection is in use (409): the server's words, which name
    the channels being watched, in a box that waits to be read."""
    xbmcgui.Dialog().ok(kodi.text("all_in_use"), message or kodi.text("all_in_use"))
