import unittest

import support  # noqa: F401
from pley import listing

BASE = "http://127.0.0.1:5000/s"
EPISODE = {
    "id": 31,
    "library_id": 2,
    "kind": "episode",
    "title": "Pilot",
    "sort_title": "Pilot",
    "index": 1,
    "parent_index": 2,
    "grandparent_title": "The Show",
    "duration_ms": 1_800_000,
    "added_at": 0,
    "poster_url": "/api/items/30/images/poster?v=9",
    "thumb_url": "/api/items/31/images/thumb?v=9",
    "summary": "It begins.",
    "user_state": {"position_ms": 600_000, "watched": False, "play_count": 0},
}


class ListingTest(unittest.TestCase):
    def test_episodes_play_and_resume(self):
        li = listing.list_item(BASE, EPISODE)
        self.assertEqual(li.label, "1. Pilot")
        self.assertEqual(li.properties["IsPlayable"], "true")
        tag = li.video.values
        self.assertEqual((tag["setMediaType"], tag["setTvShowTitle"], tag["setSeason"], tag["setEpisode"]),
                         ("episode", "The Show", 2, 1))
        self.assertEqual(tag["setResumePoint"], (600.0, 1800.0))
        self.assertEqual(tag["setPlaycount"], 0)
        self.assertEqual(li.art["thumb"], BASE + "/api/items/31/images/thumb?v=9&w=640")
        # The show's poster goes as the show's: skins show an episode's own
        # poster in place of its still.
        self.assertEqual(li.art["tvshow.poster"], BASE + "/api/items/30/images/poster?v=9&w=500")
        self.assertNotIn("poster", li.art)

    def test_episodes_among_films_show_their_shows_poster(self):
        li = listing.list_item(BASE, EPISODE, with_show=True, posters=True)
        self.assertEqual(li.art["poster"], BASE + "/api/items/30/images/poster?v=9&w=500")
        self.assertEqual(li.art["thumb"], BASE + "/api/items/31/images/thumb?v=9&w=640")

    def test_shows_open_and_count_their_episodes(self):
        show = {"id": 30, "library_id": 2, "kind": "show", "title": "The Show", "sort_title": "Show", "added_at": 0,
                "leaf_count": 10, "viewed_leaf_count": 4, "child_count": 2, "user_state": {"watched": False}}
        li = listing.list_item(BASE, show)
        self.assertNotIn("IsPlayable", li.properties)
        self.assertEqual((li.properties["TotalEpisodes"], li.properties["UnWatchedEpisodes"]), ("10", "6"))
        self.assertEqual(li.properties["TotalSeasons"], "2")
        # Library listings leave the backdrop out: it is asked for by id.
        self.assertEqual(li.art, {"fanart": BASE + "/api/items/30/images/backdrop?w=1280"})
        self.assertNotIn("setResumePoint", li.video.values)

    def test_episodes_away_from_their_season_say_their_show(self):
        self.assertEqual(listing.label(EPISODE, with_show=True), "The Show · S2 · E1 · Pilot")
        self.assertEqual(listing.list_item(BASE, EPISODE, with_show=True).label, "The Show · S2 · E1 · Pilot")
        no_numbers = dict(EPISODE, index=None, parent_index=None)
        self.assertEqual(listing.label(no_numbers, with_show=True), "The Show · Pilot")

    def test_a_title_never_played_says_so(self):
        # Kodi would otherwise offer a resume point of its own for the address.
        fresh = dict(EPISODE, user_state={"position_ms": 0, "watched": False})
        self.assertEqual(listing.list_item(BASE, fresh).video.values["setResumePoint"], (0.0, 1800.0))

    def test_tracks_are_music(self):
        track = {"id": 5, "library_id": 3, "kind": "track", "title": "Song", "sort_title": "Song", "index": 3,
                 "parent_index": 1, "parent_title": "Album", "grandparent_title": "Artist", "duration_ms": 200_000,
                 "added_at": 0, "user_state": {"play_count": 2}}
        li = listing.list_item(BASE, track)
        tag = li.music.values
        self.assertEqual((tag["setArtist"], tag["setAlbum"], tag["setTrack"], tag["setDuration"]), ("Artist", "Album", 3, 200))
        self.assertEqual(tag["setPlayCount"], 2)
        self.assertEqual(li.video.values, {})

    def test_details_add_cast_and_ids(self):
        movie = dict(EPISODE, kind="movie", title="Film")
        li = listing.list_item(BASE, movie)
        detail = dict(movie, genres=["Drama"], studio="A24", content_rating="R", tagline=None, original_title=None,
                      tmdb_id=12, imdb_id="tt1", crew=[{"name": "Dir", "role": "director"}],
                      cast=[{"name": "Lead", "character": "Hero", "photo_url": "/api/people/1/photo?v=1"}])
        listing.video_details(li, detail, BASE)
        tag = li.video.values
        self.assertEqual(tag["setUniqueIDs"], ({"tmdb": "12", "imdb": "tt1"}, "tmdb"))
        self.assertEqual(tag["setDirectors"], ["Dir"])
        self.assertEqual(tag["setCast"][0].thumbnail, BASE + "/api/people/1/photo?v=1&w=300")


if __name__ == "__main__":
    unittest.main()
