"""Signing in with a code shown on screen (RFC 8628), and the tokens that
come of it.

The plugin signs in and writes the tokens to a file. From then on only the
service reads and refreshes them, one refresh at a time: each refresh token
works once, and two refreshes with the same one end the sign-in. Plugin
calls reach the server through the service's proxy, which adds the token.
"""

from __future__ import annotations

import json
import os
import threading
import time

from .api import ApiError, Client

CLIENT_ID = "https://hhaidar.github.io/pley/oauth/kodi.json"
DEVICE_GRANT = "urn:ietf:params:oauth:grant-type:device_code"
SCOPE = "library play"
# Access tokens are refreshed this long before they expire.
REFRESH_MARGIN = 300


class SignedOut(Exception):
    """No tokens, or the server ended the sign-in."""


def metadata(server: str, user_agent: str) -> dict:
    """The server's OAuth metadata (RFC 8414), checked to be its own: the
    issuer is the address asked (§3.3), and the endpoints are on it."""
    meta = Client(server, user_agent).get("/.well-known/oauth-authorization-server")
    if not isinstance(meta, dict) or meta.get("issuer") != server:
        raise ApiError(0, "issuer", "the server says it is {}, not {}".format(meta.get("issuer"), server))
    for name in ("token_endpoint", "device_authorization_endpoint", "revocation_endpoint"):
        if not str(meta.get(name, "")).startswith(server + "/"):
            raise ApiError(0, "issuer", "the server's {} isn't on {}".format(name, server))
    return meta


def path_of(server: str, endpoint: str) -> str:
    return endpoint[len(server) :]


class DeviceSignIn:
    """One sign-in with a code: `start`, then `poll` every `interval`."""

    def __init__(self, server: str, user_agent: str):
        self.server = server
        self.user_agent = user_agent
        self.meta = metadata(server, user_agent)
        self.client = Client(server, user_agent)
        self.interval = 5

    def start(self) -> dict:
        path = path_of(self.server, self.meta["device_authorization_endpoint"])
        started = self.client.json("POST", path, form={"client_id": CLIENT_ID, "scope": SCOPE})
        self.interval = max(int(started.get("interval") or 5), 1)
        return started

    def poll(self, device_code: str):
        """("allowed", tokens), or (what to do, None): "pending",
        "slow_down" (the interval is now 5 seconds longer), "denied",
        "expired"."""
        path = path_of(self.server, self.meta["token_endpoint"])
        form = {"grant_type": DEVICE_GRANT, "device_code": device_code, "client_id": CLIENT_ID}
        response = self.client.request("POST", path, form=form)
        if response.status == 200:
            return "allowed", response.json()
        error = (response.json() or {}).get("error") if response.body.startswith(b"{") else None
        if error == "authorization_pending":
            return "pending", None
        if error == "slow_down":
            self.interval += 5
            return "slow_down", None
        if error == "access_denied":
            return "denied", None
        if error in ("expired_token", "invalid_grant"):
            return "expired", None
        raise ApiError(response.status, error or "http_%d" % response.status, "the server refused the code")

    def record(self, tokens: dict, server_name: str, server_id: str, now: float | None = None) -> dict:
        """What the token file keeps of a sign-in."""
        return {
            "server": self.server,
            "server_name": server_name,
            "server_id": server_id,
            "token_endpoint": self.meta["token_endpoint"],
            "revocation_endpoint": self.meta["revocation_endpoint"],
            "access_token": tokens["access_token"],
            "expires_at": (now if now is not None else time.time()) + int(tokens.get("expires_in") or 3600),
            "refresh_token": tokens["refresh_token"],
            "scope": tokens.get("scope", SCOPE),
        }


class Store:
    """The token file, written whole and atomically."""

    def __init__(self, path: str):
        self.path = path

    def stamp(self):
        try:
            stat = os.stat(self.path)
        except OSError:
            return None
        return stat.st_mtime_ns, stat.st_size

    def load(self) -> dict | None:
        try:
            with open(self.path, encoding="utf-8") as f:
                data = json.load(f)
        except (OSError, ValueError):
            return None
        return data if isinstance(data, dict) and data.get("refresh_token") else None

    def save(self, data: dict) -> None:
        temporary = self.path + ".new"
        with open(temporary, "w", encoding="utf-8") as f:
            json.dump(data, f)
        os.replace(temporary, self.path)

    def clear(self) -> None:
        try:
            os.remove(self.path)
        except OSError:
            pass


class Tokens:
    """The service's hold on the sign-in: a current access token, refreshed
    one at a time. A file the plugin wrote since is read again."""

    def __init__(self, store: Store, user_agent: str, clock=time.time):
        self.store = store
        self.user_agent = user_agent
        self.clock = clock
        self.lock = threading.Lock()
        self.data = None
        self.stamp = None

    def current(self) -> dict | None:
        stamp = self.store.stamp()
        if stamp != self.stamp:
            self.data = self.store.load() if stamp else None
            self.stamp = stamp
        return self.data

    def server(self) -> str | None:
        data = self.current()
        return data["server"] if data else None

    def access(self) -> str:
        """An access token good for a while yet."""
        with self.lock:
            data = self.current()
            if not data:
                raise SignedOut()
            if data["expires_at"] - self.clock() > REFRESH_MARGIN:
                return data["access_token"]
            return self._refresh(data)["access_token"]

    def refreshed(self, stale: str) -> str:
        """A new access token after `stale` was refused, unless it was
        refreshed meanwhile."""
        with self.lock:
            data = self.current()
            if not data:
                raise SignedOut()
            if data["access_token"] != stale:
                return data["access_token"]
            return self._refresh(data)["access_token"]

    def _refresh(self, data: dict) -> dict:
        client = Client(data["server"], self.user_agent)
        form = {"grant_type": "refresh_token", "refresh_token": data["refresh_token"], "client_id": CLIENT_ID}
        response = client.request("POST", path_of(data["server"], data["token_endpoint"]), form=form)
        if response.status == 200:
            tokens = response.json()
            renewed = dict(data)
            renewed.update(
                access_token=tokens["access_token"],
                refresh_token=tokens["refresh_token"],
                expires_at=self.clock() + int(tokens.get("expires_in") or 3600),
                scope=tokens.get("scope", data.get("scope")),
            )
            # Saved before it is used: the old refresh token is spent.
            self.store.save(renewed)
            self.data, self.stamp = renewed, self.store.stamp()
            return renewed
        if response.status in (400, 401) and response.body.startswith(b"{"):
            if (response.json() or {}).get("error") in ("invalid_grant", "invalid_client", "unauthorized_client"):
                self.forget()
                raise SignedOut()
        raise ApiError(response.status, "refresh", "the server didn't renew the sign-in (%d)" % response.status)

    def forget(self) -> None:
        self.store.clear()
        self.data, self.stamp = None, None

    def sign_out(self) -> None:
        """Ends the sign-in on the server (best effort), then here."""
        with self.lock:
            data = self.current()
            if data:
                try:
                    form = {"token": data["refresh_token"], "client_id": CLIENT_ID}
                    path = path_of(data["server"], data["revocation_endpoint"])
                    Client(data["server"], self.user_agent).request("POST", path, form=form)
                except ApiError:
                    pass
            self.forget()
