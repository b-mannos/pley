import unittest

import support  # noqa: F401
from pley import autoquality as aq
from pley.autoquality import Meter, Watch


class MenuTest(unittest.TestCase):
    def test_the_menu_in_its_order(self):
        # docs/QUALITY.md: the two 4K qualities first, then 1080p and below.
        self.assertEqual(aq.IDS, ["original", "auto", "30m", "15m", "20m", "12m", "8m", "4m", "2m"])
        self.assertEqual(
            [aq.label(q) for q in aq.IDS],
            ["Original", "Automatic", "30 Mbps 4K", "15 Mbps 4K", "20 Mbps 1080p", "12 Mbps 1080p", "8 Mbps 1080p", "4 Mbps 720p", "2 Mbps 480p"],
        )
        self.assertEqual(aq.limits("15m"), (15000, 2160))
        self.assertEqual(aq.limits("auto"), (None, None))
        self.assertEqual(aq.limits("original"), (None, None))

    def test_automatic_unless_a_quality_was_chosen(self):
        self.assertEqual(aq.from_setting(""), "auto")
        self.assertEqual(aq.from_setting("nonsense"), "auto")
        self.assertEqual(aq.from_setting("original"), "original")
        self.assertEqual(aq.from_setting("8m"), "8m")

    def test_a_choice_made_in_the_old_order_keeps_its_meaning(self):
        # The old order: Original, 30m, 20m, 15m, 12m, 8m, 4m, 2m.
        self.assertEqual([aq.from_old_index(i) for i in range(8)], [None, "30m", "20m", "15m", "12m", "8m", "4m", "2m"])
        self.assertIsNone(aq.from_old_index(8))
        self.assertIsNone(aq.from_old_index(-1))

    def test_a_file_is_offered_the_qualities_that_make_it_smaller(self):
        self.assertEqual(aq.offered(10_000, 1080), ["8m", "4m", "2m"])
        self.assertEqual(aq.offered(18_000, 2160), ["20m", "15m", "12m", "8m", "4m", "2m"], "1080p is fewer lines")
        self.assertEqual(aq.offered(20_000, 1080), ["15m", "12m", "8m", "4m", "2m"])
        self.assertEqual(aq.offered(None, None), ["30m", "20m", "15m", "12m", "8m", "4m", "2m"])
        self.assertEqual(aq.offered(1_500, 480), [])

    def test_where_a_remembered_ceiling_starts(self):
        self.assertIsNone(aq.start_quality(None))
        self.assertEqual(aq.start_quality(9_000), "8m")
        self.assertEqual(aq.start_quality(22_000), "20m")
        self.assertEqual(aq.start_quality(500), "2m", "never below 2 Mbps 480p")

    def test_a_ceiling_is_remembered_for_30_minutes(self):
        value = aq.remember_ceiling(6_000, 1_000_000)
        self.assertEqual(aq.remembered_ceiling(value, 1_000_000 + 29 * 60), 6_000)
        self.assertIsNone(aq.remembered_ceiling(value, 1_000_000 + 31 * 60))
        self.assertIsNone(aq.remembered_ceiling("", 1_000_000))
        self.assertIsNone(aq.remembered_ceiling("junk", 1_000_000))

    def test_the_paths_of_a_playback(self):
        self.assertEqual(aq.media_prefix("/api/playback/abc/master.m3u8"), "/api/playback/abc/")
        self.assertEqual(aq.media_prefix("/api/files/7/stream?x=1"), "/api/files/7/stream")


