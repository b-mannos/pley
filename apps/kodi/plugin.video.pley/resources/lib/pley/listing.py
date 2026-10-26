"""pley's items as Kodi list items: their artwork, info and resume points."""

from __future__ import annotations

import xbmc
import xbmcgui

from .proxy import path_in

# What plays, and what opens a listing.
PLAYABLE = ("movie", "episode", "track", "extra")
MEDIA_TYPES = {
    "movie": "movie",
    "show": "tvshow",
    "season": "season",
    "episode": "episode",
    "extra": "video",
    "artist": "artist",
    "album": "album",
    "track": "song",
}
MUSIC = ("artist", "album", "track")
# Kodi's container content for a listing of each kind.
CONTENT = {
    "movie": "movies",
    "show": "tvshows",
    "season": "seasons",
    "episode": "episodes",
    "extra": "videos",
    "artist": "artists",
    "album": "albums",
    "track": "songs",
}


def image(base: str, url, width: int):
    """The proxy's address of an image at `width` pixels, or None."""
    if not url:
        return None
    return path_in(base, url + ("&" if "?" in url else "?") + "w=%d" % width)


def label(item: dict) -> str:
    if item["kind"] == "episode" and item.get("index") is not None:
        return "%d. %s" % (item["index"], item["title"])
    if item["kind"] == "track" and item.get("index") is not None:
        return "%d. %s" % (item["index"], item["title"])
    return item["title"]


def art(base: str, item: dict) -> dict:
    poster = image(base, item.get("poster_url"), 500)
    backdrop = image(base, item.get("backdrop_url"), 1920)
    still = image(base, item.get("thumb_url"), 640)
    found = {}
    if item["kind"] == "episode":
        found["thumb"] = still or poster
        found["poster"] = poster
    elif item["kind"] in MUSIC:
        found["thumb"] = poster
    else:
        found["poster"] = poster
        found["thumb"] = poster
    if backdrop:
        found["fanart"] = backdrop
    return {k: v for k, v in found.items() if v}


def list_item(base: str, item: dict) -> xbmcgui.ListItem:
    kind = item["kind"]
    li = xbmcgui.ListItem(label(item), offscreen=True)
    li.setArt(art(base, item))
    state = item.get("user_state") or {}
    seconds = (item.get("duration_ms") or 0) // 1000
    if kind in MUSIC:
        tag = li.getMusicInfoTag()
        tag.setMediaType(MEDIA_TYPES[kind])
        tag.setTitle(item["title"])
        if kind == "track":
            tag.setArtist(item.get("grandparent_title") or "")
            tag.setAlbum(item.get("parent_title") or "")
            tag.setTrack(item.get("index") or 0)
            tag.setDisc(item.get("parent_index") or 0)
            tag.setDuration(seconds)
        elif kind == "album":
            tag.setArtist(item.get("parent_title") or "")
            tag.setAlbum(item["title"])
        if item.get("year"):
            tag.setYear(item["year"])
        tag.setPlayCount(state.get("play_count") or 0)
    else:
        tag = li.getVideoInfoTag()
        tag.setMediaType(MEDIA_TYPES.get(kind, "video"))
        tag.setTitle(item["title"])
        tag.setSortTitle(item.get("sort_title") or item["title"])
        if item.get("year"):
            tag.setYear(item["year"])
        if item.get("summary"):
            tag.setPlot(item["summary"])
        if item.get("release_date"):
            tag.setPremiered(item["release_date"])
        if item.get("rating") is not None:
            tag.setRating(float(item["rating"]))
        if seconds:
            tag.setDuration(seconds)
        if kind == "episode":
            tag.setTvShowTitle(item.get("grandparent_title") or "")
            tag.setSeason(item.get("parent_index") or 0)
            tag.setEpisode(item.get("index") or 0)
        elif kind == "season":
            tag.setTvShowTitle(item.get("parent_title") or "")
            tag.setSeason(item.get("index") or 0)
        tag.setPlaycount(1 if state.get("watched") else 0)
        if kind in PLAYABLE and state.get("position_ms") and seconds:
            tag.setResumePoint(state["position_ms"] / 1000.0, float(seconds))
        if kind in ("show", "season") and item.get("leaf_count") is not None:
            total = item["leaf_count"]
            watched = item.get("viewed_leaf_count") or 0
            li.setProperty("TotalEpisodes", str(total))
            li.setProperty("WatchedEpisodes", str(watched))
            li.setProperty("UnWatchedEpisodes", str(max(total - watched, 0)))
        if kind == "show" and item.get("child_count") is not None:
            li.setProperty("TotalSeasons", str(item["child_count"]))
    if kind in PLAYABLE:
        li.setProperty("IsPlayable", "true")
    return li


def video_details(li: xbmcgui.ListItem, detail: dict, base: str) -> None:
    """What only an item's details have: genres, studio, rating, cast."""
    if detail["kind"] in MUSIC:
        return
    tag = li.getVideoInfoTag()
    if detail.get("genres"):
        tag.setGenres(detail["genres"])
    if detail.get("studio"):
        tag.setStudios([detail["studio"]])
    if detail.get("content_rating"):
        tag.setMpaa(detail["content_rating"])
    if detail.get("tagline"):
        tag.setTagLine(detail["tagline"])
    if detail.get("original_title"):
        tag.setOriginalTitle(detail["original_title"])
    ids = {}
    if detail.get("tmdb_id"):
        ids["tmdb"] = str(detail["tmdb_id"])
    if detail.get("imdb_id"):
        ids["imdb"] = detail["imdb_id"]
    if ids:
        tag.setUniqueIDs(ids, "tmdb" if "tmdb" in ids else "imdb")
    cast = [
        xbmc.Actor(c["name"], c.get("character") or "", i, image(base, c.get("photo_url"), 300) or "")
        for i, c in enumerate(detail.get("cast") or [])
    ]
    if cast:
        tag.setCast(cast)
    crew = detail.get("crew") or []
    directors = [c["name"] for c in crew if c.get("role") == "director"]
    writers = [c["name"] for c in crew if c.get("role") == "writer"]
    if directors:
        tag.setDirectors(directors)
    if writers:
        tag.setWriters(writers)
