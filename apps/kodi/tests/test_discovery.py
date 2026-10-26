import json
import socket
import threading
import unittest

import support  # noqa: F401
from pley import discovery


class DiscoveryTest(unittest.TestCase):
    def test_answers_are_read_and_anything_else_ignored(self):
        answer = json.dumps({"pley": 1, "id": "abc", "name": "Living room", "version": "0.2.0", "port": 7539})
        self.assertEqual(
            discovery.parse(answer.encode(), "192.168.1.5"),
            {"id": "abc", "name": "Living room", "version": "0.2.0", "url": "http://192.168.1.5:7539"},
        )
        self.assertEqual(discovery.parse(answer.encode(), "fe80::1")["url"], "http://[fe80::1]:7539")
        # On port 80 the address is written as the server writes its issuer.
        on_80 = json.dumps({"pley": 1, "id": "abc", "name": "NAS", "port": 80})
        self.assertEqual(discovery.parse(on_80.encode(), "192.168.1.5")["url"], "http://192.168.1.5")
        for junk in [b"pley?", b"{}", b'{"pley": 1, "id": "x", "port": 0}', b'{"id": "x", "port": 1}', b"\xff"]:
            self.assertIsNone(discovery.parse(junk, "10.0.0.1"), junk)

    def test_servers_that_answer_are_found_once(self):
        server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        server.bind(("127.0.0.1", 0))
        port = server.getsockname()[1]
        heard = []

        def respond():
            data, sender = server.recvfrom(64)
            heard.append(data)
            for name in ("Living room", "Living room"):
                body = {"pley": 1, "id": "abc", "name": name, "version": "0.2.0", "port": 7539}
                server.sendto(json.dumps(body).encode(), sender)
            server.sendto(b"not pley", sender)

        threading.Thread(target=respond, daemon=True).start()
        found = discovery.find(wait=0.5, targets=(("127.0.0.1", port),))
        server.close()
        self.assertEqual(heard, [b"pley?"])
        self.assertEqual([s["name"] for s in found], ["Living room"])
        self.assertEqual(found[0]["url"], "http://127.0.0.1:7539")


if __name__ == "__main__":
    unittest.main()