class StepTest(unittest.TestCase):
    def test_down_from_the_original_by_what_the_network_carried(self):
        # 40 Mbps arriving at 0.5 of real time: 20 Mbps carried, 15 with the margin.
        self.assertEqual(aq.step_down("original", 40_000, 0.5, None, 40_000, 2160, None), ("15m", 15_000))
        # A 1080p file at 10 Mbps, delivered at 0.9: 6.75 Mbps.
        self.assertEqual(aq.step_down("original", 10_000, 0.9, None, 10_000, 1080, None), ("4m", 6_750))

    def test_never_above_the_servers_own_quality(self):
        quality, _ = aq.step_down("original", 60_000, 0.95, None, 60_000, 2160, "15m")
        self.assertEqual(quality, "15m")

    def test_always_lower_and_never_below_2_mbps(self):
        # Delivery that looks fine still goes a quality down.
        self.assertEqual(aq.step_down("8m", 8_000, 1.4, None, 20_000, 1080, None)[0], "4m")
        self.assertEqual(aq.step_down("4m", 4_000, 0.1, None, 20_000, 1080, None)[0], "2m")
        self.assertEqual(aq.step_down("2m", 2_000, 0.1, None, 20_000, 1080, None), (None, None))

    def test_without_a_delivery_measure(self):
        # What the proxy saw arrive.
        self.assertEqual(aq.step_down("original", 20_000, None, 9_000, 20_000, 1080, None)[0], "4m")
        self.assertEqual(aq.step_down("original", 20_000, None, None, 20_000, 1080, None), ("4m", 7_500))
        # Not even the file's bitrate: a middling quality.
        self.assertEqual(aq.step_down("original", None, None, None, None, None, None)[0], "8m")

    def test_up_one_quality_at_a_time_then_the_original(self):
        self.assertEqual(aq.step_up("4m", 25_000, 2160, None), ("8m", 8000))
        self.assertEqual(aq.step_up("20m", 25_000, 2160, None), ("original", 25_000))
        # Under the server's own quality, the original is what it converts to.
        self.assertEqual(aq.step_up("12m", 60_000, 2160, "15m"), ("15m", 15000))
        self.assertEqual(aq.step_up("15m", 60_000, 2160, "15m"), ("original", 15000))
        self.assertEqual(aq.step_up("original", 25_000, 2160, None), (None, None))
        self.assertEqual(aq.step_up("30m", None, None, None), (None, None), "unknown file: can't measure against it")


class Kodi:
    """A playback for a Watch: a second at a time, playing, waiting or seeking."""

    def __init__(self, watch, now=0.0, position=0.0):
        self.watch = watch
        self.now = now
        self.position = position
        self.ahead = None
        self.arriving = None
        self.carried = None
        self.said = []

    def second(self, moves=1.0, caching=False, paused=False, seeking=False, ahead=None):
        self.now += 1
        self.position += moves
        if ahead is not None:
            self.ahead = ahead
        said = self.watch.tick(self.now, self.position, paused, seeking, caching, self.ahead, self.arriving, self.carried)
        if said:
            self.said.append((self.now, said))
        return said

    def play(self, seconds):
        for _ in range(seconds):
            self.second()

    def wait(self, seconds):
        for _ in range(seconds):
            self.second(0.0, caching=True)


def watch(quality="original", playing=20_000, file=20_000, lines=1080, cap=None, lowered=False):
    return Watch(0.0, quality, playing, file, lines, cap, lowered)


