import os
import shutil
import tempfile
import unittest
from unittest import mock

import support
from pley import kodi, plugin

PNG = b"\x89PNG\r\n\x1a\n" + b"\0" * 16


class QrTest(unittest.TestCase):
    """The QR code of the sign-in, which the server draws."""

    def setUp(self):
        self.root = tempfile.mkdtemp()
        self.temp = os.path.join(self.root, "temp")
        os.mkdir(self.temp)
        patch = mock.patch.object(kodi, "temp_dir", return_value=self.temp)
        patch.start()
        self.addCleanup(patch.stop)
        self.server = support.FakeServer(lambda request: (200, {"Content-Type": "image/png"}, PNG))
        self.addCleanup(self.server.close)
        self.addCleanup(shutil.rmtree, self.root, True)

    def test_the_code_names_the_file(self):
        path = plugin.fetch_qr(self.server.url, "BCDF-GHJK")
        self.assertEqual(path, os.path.join(self.temp, "pley-qr-BCDFGHJK.png"))
        with open(path, "rb") as f:
            self.assertEqual(f.read(), PNG)
        self.assertEqual(self.server.requests[0].path, "/api/oauth/device/qr?code=BCDFGHJK")

    def test_a_code_that_isnt_one_writes_nothing(self):
        # A server's code that would lead the file elsewhere.
        for code in ["../../escaped", "..\\..\\escaped", "AB/CD", "BCDF GHJK", "", "B" * 40]:
            self.assertEqual(plugin.fetch_qr(self.server.url, code), "", code)
        self.assertEqual(self.server.requests, [])
        self.assertEqual(os.listdir(self.root), ["temp"])
        self.assertEqual(os.listdir(self.temp), [])


if __name__ == "__main__":
    unittest.main()
