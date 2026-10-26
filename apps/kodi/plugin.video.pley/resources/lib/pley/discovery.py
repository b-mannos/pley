"""Finding pley servers on the local network: a broadcast of `pley?` to
UDP port 7539, which each server answers with its id, name, version and
web port (apps/server/src/discovery.rs)."""

from __future__ import annotations

import json
import socket
import time

from .api import normalize

PORT = 7539
REQUEST = b"pley?"


def parse(data: bytes, address: str):
    """A server's answer, or None for anything else."""
    try:
        answer = json.loads(data.decode("utf-8"))
    except ValueError:
        return None
    if not isinstance(answer, dict) or not answer.get("pley") or not answer.get("id"):
        return None
    port = answer.get("port")
    if not isinstance(port, int) or not 0 < port < 65536:
        return None
    host = "[%s]" % address if ":" in address else address
    return {
        "id": str(answer["id"]),
        "name": str(answer.get("name") or address),
        "version": str(answer.get("version") or ""),
        # Written as the server writes its issuer (port 80 left out).
        "url": normalize("http://%s:%d" % (host, port)),
    }


def find(wait: float = 1.5, targets=(("255.255.255.255", PORT),)) -> list:
    """The servers that answer within `wait` seconds, each once."""
    found = {}
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        sock.bind(("", 0))
        for target in targets:
            try:
                sock.sendto(REQUEST, target)
            except OSError:
                pass
        end = time.monotonic() + wait
        while True:
            left = end - time.monotonic()
            if left <= 0:
                break
            sock.settimeout(left)
            try:
                data, (address, _) = sock.recvfrom(2048)
            except (socket.timeout, ConnectionResetError):
                # Windows reports an earlier send's ICMP "port unreachable" here.
                continue
            except OSError:
                break
            server = parse(data, address)
            if server and server["id"] not in found:
                found[server["id"]] = server
    return sorted(found.values(), key=lambda s: s["name"].lower())
