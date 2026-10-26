import unittest

import support
from pley import api


class NormalizeTest(unittest.TestCase):
    def test_addresses_are_written_as_pley_writes_its_issuer(self):
        cases = {
            "192.168.1.5": "http://192.168.1.5:7539",
            "192.168.1.5:7539": "http://192.168.1.5:7539",
            "HTTP://NAS.local:8096/": "http://nas.local:8096",
            "http://nas.local:80": "http://nas.local",
            "https://pley.example.com": "https://pley.example.com",
            "https://pley.example.com:443/": "https://pley.example.com",
            "http://[FE80::1]:7539": "http://[fe80::1]:7539",
            " nas:7539 ": "http://nas:7539",
        }
        for typed, written in cases.items():
            self.assertEqual(api.normalize(typed), written, typed)

    def test_anything_else_is_refused(self):
        for typed in ["ftp://nas", "http://", "http://nas/pley", "http://user@nas", "http://nas:99999", "nas?x=1"]:
            with self.assertRaises(ValueError, msg=typed):
                api.normalize(typed)


class NotPleyTest(unittest.TestCase):
    def test_a_web_page_is_said_to_be_something_else(self):
        server = support.FakeServer(lambda request: (200, {"Content-Type": "text/html"}, b"<html>router</html>"))
        try:
            with self.assertRaises(api.ApiError) as refused:
                api.Client(server.url, "pleyKodi/test").get("/.well-known/oauth-authorization-server")
            self.assertEqual(refused.exception.code, "not_pley")
        finally:
            server.close()


class ErrorsTest(unittest.TestCase):
    def test_both_kinds_of_refusal_are_read(self):
        pley = api.error_of(api.Response(404, {}, b'{"code": "not_found", "message": "item 42 not found"}'))
        self.assertEqual((pley.status, pley.code, pley.message), (404, "not_found", "item 42 not found"))
        oauth = api.error_of(api.Response(400, {}, b'{"error": "invalid_grant", "error_description": "used"}'))
        self.assertEqual((oauth.code, oauth.message), ("invalid_grant", "used"))
        bare = api.error_of(api.Response(502, {}, b"<html>bad gateway</html>"))
        self.assertEqual((bare.code, bare.message), ("http_502", "the server answered 502"))


if __name__ == "__main__":
    unittest.main()
