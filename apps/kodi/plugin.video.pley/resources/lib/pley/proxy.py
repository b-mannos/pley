"""The service's proxy on 127.0.0.1: everything Kodi asks pley for goes
through it, and it adds the access token.

pley takes an app's token in the Authorization header only, never in an
address, and it lasts an hour. Kodi's player and its artwork cache only
know addresses: the proxy gives them addresses of its own,
http://127.0.0.1:<port>/<secret>/api/…, and asks the server with the
current token, renewing it when the server refuses it. The secret keeps
other programs (and web pages) on this computer out.
"""

from __future__ import annotations

import base64
import http.client
import json
import mimetypes
import os
import socket
import threading

# Python 3.8's http.server reads the system's MIME types as it loads (on
# Windows from the registry, about half a second of a Kodi start); the
# proxy never uses them.
mimetypes.inited = True
import http.server  # noqa: E402

from . import api
from .auth import SignedOut, Tokens

# What passes from Kodi to the server, and back.
REQUEST_HEADERS = ("range", "if-range", "if-none-match", "if-modified-since", "content-type", "accept")
RESPONSE_HEADERS = (
    "content-type",
    "content-length",
    "content-range",
    "accept-ranges",
    "etag",
    "last-modified",
    "cache-control",
    "x-pley-complete",
)
MAX_BODY = 1 << 20
CHUNK = 256 * 1024
# Segments of a conversion can take a while to be made.
UPSTREAM_TIMEOUT = 120


def new_secret() -> str:
    """192 random bits, as secrets.token_urlsafe(24) makes them (without
    loading what secrets loads)."""
    return base64.urlsafe_b64encode(os.urandom(24)).decode("ascii").rstrip("=")


class Proxy:
    """Serves on 127.0.0.1:`port` (another one when it is taken) until
    `stop`. `on_stopped(message)` hears of streams an administrator
    stopped."""

    def __init__(self, tokens: Tokens, user_agent: str, secret: str, port: int = 0, on_stopped=None):
        self.tokens = tokens
        self.user_agent = user_agent
        self.secret = secret
        self.on_stopped = on_stopped
        self.server = None
        self.thread = None
        self.wanted_port = port
        # Every socket in use, Kodi's and the server's: Kodi waits for each
        # of the service's threads when it quits, and kills the service
        # after 5 seconds.
        self.lock = threading.Lock()
        self.sockets = set()

    @property
    def port(self) -> int:
        return self.server.server_address[1]

    @property
    def base(self) -> str:
        return "http://127.0.0.1:%d/%s" % (self.port, self.secret)

    def start(self) -> int:
        handler = type("Handler", (_Handler,), {"proxy": self})
        try:
            self.server = _Server(("127.0.0.1", self.wanted_port), handler)
        except OSError:
            self.server = _Server(("127.0.0.1", 0), handler)
        self.thread = threading.Thread(target=self.server.serve_forever, name="pley-proxy", daemon=True)
        self.thread.start()
        return self.port

    def stop(self) -> None:
        if self.server:
            self.server.shutdown()
            self.server.server_close()
        with self.lock:
            open_now, self.sockets = self.sockets, set()
        for sock in open_now:
            hard_close(sock)
        if self.thread:
            self.thread.join(5)

    def opened(self, sock) -> None:
        with self.lock:
            self.sockets.add(sock)

    def closed(self, sock) -> None:
        with self.lock:
            self.sockets.discard(sock)


class _Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def process_request(self, request, client_address):
        thread = threading.Thread(
            target=self.process_request_thread, args=(request, client_address), name="pley-proxy-request", daemon=True
        )
        thread.start()

    def handle_error(self, request, client_address):
        # Connections Kodi or the proxy closed: nothing to report.
        pass


class _Upstream:
    """One request to the server, its socket known to the proxy until it
    is closed (http.client hands the socket to the answer and forgets it)."""

    def __init__(self, proxy: Proxy, server: str):
        self.proxy = proxy
        self.server = server
        self.conn = None
        self.sock = None
        self.response = None

    def send(self, method: str, path: str, body, headers: dict, token: str) -> http.client.HTTPResponse:
        sent = dict(headers)
        sent["Authorization"] = "Bearer " + token
        # Connected within seconds, or the server is out of reach; answers
        # may take longer (a conversion's segments).
        self.conn = api.connect(self.server, UPSTREAM_TIMEOUT)
        self.sock = self.conn.sock
        self.proxy.opened(self.sock)
        self.conn.request(method, path, body=body, headers=sent)
        self.response = self.conn.getresponse()
        return self.response

    def close(self) -> None:
        if self.response is not None:
            self.response.close()
        if self.conn is not None:
            self.conn.close()
        if self.sock is not None:
            self.proxy.closed(self.sock)
            self.sock.close()


