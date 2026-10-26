import json
import unittest

import support  # noqa: F401
import xbmc
from pley import kodi
from pley.service import PAUSED_EVERY, REPORT_EVERY, Player, Reports


class ReportsTest(unittest.TestCase):
    def setUp(self):
        self.now = 0.0
        self.sent = []
        self.ended = []
        playing = {"item_id": 7, "session_id": "abc", "duration_ms": 100_000}
        self.reports = Reports(playing, lambda path, body: self.sent.append((path, body)), self.ended.append,
                               clock=lambda: self.now)

    def states(self):
        return [(body["state"], body["position_ms"]) for _, body in self.sent]

    def test_playing_is_reported_every_ten_seconds(self):
        self.reports.at(0)
        for second in range(1, 25):
            self.now = second
            self.reports.at(second * 1000)
        self.assertEqual(self.states(), [("playing", 0), ("playing", 10_000), ("playing", 20_000)])
        path, body = self.sent[0]
        self.assertEqual(path, "/api/items/7/progress")
        self.assertEqual((body["session_id"], body["duration_ms"]), ("abc", 100_000))

    def test_pause_resume_and_stop(self):
        self.reports.at(1000)
        self.now = 3
        self.reports.position_ms = 4000
        self.reports.pause()
        self.now = 30
        self.reports.at(4000)
        self.reports.resume()
        self.now = 31
        self.reports.at(5000)
        self.reports.finish(ended=False)
        self.assertEqual(self.states(), [("playing", 1000), ("paused", 4000), ("playing", 4000), ("stopped", 5000)])
        self.assertEqual(self.ended, ["/api/playback/abc"])

    def test_a_paused_playback_is_reported_every_30_seconds(self):
        # The server ends a session it hasn't heard of for 90 seconds.
        self.reports.at(5000)
        self.now = 1
        self.reports.position_ms = 5000
        self.reports.pause()
        for second in range(2, 70):
            self.now = second
            self.reports.at(9_999_999)
        self.assertEqual(self.states(), [("playing", 5000), ("paused", 5000), ("paused", 5000), ("paused", 5000)])
        self.assertEqual(PAUSED_EVERY, 30)

    def test_a_stream_that_broke_off_early_isnt_watched(self):
        self.reports.at(40_000)
        self.reports.finish(ended=True)
        self.assertEqual(self.states()[-1], ("stopped", 40_000), "40 % in: where it was")
        reports = Reports({"item_id": 7, "session_id": "s", "duration_ms": 100_000}, lambda p, b: self.sent.append((p, b)), self.ended.append)
        reports.credits_from = 80_000
        reports.at(81_000)
        reports.finish(ended=True)
        self.assertEqual(self.states()[-1], ("stopped", 100_000), "in the credits: the whole length")

    def test_the_end_reports_the_whole_length(self):
        self.reports.at(98_000)
        self.reports.finish(ended=True)
        self.assertEqual(self.states()[-1], ("stopped", 100_000))
        self.assertEqual(REPORT_EVERY, 10)


class Now:
    """The service's worker, doing each job at once."""

    def put(self, job):
        job()


class KodiPlayer(Player):
    """The service's player, with what Kodi plays stood in for."""

    def __init__(self):
        super().__init__(tokens=None, user_agent="pleyKodi/test", worker=Now())
        self.file = ""
        self.seconds = 0.0
        self.sent = []

    def isPlaying(self):
        return bool(self.file)

    def getPlayingFile(self):
        return self.file

    def getTime(self):
        return self.seconds

    def send(self, playing, path, body):
        self.sent.append((body["state"], body["position_ms"]))

    def end(self, path):
        self.sent.append(("ended", path))


class PlayerTest(unittest.TestCase):
    def setUp(self):
        self.player = KodiPlayer()
        playing = {"item_id": 7, "session_id": "s", "duration_ms": 100_000, "path": "http://proxy/7"}
        kodi.set_property(kodi.PLAYING, json.dumps(playing))
        self.player.file = playing["path"]

    def tearDown(self):
        xbmc.conditions.discard("Player.Paused")
        kodi.clear_property(kodi.PLAYING)

    def test_a_pause_kodi_told_of_too_early_is_reported(self):
        # Paused while the stream started: Kodi said so before it said the
        # stream started, when there was nothing to report.
        self.player.seconds = 3.0
        xbmc.conditions.add("Player.Paused")
        self.player.onAVStarted()
        self.player.tick()
        self.player.tick()
        xbmc.conditions.discard("Player.Paused")
        self.player.seconds = 4.0
        self.player.tick()
        self.assertEqual(self.player.sent, [("playing", 3000), ("paused", 3000), ("playing", 3000)])

    def test_a_stop_ends_the_session(self):
        self.player.seconds = -0.099
        self.player.onAVStarted()
        self.player.seconds = 20.0
        self.player.tick()
        self.player.file = ""
        self.player.onPlayBackStopped()
        self.assertEqual(self.player.sent, [("playing", 0), ("stopped", 20_000), ("ended", "/api/playback/s")])


if __name__ == "__main__":
    unittest.main()
