"""What the add-on's tests share: Kodi's stand-ins on the path, and a
stand-in pley server."""

import http.server
import json
import os
import sys
import threading
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [os.path.join(HERE, "stubs"), os.path.join(HERE, "..", "plugin.video.pley", "resources", "lib")]


class Request:
    def __init__(self, method, path, headers, body):
        self.method = method
        self.path = path
        self.headers = headers
        self.body = body

    def form(self):
        return dict(urllib.parse.parse_qsl(self.body.decode("utf-8")))

    def json(self):
        return json.loads(self.body.decode("utf-8"))


def answer(status, body):
    return status, {"Content-Type": "application/json"}, json.dumps(body).encode("utf-8")


class FakeServer:
    """A stand-in pley server on 127.0.0.1: `handle(request)` gives each
    answer as (status, headers, body)."""

    def __init__(self, handle):
        self.requests = []
        outer = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = "HTTP/1.1"

            def log_message(self, *args):
                pass

            def any(self):
                length = int(self.headers.get("Content-Length") or 0)
                body = self.rfile.read(length) if length else b""
                headers = {k.lower(): v for k, v in self.headers.items()}
                request = Request(self.command, self.path, headers, body)
                outer.requests.append(request)
                status, sent, data = handle(request)
                self.send_response(status)
                for name, value in sent.items():
                    self.send_header(name, value)
                if "Content-Length" not in sent:
                    self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                if self.command != "HEAD":
                    self.wfile.write(data)

            do_GET = do_POST = do_PUT = do_DELETE = do_HEAD = do_PATCH = any

        self.httpd = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.httpd.daemon_threads = True
        # The proxy closing a connection mid-answer is expected.
        self.httpd.handle_error = lambda request, address: None
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()
        self.url = "http://127.0.0.1:%d" % self.httpd.server_address[1]

    def close(self):
        self.httpd.shutdown()
        self.httpd.server_close()


def metadata(issuer):
    return {
        "issuer": issuer,
        "token_endpoint": issuer + "/api/oauth/token",
        "revocation_endpoint": issuer + "/api/oauth/revoke",
        "device_authorization_endpoint": issuer + "/api/oauth/device",
    }
