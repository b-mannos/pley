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


if __name__ == "__main__":
    unittest.main()