class WatchTest(unittest.TestCase):
    def test_a_long_wait_steps_down(self):
        kodi = Kodi(watch())
        kodi.play(30)
        kodi.wait(1)
        self.assertEqual(kodi.said, [])
        kodi.wait(1)
        self.assertEqual(kodi.said, [(32, "4m")], "2 s: half with the margin, without a measure")
        kodi.wait(5)
        self.assertEqual(len(kodi.said), 1, "one wait, one step")

    def test_two_short_waits_within_a_minute_step_down(self):
        kodi = Kodi(watch())
        kodi.play(30)
        kodi.second(0.0, caching=True)
        kodi.play(20)
        self.assertEqual(kodi.said, [])
        kodi.second(0.0, caching=True)
        kodi.play(1)
        self.assertEqual([q for _, q in kodi.said], ["4m"])

    def test_short_waits_a_minute_apart_dont(self):
        kodi = Kodi(watch())
        kodi.play(30)
        kodi.second(0.0, caching=True)
        kodi.play(70)
        kodi.second(0.0, caching=True)
        kodi.play(5)
        self.assertEqual(kodi.said, [])

    def test_the_start_a_seek_and_a_pause_are_not_the_network(self):
        kodi = Kodi(watch())
        kodi.wait(10)
        kodi.play(30)
        # A seek Kodi said it made, then its wait.
        kodi.second(0.0, seeking=True)
        kodi.wait(6)
        # A jump Kodi didn't say it made (a skip), then its wait.
        kodi.play(5)
        kodi.second(120.0)
        kodi.wait(6)
        # Paused.
        kodi.play(5)
        for _ in range(10):
            kodi.second(0.0, paused=True)
        kodi.wait(1)
        kodi.watch.seeked()
        kodi.wait(5)
        self.assertEqual(kodi.said, [])

    def test_slow_delivery_steps_down_before_a_wait(self):
        kodi = Kodi(watch())
        kodi.second(ahead=9.0)
        kodi.play(5)
        # 0.8 s of video arriving each second: 0.2 s less ahead every second.
        for _ in range(30):
            kodi.second(ahead=kodi.ahead - 0.2)
            if kodi.said:
                break
        self.assertEqual(len(kodi.said), 1)
        when, quality = kodi.said[0]
        # Under 1.1 for 15 s; 20 Mbps at 0.8 with the margin is 12 Mbps.
        self.assertEqual(quality, "12m")
        self.assertLess(when, 30)

    def test_plenty_ahead_or_steady_is_fine(self):
        kodi = Kodi(watch())
        kodi.second(ahead=30.0)
        for _ in range(60):
            kodi.second(ahead=kodi.ahead - 0.2)
        self.assertEqual(kodi.said, [], "more than 10 s ahead")
        kodi = Kodi(watch())
        for _ in range(60):
            kodi.second(ahead=4.0)
        self.assertEqual(kodi.said, [], "a full cache, read as it plays")

    def test_again_but_not_within_20_seconds(self):
        w = watch()
        kodi = Kodi(w)
        # 12 Mbps arriving through the proxy: 9 with the margin.
        kodi.arriving = 12_000
        kodi.play(30)
        kodi.wait(2)
        self.assertEqual(kodi.said[-1][1], "8m")
        self.assertEqual(w.ceiling, 9_000)
        w.restarted(kodi.now, "8m", 8_000)
        kodi.play(5)
        kodi.wait(3)
        self.assertEqual(len(kodi.said), 1, "within 20 s of the change")
        kodi.play(20)
        kodi.arriving = 100_000
        kodi.wait(2)
        self.assertEqual(kodi.said[-1][1], "4m", "one quality at least")

    def test_back_up_when_the_network_carries_it(self):
        w = watch("4m", 4_000, 20_000, 1080, lowered=True)
        kodi = Kodi(w)
        kodi.carried = 13_000
        kodi.play(299)
        self.assertEqual(kodi.said, [], "not within 5 minutes of a step down")
        kodi.play(1)
        self.assertEqual(kodi.said, [(300, "8m")], "13 Mbps carries 1.6 times 8")
        w.restarted(kodi.now, "8m", 8_000)
        kodi.play(400)
        self.assertEqual(len(kodi.said), 1, "12 Mbps needs 19.2")
        kodi.carried = 40_000
        kodi.play(1)
        self.assertEqual(kodi.said[-1][1], "12m")
        w.restarted(kodi.now, "12m", 12_000)
        kodi.play(179)
        self.assertEqual(len(kodi.said), 2, "one step every 3 minutes at most")
        kodi.play(1)
        # 15 Mbps makes the 20 Mbps 1080p file smaller; 20 Mbps 1080p doesn't.
        self.assertEqual(kodi.said[-1][1], "15m")
        w.restarted(kodi.now, "15m", 15_000)
        kodi.play(180)
        self.assertEqual(kodi.said[-1][1], "original", "40 Mbps carries 1.6 times the file")

    def test_no_measure_stays_down(self):
        kodi = Kodi(watch("8m", 8_000, 20_000, 1080, lowered=True))
        kodi.play(1200)
        self.assertEqual(kodi.said, [])

    def test_an_issue_restarts_the_quiet_time(self):
        w = watch("4m", 4_000, 20_000, 1080, lowered=True)
        kodi = Kodi(w)
        kodi.carried = 50_000
        kodi.play(250)
        # A wait: down to 2 Mbps, the lowest.
        kodi.wait(2)
        self.assertEqual(kodi.said, [(252, "2m")])
        w.restarted(kodi.now, "2m", 2_000)
        kodi.play(30)
        # Another, which has nowhere lower to go.
        kodi.wait(2)
        self.assertEqual(len(kodi.said), 1)
        kodi.play(179)
        self.assertEqual(len(kodi.said), 1, "3 minutes after the last issue, 5 after the step down")
        kodi.play(300 - 179 - 32)
        self.assertEqual(kodi.said[-1], (552, "4m"))


