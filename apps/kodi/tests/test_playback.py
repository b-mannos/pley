import unittest

import support  # noqa: F401
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


if __name__ == "__main__":
    unittest.main()