class _Handler(http.server.BaseHTTPRequestHandler):
    proxy: Proxy
    protocol_version = "HTTP/1.1"
    # Idle connections from Kodi end after a minute.
    timeout = 60

    def setup(self):
        super().setup()
        self.proxy.opened(self.connection)

    def finish(self):
        try:
            super().finish()
        finally:
            self.proxy.closed(self.connection)

    def do_GET(self):
        self.forward()

    def do_HEAD(self):
        self.forward()

    def do_POST(self):
        self.forward()

    def do_PUT(self):
        self.forward()

    def do_PATCH(self):
        self.forward()

    def do_DELETE(self):
        self.forward()

    def log_message(self, format, *args):
        pass

    def answer(self, status: int, body: dict) -> None:
        data = json.dumps(body).encode("utf-8")
        try:
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            if self.command != "HEAD":
                self.wfile.write(data)
        except OSError:
            # Kodi is gone (or the proxy is stopping).
            self.close_connection = True

    def forward(self) -> None:
        proxy = self.proxy
        host = (self.headers.get("Host") or "").lower()
        if host not in ("127.0.0.1:%d" % proxy.port, "localhost:%d" % proxy.port):
            return self.answer(403, {"code": "forbidden", "message": "not from this computer"})
        prefix = "/" + proxy.secret + "/"
        if not self.path.startswith(prefix):
            return self.answer(404, {"code": "not_found", "message": "not found"})
        rest = self.path[len(prefix) - 1 :]
        if rest == "/status":
            data = proxy.tokens.current()
            return self.answer(200, {"signed_in": bool(data), "server": data["server"] if data else None})
        if rest == "/sign-out" and self.command == "POST":
            # Here, under the refresh lock: a refresh can't bring it back.
            proxy.tokens.sign_out()
            return self.answer(200, {"signed_in": False, "server": None})
        if not rest.startswith("/api/"):
            return self.answer(404, {"code": "not_found", "message": "not found"})
        length = int(self.headers.get("Content-Length") or 0)
        if length > MAX_BODY:
            return self.answer(413, {"code": "bad_request", "message": "too large"})
        body = self.rfile.read(length) if length else None
        headers = {"User-Agent": proxy.user_agent}
        for name in REQUEST_HEADERS:
            if self.headers.get(name):
                headers[name] = self.headers[name]
        upstream = None
        try:
            try:
                token = proxy.tokens.access()
                server = proxy.tokens.server()
                upstream = _Upstream(proxy, server)
                response = upstream.send(self.command, rest, body, headers, token)
                if response.status == 401:
                    upstream.close()
                    token = proxy.tokens.refreshed(token)
                    upstream = _Upstream(proxy, server)
                    response = upstream.send(self.command, rest, body, headers, token)
            except SignedOut:
                return self.answer(401, {"code": "signed_out", "message": "pley is signed out"})
            except (api.ApiError, OSError, http.client.HTTPException) as e:
                return self.answer(502, {"code": "unreachable", "message": str(e)})
            self.relay(response)
        finally:
            if upstream:
                upstream.close()

    def relay(self, response: http.client.HTTPResponse) -> None:
        try:
            if response.status == 410 and self.proxy.on_stopped:
                data = response.read()
                try:
                    message = json.loads(data.decode("utf-8")).get("message")
                except ValueError:
                    message = None
                self.proxy.on_stopped(message)
                self.send_response(410)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            self.send_response(response.status)
            length = response.getheader("Content-Length")
            for name in RESPONSE_HEADERS:
                value = response.getheader(name)
                if value is not None:
                    self.send_header(name, value)
            if length is None and self.command != "HEAD" and response.status not in (204, 304):
                # Without a length the answer ends when the connection does.
                self.send_header("Connection", "close")
                self.close_connection = True
            self.end_headers()
            if self.command == "HEAD" or response.status in (204, 304):
                return
            while True:
                chunk = response.read(CHUNK)
                if not chunk:
                    break
                self.wfile.write(chunk)
        except (OSError, http.client.HTTPException):
            # Kodi let go (a seek, a stop), the server did, or the proxy is
            # stopping: the connection ends, and Kodi asks again from where
            # it got to.
            self.close_connection = True


def hard_close(sock: socket.socket) -> None:
    """Closes `sock` under a thread blocked on it, which then wakes with an
    error: shutdown wakes it on Linux and macOS, only closing the socket
    itself does on Windows. Python's close() waits for the files made of
    the socket, so the handle is taken from it and closed."""
    try:
        sock.shutdown(socket.SHUT_RDWR)
    except OSError:
        pass
    try:
        handle = sock.detach()
        if handle != -1:
            socket.socket(fileno=handle).close()
    except OSError:
        pass
