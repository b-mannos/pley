import json
import unittest

import support  # noqa: F401
import xbmc
import xbmcaddon
import xbmcgui
from pley import kodi
from pley.autoquality import Meter
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


def automatic(session, quality="original", kbps=20_000, prefix=None):
    """A playback in Automatic, as playback.prepare tells the service of it."""
    return {
        "item_id": 7,
        "session_id": session,
        "duration_ms": 3_600_000,
        "path": "http://proxy/" + session,
        "auto": {
            "quality": quality,
            "playing_kbps": kbps,
            "file_kbps": 20_000,
            "file_lines": 1080,
            "cap": None,
            "lowered": False,
            "server": "srv-1",
            "prefix": prefix or "/api/playback/%s/" % session,
        },
    }


class AutoPlayer(KodiPlayer):
    """The service's player with its clock stood in for, and its restarts
    recorded rather than made."""

    def __init__(self):
        super().__init__()
        self.now = 0.0
        self.clock = lambda: self.now
        self.meter = Meter(clock=lambda: self.now)
        self.restarts = []
        self.played = []

    def restart(self, item_id, position_ms, quality):
        self.restarts.append((item_id, position_ms, quality))

    def play(self, path, li=None):
        self.played.append((path, li))

    def starts(self, playing):
        kodi.set_property(kodi.PLAYING, json.dumps(playing))
        self.file = playing["path"]
        self.onAVStarted()

    def second(self, moves=1.0, caching=False, arriving=0):
        self.now += 1
        self.seconds += moves
        if arriving:
            self.meter.chunk(self.auto["prefix"] + "1.ts", arriving)
        if caching:
            xbmc.conditions.add("Player.Caching")
        try:
            self.tick()
        finally:
            xbmc.conditions.discard("Player.Caching")


class AutomaticTest(unittest.TestCase):
    def setUp(self):
        import tempfile

        self.profile = tempfile.mkdtemp()
        xbmcaddon.settings["_profile"] = self.profile
        del xbmcgui.notified[:]
        self.player = AutoPlayer()

    def tearDown(self):
        xbmcaddon.settings.pop("_profile", None)
        kodi.clear_property(kodi.PLAYING)

    def test_a_wait_plays_the_title_again_lower_where_it_was(self):
        player = self.player
        player.starts(automatic("s1"))
        first = player.watch
        self.assertEqual(first.quality, "original")
        # 1.5 MB a second through the proxy: 12 Mbps.
        for _ in range(30):
            player.second(arriving=1_500_000)
        player.second(0.0, caching=True)
        player.second(0.0, caching=True)
        self.assertEqual(player.restarts, [(7, 30_000, "8m")])
        self.assertEqual(kodi.remembered("auto_ceiling:srv-1").split("@")[0], "8550", "12 Mbps for 19 of the last 20 s, with the margin")
        self.assertEqual(xbmcgui.notified, [], "said once it plays")
        player.second(0.0, caching=True)
        self.assertEqual(len(player.restarts), 1, "nothing more while it restarts")

        # The service's restart started playing it at 8 Mbps.
        player.starts(automatic("s2", "8m", 8_000))
        self.assertEqual(player.sent[-3:], [("stopped", 30_000), ("ended", "/api/playback/s1"), ("playing", 30_000)])
        self.assertIs(player.watch, first, "the title's watch, kept")
        self.assertEqual(first.quality, "8m")
        self.assertEqual(xbmcgui.notified, [("pley", "Playing at 8 Mbps 1080p for a smoother picture")])

    def test_back_to_the_original_forgets_the_ceiling(self):
        player = self.player
        kodi.remember("auto_ceiling:srv-1", "6000@%d" % 10**9)
        player.starts(automatic("s1", "4m", 4_000))
        player.restarting = (7, player.watch, None)
        player.starts(automatic("s2"))
        self.assertEqual(kodi.remembered("auto_ceiling:srv-1"), "")
        self.assertEqual(xbmcgui.notified, [], "silently")

    def test_a_seek_and_its_wait_arent_the_network(self):
        player = self.player
        player.starts(automatic("s1"))
        for _ in range(30):
            player.second()
        player.onPlayBackSeek(600_000, 570_000)
        for _ in range(8):
            player.second(0.0, caching=True)
        player.second(570.0)
        for _ in range(5):
            player.second()
        self.assertEqual(player.restarts, [])

    def test_a_chosen_quality_isnt_watched(self):
        player = self.player
        playing = automatic("s1")
        del playing["auto"]
        player.starts(playing)
        self.assertIsNone(player.watch)
        for _ in range(10):
            player.second()
        for _ in range(5):
            player.second(0.0, caching=True)
        self.assertEqual(player.restarts, [])

    def test_another_title_gets_its_own_watch(self):
        player = self.player
        player.starts(automatic("s1", "4m", 4_000))
        first = player.watch
        player.restarting = (8, first, "said")
        player.starts(automatic("s2"))
        self.assertIsNot(player.watch, first)
        self.assertEqual(xbmcgui.notified, [])


class RestartTest(unittest.TestCase):
    """The service playing a title again at another quality."""

    def setUp(self):
        from pley import playback

        self.playback = playback
        self.prepare = playback.prepare
        self.asked = []
        kodi.set_property(kodi.PROXY, "http://127.0.0.1:1/secret")

        def prepare(client, base, item_id, start_ms, detail=None, restart=None):
            self.asked.append((base, item_id, start_ms, restart))
            kodi.set_property(kodi.PLAYING, json.dumps({"session_id": "new", "path": "http://proxy/new"}))
            return xbmcgui.ListItem(path="http://proxy/new")

        playback.prepare = prepare
        self.player = AutoPlayer()
        self.player.restart = Player.restart.__get__(self.player)
        self.player.starts(automatic("s1"))

    def tearDown(self):
        self.playback.prepare = self.prepare
        kodi.clear_property(kodi.PLAYING)
        kodi.clear_property(kodi.PROXY)

    def test_it_plays_from_where_it_was(self):
        self.player.restart(7, 754_321, "8m")
        self.assertEqual(self.asked, [("http://127.0.0.1:1/secret", 7, 754_321, "8m")])
        (path, li), = self.player.played
        self.assertEqual((path, li.properties["StartOffset"]), ("http://proxy/new", "754.321"))

    def test_stopped_meanwhile_it_isnt_played(self):
        self.player.file = ""
        self.player.onPlayBackStopped()
        self.player.restarting = (7, None, None)
        self.player.restart(7, 1000, "8m")
        self.assertEqual((self.asked, self.player.played), ([], []))
        self.assertIsNone(self.player.restarting)


if __name__ == "__main__":
    unittest.main()
