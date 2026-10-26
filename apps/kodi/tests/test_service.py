import unittest

import support  # noqa: F401
from pley.service import REPORT_EVERY, Reports


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

    def test_the_end_reports_the_whole_length(self):
        self.reports.at(98_000)
        self.reports.finish(ended=True)
        self.assertEqual(self.states()[-1], ("stopped", 100_000))
        self.assertEqual(REPORT_EVERY, 10)


if __name__ == "__main__":
    unittest.main()
