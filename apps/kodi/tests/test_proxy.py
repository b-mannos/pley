import http.client
import json
import os
import shutil
import tempfile
import threading
import time
import unittest

import support
from pley import auth
from pley.api import path_in
from pley.proxy import Proxy

SECRET = "s3cret"
MOVIE = bytes(range(256)) * 12_000  # 3 MB


class ProxyTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.mkdtemp()
        self.valid = {"at-1"}
        self.stopped = []
        self.server = support.FakeServer(self.handle)
        self.store = auth.Store(os.path.join(self.dir, "tokens.json"))
        self.store.save(
            {
                "server": self.server.url,
                "server_name": "Test",
                "server_id": "abc",
                "token_endpoint": self.server.url + "/api/oauth/token",
                "revocation_endpoint": self.server.url + "/api/oauth/revoke",
                "access_token": "at-1",
                "expires_at": 4_000_000_000,
                "refresh_token": "rt-1",
                "scope": "library play",
            }
        )
        self.tokens = auth.Tokens(self.store, "pleyKodi/test")
        self.proxy = Proxy(self.tokens, "pleyKodi/test", SECRET, on_stopped=self.stopped.append)
        self.port = self.proxy.start()

    def tearDown(self):
        self.proxy.stop()
        self.server.close()
        shutil.rmtree(self.dir)

    def handle(self, request):
        if request.path == "/api/oauth/token":
            self.valid = {"at-2"}
            return support.answer(200, {"access_token": "at-2", "refresh_token": "rt-2", "expires_in": 3600})
        if request.path == "/api/oauth/revoke":
            return 200, {}, b""
        token = request.headers.get("authorization", "")[len("Bearer ") :]
        if token not in self.valid:
            return support.answer(401, {"code": "unauthorized", "message": "expired"})
        if request.path == "/api/files/7/stream":
            start, end = 0, len(MOVIE) - 1
            if "range" in request.headers:
                first, last = request.headers["range"][len("bytes=") :].split("-")
                start, end = int(first), int(last or end)
                part = MOVIE[start : end + 1]
                headers = {
                    "Content-Type": "video/x-matroska",
                    "Content-Range": "bytes %d-%d/%d" % (start, end, len(MOVIE)),
                    "Accept-Ranges": "bytes",
                    "Content-Length": str(len(part)),
                }
                return 206, headers, part
            return 200, {"Content-Type": "video/x-matroska", "Accept-Ranges": "bytes"}, MOVIE
        if request.path == "/api/playback/s1/12.ts":
            return support.answer(410, {"code": "session_stopped", "message": "Maintenance tonight"})
        if request.path.startswith("/api/progress"):
            return 200, {"Content-Type": "application/json"}, request.body
        return support.answer(200, {"path": request.path, "agent": request.headers.get("user-agent")})

    def ask(self, method, path, host=None, headers=None, body=None):
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        sent = {"Host": host or "127.0.0.1:%d" % self.port}
        sent.update(headers or {})
        conn.request(method, path, body=body, headers=sent)
        response = conn.getresponse()
        data = response.read()
        conn.close()
        return response, data

    def test_requests_reach_the_server_with_the_token(self):
        response, data = self.ask("GET", "/%s/api/libraries?x=1" % SECRET)
        self.assertEqual(response.status, 200)
        self.assertEqual(json.loads(data), {"path": "/api/libraries?x=1", "agent": "pleyKodi/test"})
        self.assertEqual(self.server.requests[-1].headers["authorization"], "Bearer at-1")

    def test_a_refused_token_is_renewed_and_the_request_sent_again(self):
        self.valid = {"at-2"}
        response, data = self.ask("GET", "/%s/api/hubs/home" % SECRET)
        self.assertEqual(response.status, 200, data)
        self.assertEqual(self.store.load()["access_token"], "at-2")
        paths = [r.path for r in self.server.requests]
        self.assertEqual(paths, ["/api/hubs/home", "/api/oauth/token", "/api/hubs/home"])

    def test_bodies_go_through_as_they_are(self):
        body = json.dumps({"position_ms": 5000, "state": "playing"}).encode()
        response, data = self.ask(
            "POST", "/%s/api/progress" % SECRET, headers={"Content-Type": "application/json"}, body=body
        )
        self.assertEqual((response.status, data), (200, body))
        self.assertEqual(self.server.requests[-1].headers["content-type"], "application/json")

    def test_files_stream_whole_or_by_range(self):
        response, data = self.ask("GET", "/%s/api/files/7/stream" % SECRET)
        self.assertEqual((response.status, len(data)), (200, len(MOVIE)))
        self.assertEqual(data, MOVIE)
        self.assertEqual(response.getheader("Accept-Ranges"), "bytes")
        response, data = self.ask("GET", "/%s/api/files/7/stream" % SECRET, headers={"Range": "bytes=1000-1999"})
        self.assertEqual(response.status, 206)
        self.assertEqual(response.getheader("Content-Range"), "bytes 1000-1999/%d" % len(MOVIE))
        self.assertEqual(data, MOVIE[1000:2000])
        response, data = self.ask("HEAD", "/%s/api/files/7/stream" % SECRET)
        self.assertEqual((response.status, data), (200, b""))
        self.assertEqual(response.getheader("Content-Type"), "video/x-matroska")

    def test_a_stream_stopped_on_the_server_is_told(self):
        response, data = self.ask("GET", "/%s/api/playback/s1/12.ts" % SECRET)
        self.assertEqual(response.status, 410)
        self.assertEqual(self.stopped, ["Maintenance tonight"])

    def test_only_kodi_on_this_computer_gets_in(self):
        for host, path in [
            ("pley.attacker.example:%d" % self.port, "/%s/api/libraries" % SECRET),
            (None, "/wrong/api/libraries"),
            (None, "/%s/.well-known/oauth-authorization-server" % SECRET),
        ]:
            response, _ = self.ask("GET", path, host=host)
            self.assertIn(response.status, (403, 404), (host, path))
        self.assertEqual(self.server.requests, [])

    def test_status_and_signing_out(self):
        response, data = self.ask("GET", "/%s/status" % SECRET)
        self.assertEqual(json.loads(data), {"signed_in": True, "server": self.server.url})
        response, data = self.ask("POST", "/%s/sign-out" % SECRET)
        self.assertEqual(json.loads(data)["signed_in"], False)
        self.assertEqual(self.server.requests[-1].path, "/api/oauth/revoke")
        response, data = self.ask("GET", "/%s/api/libraries" % SECRET)
        self.assertEqual((response.status, json.loads(data)["code"]), (401, "signed_out"))

    def test_a_server_out_of_reach_is_said_within_seconds(self):
        # 192.0.2.1 is for documentation: nothing ever answers there, as a
        # server that is switched off doesn't.
        self.store.save(dict(self.store.load(), server="http://192.0.2.1:7539"))
        started = time.monotonic()
        response, data = self.ask("GET", "/%s/api/hubs/home" % SECRET)
        self.assertEqual((response.status, json.loads(data)["code"]), (502, "unreachable"))
        self.assertLess(time.monotonic() - started, 7)

    def test_stopping_ends_every_connection_at_once(self):
        # Kodi kills the service when its threads outlive 5 seconds.
        idle = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        idle.request("GET", "/%s/status" % SECRET, headers={"Host": "127.0.0.1:%d" % self.port})
        idle.getresponse().read()
        streaming = http.client.HTTPConnection("127.0.0.1", self.port, timeout=10)
        streaming.request("GET", "/%s/api/files/7/stream" % SECRET, headers={"Host": "127.0.0.1:%d" % self.port})
        response = streaming.getresponse()
        response.read(1024)
        started = time.monotonic()
        self.proxy.stop()
        self.assertLess(time.monotonic() - started, 2)
        deadline = time.monotonic() + 2
        while time.monotonic() < deadline and self.proxy_threads():
            time.sleep(0.05)
        self.assertEqual(self.proxy_threads(), [])
        idle.close()
        streaming.close()

    def proxy_threads(self):
        return [t for t in threading.enumerate() if t.name.startswith("pley-proxy") and t.is_alive()]

    def test_addresses_the_server_gives_become_the_proxys(self):
        base = "http://127.0.0.1:5000/s3cret"
        self.assertEqual(path_in(base, "/api/items/1/images/poster?v=2&w=500"), base + "/api/items/1/images/poster?v=2&w=500")
        self.assertEqual(path_in(base, "/api/playback/abc/master.m3u8"), base + "/api/playback/abc/master.m3u8")


if __name__ == "__main__":
    unittest.main()
