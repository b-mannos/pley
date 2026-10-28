import tempfile
import unittest

import support  # noqa: F401
import xbmcaddon
from pley import playback

BASE = "http://127.0.0.1:5000/s"
TRACKS = [
    {"index": 3, "language": "eng", "forced": False, "external": False, "text_based": True, "url": "/api/files/7/subtitles/3.vtt"},
    {"index": 4, "language": None, "forced": False, "external": False, "text_based": False, "url": None},
    {"index": 1000, "language": "fre", "forced": True, "external": True, "text_based": True, "url": "/api/files/7/subtitles/1000.vtt"},
]


class PlaybackTest(unittest.TestCase):
    def test_text_subtitles_beside_what_plays(self):
        # A converted stream carries no subtitles: every text track comes beside it.
        converted = playback.beside(BASE, {"hls": True, "subtitles": TRACKS})
        self.assertEqual(converted, [BASE + "/api/files/7/subtitles/3.eng.vtt", BASE + "/api/files/7/subtitles/1000.fre.forced.vtt"])
        # A file played as it is: Kodi reads its own tracks, and gets the files next to it.
        direct = playback.beside(BASE, {"hls": False, "subtitles": TRACKS})
        self.assertEqual(direct, [BASE + "/api/files/7/subtitles/1000.fre.forced.vtt"])
        self.assertEqual(playback.beside(BASE, {"hls": False, "subtitles": None}), [])

    def test_subtitles_only_named_forced_are_named_forced_for_kodi(self):
        # Many files flag no forced track and only name it: the server counts
        # it forced, and Kodi would list it as a second "English".
        named = [
            {"index": 6, "language": "eng", "title": "English", "forced": False, "external": False, "text_based": True, "url": "/api/files/1/subtitles/6.vtt"},
            {"index": 7, "language": "eng", "title": "English (Forced)", "forced": False, "external": False, "text_based": True, "url": "/api/files/1/subtitles/7.vtt"},
            {"index": 4, "language": "eng", "title": "Signs & Songs", "forced": False, "external": False, "text_based": True, "url": "/api/files/1/subtitles/4.vtt"},
            {"index": 8, "language": "eng", "title": "Forcedly Bilingual", "forced": False, "external": False, "text_based": True, "url": "/api/files/1/subtitles/8.vtt"},
        ]
        converted = playback.beside(BASE, {"hls": True, "subtitles": named})
        self.assertEqual(
            [path.rsplit("/", 1)[-1] for path in converted],
            ["6.eng.vtt", "7.eng.forced.vtt", "4.eng.forced.vtt", "8.eng.vtt"],
        )


class QualityTest(unittest.TestCase):
    """The quality setting and how a playback is asked for (docs/QUALITY.md)."""

    def setUp(self):
        self.profile = tempfile.mkdtemp()
        self.saved = dict(xbmcaddon.settings)
        xbmcaddon.settings.clear()
        xbmcaddon.settings.update({"_profile": self.profile, "server_id": "srv-1"})

    def tearDown(self):
        xbmcaddon.settings.clear()
        xbmcaddon.settings.update(self.saved)

    def test_automatic_when_nothing_was_chosen(self):
        self.assertEqual(playback.chosen_quality(), "auto")
        xbmcaddon.settings["quality"] = 0
        self.assertEqual(playback.chosen_quality(), "auto", "Original or never changed: they can't be told apart")

    def test_a_quality_chosen_before_is_carried_over_once(self):
        xbmcaddon.settings["quality"] = 3  # 15 Mbps 4K in the old order
        self.assertEqual(playback.chosen_quality(), "15m")
        self.assertEqual((xbmcaddon.settings["video_quality"], xbmcaddon.settings["quality"]), ("15m", 0))
        xbmcaddon.settings["video_quality"] = "original"
        self.assertEqual(playback.chosen_quality(), "original", "a choice made since is kept")
        xbmcaddon.settings["quality"] = 99
        self.assertEqual(playback.chosen_quality(), "original", "nonsense isn't carried")
        self.assertEqual(xbmcaddon.settings["quality"], 0)

    def test_how_each_choice_is_asked_for(self):
        xbmcaddon.settings["video_quality"] = "original"
        self.assertEqual(playback.ask(), dict(automatic=False, quality="original", lowered=False, mode="auto", max_bitrate=None, max_height=None))
        # A quality: the server converts a file over it, and plays one under it as it is.
        xbmcaddon.settings["video_quality"] = "12m"
        self.assertEqual(playback.ask(), dict(automatic=False, quality="12m", lowered=False, mode="auto", max_bitrate=12000, max_height=1080))
        xbmcaddon.settings["video_quality"] = "auto"
        self.assertEqual(playback.ask(), dict(automatic=True, quality="original", lowered=False, mode="auto", max_bitrate=None, max_height=None))
        # The service's restarts.
        self.assertEqual(playback.ask("4m"), dict(automatic=True, quality="4m", lowered=False, mode="transcode", max_bitrate=4000, max_height=720))
        self.assertEqual(playback.ask("original")["mode"], "auto")

    def test_automatic_starts_under_a_ceiling_remembered_for_the_server(self):
        xbmcaddon.settings["video_quality"] = "auto"
        playback.remember_ceiling("srv-1", 9_450)
        self.assertEqual(playback.ask(), dict(automatic=True, quality="8m", lowered=True, mode="auto", max_bitrate=8000, max_height=1080))
        xbmcaddon.settings["server_id"] = "srv-2"
        self.assertEqual(playback.ask()["quality"], "original", "another server's network")
        xbmcaddon.settings["server_id"] = "srv-1"
        playback.remember_ceiling("srv-1", None)
        self.assertEqual(playback.ask()["quality"], "original", "forgotten")

    def test_what_automatic_is_told_of_a_playback(self):
        file = {"id": 3, "bitrate": 24_000_000, "streams": [{"kind": "video", "height": 2160}]}
        asked = playback.ask("8m")
        info = {"method": "transcode", "url": "/api/playback/s1/master.m3u8", "auto_quality": None}
        found = playback.automatic(asked, info, file)
        self.assertEqual(
            found,
            {"quality": "8m", "playing_kbps": 8000, "file_kbps": 24_000, "file_lines": 2160, "cap": None, "lowered": False,
             "server": "srv-1", "prefix": "/api/playback/s1/"},
        )
        # Under a remembered ceiling the file fitted: played as it is.
        lowered = dict(playback.ask("8m"), lowered=True)
        direct = playback.automatic(lowered, {"method": "direct", "url": "/api/files/3/stream"}, dict(file, bitrate=6_000_000))
        self.assertEqual((direct["quality"], direct["playing_kbps"], direct["lowered"]), ("original", 6000, False))
        # The original converted: at the server's own quality, which caps Automatic.
        converted = playback.automatic(playback.ask("original"), dict(info, auto_quality="15m"), file)
        self.assertEqual((converted["quality"], converted["playing_kbps"], converted["cap"]), ("original", 15000, "15m"))
        self.assertIsNone(playback.automatic(asked, info, None)["file_kbps"], "details that didn't come")


if __name__ == "__main__":
    unittest.main()
