import unittest

import support  # noqa: F401
from pley import prompts

INTRO = {"file_id": 4, "kind": "intro", "start_ms": 5_000, "end_ms": 35_000, "source": "chapter"}
CREDITS = {"file_id": 4, "kind": "credits", "start_ms": 80_000, "end_ms": 90_000, "source": "chapter"}
NEXT = {"id": 6, "kind": "episode", "title": "Pilot", "index": 2, "parent_index": 1}


class MarkersTest(unittest.TestCase):
    def test_the_button_shows_until_a_second_before_the_end(self):
        markers = [INTRO, CREDITS]
        self.assertIsNone(prompts.active(markers, 4_999))
        self.assertEqual(prompts.active(markers, 5_000), INTRO)
        self.assertEqual(prompts.active(markers, 33_999), INTRO)
        self.assertIsNone(prompts.active(markers, 34_000))
        self.assertEqual(prompts.active(markers, 80_000), CREDITS)

    def test_skipping_lands_where_the_web_player_lands(self):
        self.assertEqual(prompts.target_ms(INTRO), 34_250, "the intro's last moment plays out")
        short = dict(INTRO, start_ms=10_000, end_ms=11_000)
        self.assertEqual(prompts.target_ms(short), 10_500, "half of a short intro at most")
        self.assertEqual(prompts.target_ms(CREDITS), 90_000)

    def test_episodes_are_written_as_the_android_app_writes_them(self):
        self.assertEqual(prompts.episode_line(NEXT), "S1 · E2 · Pilot")
        self.assertEqual(prompts.episode_line({"kind": "movie", "title": "Film"}), "Film")


class MomentsTest(unittest.TestCase):
    def setUp(self):
        self.now = 0.0
        self.moments = prompts.Moments([INTRO, CREDITS], NEXT, clock=lambda: self.now)

    def test_an_episodes_credits_bring_up_the_next_one(self):
        self.assertEqual(self.moments.offer(10_000), ("skip", INTRO))
        self.assertIsNone(self.moments.offer(50_000))
        self.assertEqual(self.moments.offer(81_000), ("next", CREDITS))
        self.assertEqual(self.moments.counting(), prompts.COUNTDOWN)
        self.now = 4.5
        self.assertEqual(self.moments.counting(), 6)
        self.now = 30
        self.assertEqual(self.moments.counting(), 0)

    def test_cancelled_up_next_leaves_skip_credits(self):
        self.moments.offer(81_000)
        self.moments.counting()
        self.moments.cancel_next()
        self.assertEqual(self.moments.offer(81_000), ("skip", CREDITS))
        self.assertIsNone(self.moments.next_since)

    def test_a_dismissed_button_stays_away_for_that_segment(self):
        self.moments.dismiss(INTRO)
        self.assertIsNone(self.moments.offer(10_000))
        self.assertEqual(self.moments.offer(81_000), ("next", CREDITS))

    def test_without_a_next_episode_credits_are_skipped(self):
        movie = prompts.Moments([CREDITS])
        self.assertEqual(movie.offer(85_000), ("skip", CREDITS))


if __name__ == "__main__":
    unittest.main()