class MeterTest(unittest.TestCase):
    def setUp(self):
        self.now = 0.0
        self.meter = Meter(clock=lambda: self.now)

    def test_what_arrives_over_20_seconds(self):
        for second in range(40):
            self.now = second
            self.meter.chunk("/api/playback/a/3.ts", 1_000_000)
            self.meter.chunk("/api/images/1", 5_000_000)
        self.assertAlmostEqual(self.meter.arriving_kbps("/api/playback/a/"), 21 * 8_000 / 20.0)
        self.assertIsNone(self.meter.arriving_kbps("/api/playback/b/"))

    def test_what_the_network_carried_needs_three_answers(self):
        self.meter.answered("/api/playback/a/1.ts", 4_000_000, 1.0)
        self.meter.answered("/api/playback/a/2.ts", 4_000_000, 2.0)
        self.assertIsNone(self.meter.carried_kbps("/api/playback/a/", 0))
        self.meter.answered("/api/playback/a/3.ts", 4_000_000, 4.0)
        self.meter.answered("/api/playback/a/4.ts", 100_000, 0.001)
        self.assertEqual(self.meter.carried_kbps("/api/playback/a/", 0), 8_000, "the third fastest, of 256 KB or more")
        self.now = 10
        self.assertIsNone(self.meter.carried_kbps("/api/playback/a/", 5), "since the last issue")


class StartTest(unittest.TestCase):
    """Where Automatic starts on a network the server knows (docs/QUALITY.md)."""

    def test_the_servers_ceiling(self):
        self.assertEqual(aq.network_ceiling({"local": False, "ceiling_kbps": 8000, "source": "playback"}), 8000)
        for answer in ({"ceiling_kbps": None}, {"ceiling_kbps": 0}, {"ceiling_kbps": True}, {}, None, []):
            self.assertIsNone(aq.network_ceiling(answer), answer)
        self.assertTrue(aq.wants_probe({"local": False, "probe": True}))
        self.assertFalse(aq.wants_probe({"local": True, "probe": True}), "never on the server's own network")
        self.assertFalse(aq.wants_probe({"probe": 1}))
        self.assertFalse(aq.wants_probe(None))

    def test_a_speed_test_that_comes_whole_in_time(self):
        test = aq.SpeedTest(total=1_000_000)
        test.chunk(10.0, 100_000)
        self.assertFalse(test.done(10.5))
        for i in range(9):
            test.chunk(10.1 + i * 0.1, 100_000)
        self.assertTrue(test.done(10.9))
        # The first piece held what came before the clock started.
        self.assertEqual(test.result(11.0), {"probe_kbps": 8000, "probe_complete": True})
        self.assertIsNone(aq.probe_ceiling(test.result()), "at least that fast: no limit")

    def test_a_speed_test_stopped_two_seconds_after_its_first_byte(self):
        test = aq.SpeedTest()
        test.chunk(0.0, 262_144)
        test.chunk(1.0, 500_000)
        test.chunk(1.9, 500_000)
        self.assertFalse(test.done(1.99))
        self.assertTrue(test.done(2.0))
        self.assertEqual(test.until(5.0), 2.0)
        self.assertEqual(test.result(2.0), {"probe_kbps": 4000, "probe_complete": False})
        self.assertEqual(aq.probe_ceiling(test.result(2.0)), 2800, "70 %, as the server keeps it")

    def test_too_little_measures_nothing(self):
        test = aq.SpeedTest()
        self.assertIsNone(test.result(3.0))
        self.assertEqual(test.until(3.5), 3.5, "the first byte awaited until the budget runs out")
        test.chunk(0.0, 200_000)
        test.chunk(1.0, 60_000)
        self.assertIsNone(test.result(2.0), "under 256 KiB")


if __name__ == "__main__":
    unittest.main()
