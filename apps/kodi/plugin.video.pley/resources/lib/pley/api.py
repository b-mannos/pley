"""HTTP to a pley server, or to the service's proxy in front of one."""

from __future__ import annotations

import json
import socket
import urllib.parse

DEFAULT_PORT = 7539
TIMEOUT = 20
# A server that is off answers nothing at all: waiting longer than this to
# connect tells nobody more.
CONNECT_TIMEOUT = 4


class ApiError(Exception):
    """A refusal from the server ({code, message}, or OAuth's {error,
    error_description}), or no answer at all (status 0)."""

    def __init__(self, status: int, code: str, message: str):
        super().__init__(message)
        self.status = status
        self.code = code
        self.message = message


class Response:
    def __init__(self, status: int, headers: dict, body: bytes):
        self.status = status
        self.headers = headers
        self.body = body

    def json(self):
        return json.loads(self.body.decode("utf-8")) if self.body else None


def normalize(address: str) -> str:
    """A server's address as pley writes its issuer: scheme, lower-case host
    and port only, the default port left out. An address typed without a
    scheme is http, and http without a port is pley's port, 7539."""
    address = address.strip()
    if "://" not in address:
        address = "http://" + address
    parts = urllib.parse.urlsplit(address)
    if parts.scheme not in ("http", "https") or not parts.hostname or parts.username or parts.password:
        raise ValueError("not a server address: " + address)
    if parts.path.strip("/") or parts.query or parts.fragment:
        raise ValueError("a server address has no path: " + address)
    try:
        port = parts.port
    except ValueError:
        raise ValueError("not a server address: " + address)
    default = 443 if parts.scheme == "https" else 80
    if port is None:
        port = default if parts.scheme == "https" else DEFAULT_PORT
    host = parts.hostname.lower()
    if ":" in host:
        host = "[" + host + "]"
    return "{}://{}{}".format(parts.scheme, host, "" if port == default else ":%d" % port)


def connect(base: str, timeout: float = TIMEOUT):
    """An http.client connection to `base`, made within CONNECT_TIMEOUT, that
    then waits up to `timeout` for answers."""
    # Loaded when first needed: with ssl and email it is most of what a
    # listing loads, and the plugin's own requests go without it (local).
    import http.client

    parts = urllib.parse.urlsplit(base)
    kind = http.client.HTTPSConnection if parts.scheme == "https" else http.client.HTTPConnection
    conn = kind(parts.hostname, parts.port, timeout=CONNECT_TIMEOUT)
    conn.connect()
    conn.sock.settimeout(timeout)
    return conn


def error_of(response: Response) -> ApiError:
    try:
        body = response.json() or {}
    except ValueError:
        body = {}
    if not isinstance(body, dict):
        body = {}
    code = body.get("code") or body.get("error") or "http_%d" % response.status
    message = body.get("message") or body.get("error_description") or "the server answered %d" % response.status
    return ApiError(response.status, code, message)


class Client:
    """Requests to `base` (a server, or the proxy's address with its secret
    path), with `token` as a bearer token when given."""

    def __init__(
        self, base: str, user_agent: str, token: str | None = None, timeout: float = TIMEOUT, local: bool = False
    ):
        self.base = base.rstrip("/")
        self.prefix = urllib.parse.urlsplit(self.base).path
        self.user_agent = user_agent
        self.token = token
        self.timeout = timeout
        # The service's proxy, on this computer: plain HTTP over a socket.
        self.local = local

    def request(
        self,
        method: str,
        path: str,
        query: dict | None = None,
        json_body=None,
        form: dict | None = None,
        headers: dict | None = None,
    ) -> Response:
        target = self.prefix + path
        if query:
            pairs = [(k, v) for k, v in query.items() if v is not None]
            target += ("&" if "?" in target else "?") + urllib.parse.urlencode(pairs)
        sent = {"User-Agent": self.user_agent, "Accept": "application/json"}
        if self.token:
            sent["Authorization"] = "Bearer " + self.token
        body = None
        if json_body is not None:
            body = json.dumps(json_body).encode("utf-8")
            sent["Content-Type"] = "application/json"
        elif form is not None:
            body = urllib.parse.urlencode(form).encode("utf-8")
            sent["Content-Type"] = "application/x-www-form-urlencoded"
        sent.update(headers or {})
        if self.local:
            return local_request(self.base, method, target, sent, body, self.timeout)
        import http.client

        conn = None
        try:
            conn = connect(self.base, self.timeout)
            conn.request(method, target, body=body, headers=sent)
            answer = conn.getresponse()
            data = answer.read()
            return Response(answer.status, {k.lower(): v for k, v in answer.getheaders()}, data)
        except (OSError, http.client.HTTPException) as e:
            raise ApiError(0, "unreachable", str(e))
        finally:
            if conn is not None:
                conn.close()

    def json(self, method: str, path: str, **kwargs):
        response = self.request(method, path, **kwargs)
        if response.status >= 400:
            raise error_of(response)
        return response.json()

    def get(self, path: str, **query):
        return self.json("GET", path, query=query or None)

    def post(self, path: str, body=None):
        return self.json("POST", path, json_body=body if body is not None else {})

    def put(self, path: str, body=None):
        return self.json("PUT", path, json_body=body if body is not None else {})

    def delete(self, path: str):
        return self.json("DELETE", path)


def path_in(base: str, url: str) -> str:
    """The proxy's address for an address the server gave ("/api/…")."""
    parts = urllib.parse.urlsplit(url)
    return base + parts.path + ("?" + parts.query if parts.query else "")


def local_request(base: str, method: str, target: str, headers: dict, body, timeout: float) -> Response:
    """One request to the service's proxy: HTTP/1.1 with the connection
    closed after the answer, which the proxy always sends whole (with its
    length, or ending with the connection)."""
    parts = urllib.parse.urlsplit(base)
    lines = ["%s %s HTTP/1.1" % (method, target), "Host: %s:%d" % (parts.hostname, parts.port), "Connection: close"]
    lines += ["%s: %s" % (name, value) for name, value in headers.items()]
    if body is not None:
        lines.append("Content-Length: %d" % len(body))
    request = ("\r\n".join(lines) + "\r\n\r\n").encode("latin-1") + (body or b"")
    chunks = []
    try:
        with socket.create_connection((parts.hostname, parts.port), timeout=CONNECT_TIMEOUT) as sock:
            sock.settimeout(timeout)
            sock.sendall(request)
            while True:
                chunk = sock.recv(65536)
                if not chunk:
                    break
                chunks.append(chunk)
    except OSError as e:
        raise ApiError(0, "unreachable", str(e))
    head, _, rest = b"".join(chunks).partition(b"\r\n\r\n")
    status_line, *header_lines = head.decode("latin-1").split("\r\n")
    try:
        status = int(status_line.split(" ")[1])
    except (IndexError, ValueError):
        raise ApiError(0, "unreachable", "no answer from pley's service")
    answer_headers = {}
    for line in header_lines:
        name, _, value = line.partition(":")
        answer_headers[name.strip().lower()] = value.strip()
    length = answer_headers.get("content-length")
    return Response(status, answer_headers, rest[: int(length)] if length and length.isdigit() else rest)
