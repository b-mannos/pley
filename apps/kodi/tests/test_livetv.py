import json
import time
import unittest
from unittest import mock

import support
import test_window
import xbmc
import xbmcgui
import xbmcplugin
from pley import kodi, livetv, plugin, window
from pley import layout as L

BASE = "http://127.0.0.1:5000/s"
# A quarter past an hour, the programmes on from the hour to the next.
NOW = (int(time.time()) // 3600) * 3600 + 900
HOUR = 3600


def program(channel_id, start, stop, title, **more):
    found = {
        "channel_id": channel_id, "start": start, "stop": stop, "title": title, "subtitle": None, "description": None,
        "categories": [], "image": None, "season": None, "episode": None, "is_new": False, "is_live": False, "rating": None,
    }
    found.update(more)
    return found


NEWS = {
    "id": 1, "source_id": 1, "name": "Pley News HD", "number": "101", "group": "News", "logo": "/api/live/channels/1/logo",
    "favorite": False, "has_guide": True,
    "now": program(1, NOW - 900, NOW + 2700, "Morning Briefing", subtitle="Part 7", description="Everything you need to know.",
                   categories=["News"], season=1, episode=7, is_live=True, image="/api/live/channels/1/programs/1/image"),
    "next": program(1, NOW + 2700, NOW + 5400, "World at One"),
}
SPORT = {
    "id": 2, "source_id": 1, "name": "Sport 1080i", "number": "102", "group": "Sport", "logo": None, "favorite": True,
    "has_guide": True, "now": program(2, NOW - 900, NOW + 2700, "Match of the Day"), "next": None,
}
RADIO = {"id": 4, "source_id": 1, "name": "Radio Jazz", "number": "104", "group": "Radio", "logo": None, "favorite": False,
         "has_guide": False, "now": None, "next": None}
WEATHER = dict(RADIO, id=5, name="Weather Now", number="105", group="News")
CHANNELS = [NEWS, SPORT, RADIO, WEATHER]


def clock(ts):
    return time.strftime("%H:%M", time.localtime(ts))


class WhatsOnTest(unittest.TestCase):
    def test_now_and_next(self):
        self.assertIs(livetv.on_now(NEWS, NOW), NEWS["now"])
        self.assertIs(livetv.up_next(NEWS, NOW), NEWS["next"])
        # Listed a while ago: once the next has begun, it is what's on.
        later = NOW + 3000
        self.assertIs(livetv.on_now(NEWS, later), NEWS["next"])
        self.assertIsNone(livetv.up_next(NEWS, later))
        self.assertIsNone(livetv.on_now(RADIO, NOW))
        self.assertAlmostEqual(livetv.progress(NEWS["now"], NOW), 0.25)

    def test_times_as_kodi_writes_them(self):
        self.assertEqual(livetv.time_format(), "%H:%M", "no seconds")
        self.assertEqual(livetv.span(NEWS["now"]), "%s – %s" % (clock(NOW - 900), clock(NOW + 2700)))
        xbmc.region["time"] = "%I:%M:%S %p"
        self.addCleanup(xbmc.region.update, {"time": "%H:%M:%S"})
        self.assertEqual(livetv.time_format(), "%I:%M %p")
        self.assertFalse(livetv.clock(NOW).startswith("0"), "8:15 PM, as the web writes it")
        self.assertEqual(livetv.left(NEWS["now"], NOW), "45 min left")
        self.assertEqual(livetv.episode(NEWS["now"]), "S1 · E7")
        self.assertEqual(livetv.badges(NEWS["now"]), ["Live"])
        self.assertEqual(livetv.headline(NEWS["now"]), "Morning Briefing: Part 7")
        self.assertEqual(livetv.name(NEWS), "101 Pley News HD")
        self.assertEqual(livetv.name(dict(NEWS, number=None)), "Pley News HD")

    def test_days(self):
        self.assertEqual(livetv.day(NOW, NOW), "Today")
        self.assertEqual(livetv.day(NOW + 86_400, NOW), "Tomorrow")
        self.assertEqual(livetv.day(NOW - 86_400, NOW), "Yesterday")
        later = time.localtime(NOW + 3 * 86_400)
        self.assertEqual(livetv.day(NOW + 3 * 86_400, NOW), "%s %d %s" % (livetv.ENGLISH_DAYS[later.tm_wday], later.tm_mday, livetv.ENGLISH_MONTHS[later.tm_mon - 1]))

    def test_what_kodis_info_says(self):
        text = livetv.plot(NEWS, NOW)
        self.assertTrue(text.startswith("[B]%s – %s[/B]  45 min left · S1 · E7 · Live" % (clock(NOW - 900), clock(NOW + 2700))), text)
        self.assertIn("Everything you need to know.", text)
        self.assertIn("[B]Next[/B]  %s  World at One" % clock(NOW + 2700), text)
        self.assertEqual(livetv.plot(RADIO, NOW), "No guide")
        self.assertEqual(livetv.plot(dict(RADIO, has_guide=True), NOW), "Nothing in the guide now")

    def test_tabs(self):
        self.assertEqual(livetv.groups(CHANNELS), [("News", 2), ("Sport", 1), ("Radio", 1)])
        self.assertEqual(livetv.shown(CHANNELS, "favorites"), [SPORT])
        self.assertEqual(livetv.shown(CHANNELS, "recent", [4, 99, 1]), [RADIO, NEWS], "newest first, gone ones left out")
        self.assertEqual(livetv.shown(CHANNELS, "group", group="News"), [NEWS, WEATHER])
        self.assertEqual(livetv.shown(CHANNELS, "all"), CHANNELS)
        self.assertIs(livetv.by_number(CHANNELS, "104"), RADIO)
        self.assertIs(livetv.by_number(CHANNELS, "10"), NEWS, "the first whose number starts so")
        self.assertIsNone(livetv.by_number(CHANNELS, "7"))

    def test_one_channels_programmes_out_of_the_whole_guide(self):
        guide = {"from": NOW, "to": NOW + 6 * HOUR, "programs": {
            "11": [program(11, NOW, NOW + HOUR, "Not this one", description='Says "1":[ in its text')],
            "1": [NEWS["now"], NEWS["next"]],
        }}
        body = json.dumps(guide, separators=(",", ":")).encode("utf-8")
        self.assertEqual(livetv.programs_of(body, 1), [NEWS["now"], NEWS["next"]])
        self.assertEqual(livetv.programs_of(body, 11)[0]["title"], "Not this one")
        self.assertEqual(livetv.programs_of(body, 7), [])


class ItemsTest(unittest.TestCase):
    def setUp(self):
        xbmc.PlayList.entries.clear()

    def test_a_channel_in_kodis_lists(self):
        li = livetv.list_item(BASE, NEWS, NOW)
        self.assertEqual((li.label, li.label2), ("101 Pley News HD", "Morning Briefing: Part 7"))
        self.assertEqual(li.art["thumb"], BASE + "/api/live/channels/1/logo?w=320")
        self.assertEqual(li.art["fanart"], BASE + "/api/live/channels/1/programs/1/image?w=640")
        tag = li.video.values
        self.assertEqual((tag["setMediaType"], tag["setTitle"], tag["setTagLine"], tag["setGenres"]), ("video", "101 Pley News HD", "Morning Briefing: Part 7", ["News"]))
        # A stream without an end: Kodi counts one it stopped as played, and
        # keeps that by its address; a channel is never watched, nor resumed.
        self.assertEqual((tag["setPlaycount"], tag["setResumePoint"]), (0, (0.0, 0.0)))
        self.assertNotIn("setDuration", tag)
        self.assertEqual(li.properties["IsPlayable"], "true")

    def test_a_channel_plays_as_the_provider_sends_it(self):
        li = livetv.play_item(BASE, NEWS, "kodi-1", NOW)
        self.assertEqual(li.path, BASE + "/api/live/channels/1/stream.ts?client_id=kodi-1")
        self.assertEqual((li.mime, li.lookup), ("video/mp2t", False))

    def test_the_queue_holds_the_channels_around_the_one_played(self):
        many = [dict(RADIO, id=i, number=str(i)) for i in range(1, 501)]
        playlist, at = livetv.queue(BASE, many, 300, NOW)
        entries = xbmc.PlayList.entries[xbmc.PLAYLIST_VIDEO]
        self.assertEqual(len(entries), 2 * livetv.QUEUED + 1)
        self.assertEqual(entries[at][0], "plugin://plugin.video.pley/?route=live_play&id=301")
        playlist, at = livetv.queue(BASE, many, 3, NOW)
        self.assertEqual((at, len(xbmc.PlayList.entries[xbmc.PLAYLIST_VIDEO])), (3, livetv.QUEUED + 4))

    def test_only_a_queue_of_channels_is_let_go(self):
        mine = xbmc.PlayList(xbmc.PLAYLIST_VIDEO)
        mine.add("plugin://plugin.video.other/?play=1")
        livetv.forget_queue()
        self.assertEqual(mine.size(), 1, "someone else's queue stays")
        livetv.queue(BASE, CHANNELS, 0, NOW)
        livetv.forget_queue()
        self.assertEqual(mine.size(), 0)


class ServiceTest(unittest.TestCase):
    """What the service's proxy hears of a channel Kodi plays."""

    def setUp(self):
        from pley import service

        xbmc.PlayList.entries.clear()
        del xbmcgui.notified[:]
        del xbmcgui.boxes[:]
        self.player = service.Player(None, "pleyKodi/test", None)

    def test_a_channel_the_server_ended_doesnt_tune_the_next(self):
        livetv.queue(BASE, CHANNELS, 0, NOW)
        self.player.live_ended()
        self.assertEqual(xbmc.PlayList(xbmc.PLAYLIST_VIDEO).size(), 0)
        self.assertEqual(xbmcgui.notified, [("Pley", "The server stopped this stream.")])

    def test_a_channel_refused_on_its_way_to_kodi_is_said_once(self):
        livetv.queue(BASE, CHANNELS, 0, NOW)
        self.player.busy("The one connection is in use.")
        self.player.busy("The one connection is in use.")
        for _ in range(50):
            if xbmcgui.boxes:
                break
            time.sleep(0.02)
        time.sleep(0.1)
        self.assertEqual(xbmcgui.boxes, [("All connections are in use", "The one connection is in use.")])
        self.assertEqual(xbmc.PlayList(xbmc.PLAYLIST_VIDEO).size(), 0)


class RoutesTest(unittest.TestCase):
    """Live TV in Kodi's lists and the play route, against a stand-in server."""

    def setUp(self):
        clock_patch = mock.patch("time.time", return_value=NOW)
        clock_patch.start()
        self.addCleanup(clock_patch.stop)
        self.busy = False
        self.server = support.FakeServer(self.handle)
        self.addCleanup(self.server.close)
        kodi.set_property(kodi.PROXY, self.server.url + "/s3cret")
        self.addCleanup(kodi.clear_property, kodi.PROXY)
        self.addCleanup(kodi.xbmcaddon.settings.pop, "_profile", None)
        import tempfile

        kodi.xbmcaddon.settings["_profile"] = tempfile.mkdtemp()
        del xbmcplugin.calls[:]
        del xbmcgui.boxes[:]
        xbmc.PlayList.entries.clear()

    def handle(self, request):
        path = request.path[len("/s3cret") :]
        if path == "/api/live":
            return support.answer(200, {"available": True, "allowed": True, "channels": 4, "sources": 0, "recent": [2]})
        if path == "/api/live/channels":
            return support.answer(200, CHANNELS)
        if path == "/api/live/channels/1":
            return support.answer(200, NEWS)
        if path.startswith("/api/live/guide"):
            return support.answer(200, {"from": 0, "to": 0, "programs": {"1": [NEWS["now"], NEWS["next"]]}})
        if path == "/api/live/channels/1/watch":
            if self.busy:
                return support.answer(409, {"code": "conflict", "message": "The one connection of Test Provider is in use, watching Sport 1080i."})
            return support.answer(200, {"session_id": "w1", "url": "/api/live/sessions/w1/index.m3u8", "kind": "hls", "channel": NEWS})
        if path == "/api/live/channels/2/favorite":
            return 204, {}, b""
        return support.answer(404, {"code": "not_found", "message": path})

    def run_route(self, query):
        plugin.Plugin(["plugin://plugin.video.pley/", "1", "?" + query]).run()
        return [c for c in xbmcplugin.calls if c[0] == "item"]

    def test_live_tv_lists_what_it_has(self):
        items = self.run_route("route=live")
        self.assertEqual([c[2].label for c in items], ["Channels", "Favourites", "Recently watched", "Groups", "Guide"])
        del xbmcplugin.calls[:]
        items = self.run_route("route=live_groups")
        self.assertEqual([c[2].label for c in items], ["News (2)", "Sport (1)", "Radio (1)"])

    def test_a_tabs_channels_play_and_have_a_menu(self):
        items = self.run_route("route=live_channels&tab=group&group=News")
        self.assertEqual([c[2].label for c in items], ["101 Pley News HD", "105 Weather Now"])
        url, li, folder = items[0][1:]
        self.assertEqual((url, folder), ("plugin://plugin.video.pley/?route=live_play&id=1", False))
        self.assertEqual([name for name, _ in li.menu], ["Add to favourites", "Schedule"])
        self.assertIn(("content", "videos"), xbmcplugin.calls)
        del xbmcplugin.calls[:]
        self.assertEqual([c[2].label for c in self.run_route("route=live_channels&tab=recent")], ["102 Sport 1080i"])

    def test_a_lineup_comes_a_page_at_a_time(self):
        with mock.patch.object(plugin, "PAGE", 3):
            items = self.run_route("route=live_guide")
            self.assertEqual([c[2].label for c in items], ["101 Pley News HD", "102 Sport 1080i", "104 Radio Jazz", "Next page"])
            self.assertTrue(items[0][3], "the guide's channels open their schedules")
            self.assertIn("offset=3", items[-1][1])
            del xbmcplugin.calls[:]
            items = self.run_route("route=live_guide&offset=3")
        self.assertEqual([c[2].label for c in items], ["105 Weather Now"])

    def test_a_channels_schedule(self):
        items = self.run_route("route=live_schedule&id=1")
        on, later, more = items
        self.assertEqual(on[2].label, "On now  Morning Briefing: Part 7")
        self.assertEqual((on[1], on[3]), ("plugin://plugin.video.pley/?route=live_play&id=1", False))
        self.assertEqual(later[2].label, "Today %s  World at One" % clock(NOW + 2700))
        self.assertNotIn("IsPlayable", later[2].properties)
        self.assertEqual(more[2].label, "Later")
        self.assertIn("from=%d" % (NOW + 5400), more[1])

    def test_playing_a_channel(self):
        self.run_route("route=live_play&id=1")
        resolved = [c for c in xbmcplugin.calls if c[0] == "resolved"]
        self.assertEqual(len(resolved), 1)
        _, ok, li = resolved[0]
        self.assertTrue(ok)
        self.assertTrue(li.path.startswith(self.server.url + "/s3cret/api/live/channels/1/stream.ts?client_id=kodi-"))
        watch = next(r for r in self.server.requests if r.path.endswith("/watch"))
        self.assertEqual(watch.json()["client_id"], li.path.rsplit("=", 1)[1], "the same device for both")
        self.assertNotIn(("end", False), xbmcplugin.calls, "nothing listed: Kodi plays")

    def test_every_connection_in_use_is_said_and_the_queue_let_go(self):
        self.busy = True
        livetv.queue(BASE, CHANNELS, 0, NOW)
        self.run_route("route=live_play&id=1")
        self.assertEqual([c[:2] for c in xbmcplugin.calls if c[0] == "resolved"], [("resolved", False)])
        self.assertEqual(xbmcgui.boxes, [("All connections are in use", "The one connection of Test Provider is in use, watching Sport 1080i.")])
        self.assertEqual(xbmc.PlayList(xbmc.PLAYLIST_VIDEO).size(), 0, "Kodi would try each channel after it")

    def test_a_favourite_from_the_menu(self):
        del xbmc.builtins[:]
        self.run_route("route=live_favourite&id=2&value=0")
        self.assertEqual([(r.method, r.path) for r in self.server.requests], [("DELETE", "/s3cret/api/live/channels/2/favorite")])
        self.assertIn("Container.Refresh", xbmc.builtins)

    def test_a_server_without_live_tv_has_none(self):
        class Refusing:
            def get(self, path):
                raise livetv.api.ApiError(404, "not_found", "no route")

        self.assertEqual(livetv.summary(Refusing()), {})
        self.assertFalse(livetv.available({}))


class LiveScreenTest(unittest.TestCase):
    """Live TV in pley's window."""

    def setUp(self):
        clock_patch = mock.patch("time.time", return_value=NOW)
        clock_patch.start()
        self.addCleanup(clock_patch.stop)
        xbmc.PlayList.entries.clear()
        del xbmc.played[:]
        self.win = test_window.FakeWindow()
        self.win.me = {"id": 3, "is_admin": False}
        self.win.live = {"available": True}
        self.channels = [dict(c) for c in CHANNELS]
        self.favourites = []
        self.guides = []

        class Server:
            def get(inner, path, **query):
                if path == "/api/live":
                    return {"available": True, "recent": [4]}
                if path == "/api/live/channels":
                    return self.channels
                raise AssertionError(path)

            def request(inner, method, path, query=None):
                self.guides.append(query)
                body = {"from": 0, "to": 0, "programs": {"1": [NEWS["now"], NEWS["next"]]}}
                return livetv.api.Response(200, {}, json.dumps(body).encode("utf-8"))

            def put(inner, path, body=None):
                self.favourites.append(("PUT", path))

            def delete(inner, path):
                self.favourites.append(("DELETE", path))

        self.win.session.client = Server()
        self.screen = window.Live(self.win)
        self.win.screen = self.screen
        self.addCleanup(lambda: self.screen.timer and self.screen.timer.cancel())
        self.screen.load()

    def rows(self):
        return self.win.made[L.LIVE_LIST].items

    def test_the_channels_with_whats_on(self):
        self.screen.show(False)
        rows = self.rows()
        self.assertEqual([r.label for r in rows], ["101 Pley News HD", "102 Sport 1080i", "104 Radio Jazz", "105 Weather Now"])
        news = rows[0]
        self.assertEqual(news.label2, "Morning Briefing: Part 7")
        self.assertEqual(news.properties["when"].split(" · ")[1], "45 min left")
        self.assertEqual(news.properties["airs"], "Today · %s – %s · S1 · E7 · Live" % (clock(NOW - 900), clock(NOW + 2700)))
        self.assertEqual(news.properties["next"], "%s  World at One" % clock(NOW + 2700))
        self.assertTrue(news.properties["picture"].endswith("/image?w=640"))
        # How far into it: the progress bar, as a tile's.
        self.assertEqual(news.video.values["setResumePoint"][1], 3600.0)
        self.assertEqual((rows[1].properties["favorite"], rows[1].properties["short"]), ("1", "102"))
        self.assertEqual((rows[2].label2, rows[2].properties["when"]), ("No guide", ""))
        self.assertEqual(self.win.focused, L.LIVE_LIST)
        # Tabs: Favourites with a favourite, Recently watched with one watched, Groups with two.
        self.assertTrue(self.win.made[L.LIVE_ALL + 100].visible)
        for tab in (L.LIVE_FAVOURITES, L.LIVE_RECENT, L.LIVE_GROUP):
            self.assertTrue(self.win.made[tab].visible, tab)

    def test_tabs_and_their_memory(self):
        self.screen.show(False)
        stored = {}
        orig = (window.kodi.remember, window.kodi.setting)
        window.kodi.remember = lambda key, value: stored.update({key: value})
        window.kodi.setting = lambda name: "srv" if name == "server_id" else ""
        self.addCleanup(lambda: (setattr(window.kodi, "remember", orig[0]), setattr(window.kodi, "setting", orig[1])))
        with test_window.Dialogs(picks=[0]) as asked:
            self.screen.click(L.LIVE_GROUP)
        self.assertEqual(asked.shown[0][0], ["News (2)", "Sport (1)", "Radio (1)"])
        self.assertEqual([r.label for r in self.rows()], ["101 Pley News HD", "105 Weather Now"])
        self.assertEqual(self.win.made[L.LIVE_GROUP + 100].label, "News")
        self.assertEqual(stored, {"live_tab/srv/3": "group:News"})
        self.screen.click(L.LIVE_RECENT)
        self.assertEqual([r.label for r in self.rows()], ["104 Radio Jazz"])

    def test_watching_queues_the_tabs_channels(self):
        self.screen.show(False)
        self.win.made[L.LIVE_LIST].selected = 1
        self.screen.click(L.LIVE_LIST)
        queue, listitem, start = xbmc.played[-1]
        self.assertEqual(start, 1)
        self.assertEqual([url for url, _ in xbmc.PlayList.entries[xbmc.PLAYLIST_VIDEO]][1], "plugin://plugin.video.pley/?route=live_play&id=2")

    def test_a_favourite_from_the_menu(self):
        self.screen.show(False)
        self.win.made[L.LIVE_LIST].selected = 0
        with test_window.Dialogs(picks=[1]) as asked:
            self.screen.menu(L.LIVE_LIST)
        self.assertEqual(asked.shown[0][0], ["Watch", "Add to favourites", "Schedule"])
        self.assertEqual(self.favourites, [("PUT", "/api/live/channels/1/favorite")])
        self.assertEqual(self.rows()[0].properties["favorite"], "1")
        # The last favourite removed on Favourites: All again, and no Favourites tab.
        self.screen.switch("favorites")
        for at in (0, 0):
            self.win.made[L.LIVE_LIST].selected = at
            with test_window.Dialogs(picks=[1]):
                self.screen.menu(L.LIVE_LIST)
        self.assertEqual(self.screen.tab, "all")
        self.assertFalse(self.win.made[L.LIVE_FAVOURITES].visible)
        self.assertEqual(self.win.focused, L.LIVE_LIST)

    def test_a_channels_schedule(self):
        self.screen.show(False)
        # Info: the schedule; a programme still to come says what it is,
        # and back from it the schedule is as it was, not asked for again.
        with test_window.Dialogs(picks=[1, -1]) as asked:
            self.screen.info(L.LIVE_LIST)
        labels, preselect = asked.shown[0]
        self.assertEqual(labels, ["On now  ·  Morning Briefing: Part 7", "Today %s  ·  World at One" % clock(NOW + 2700), "Later"])
        self.assertEqual(asked.shown[1][1], 1)
        self.assertEqual(asked.asked[0][0], "World at One")
        self.assertIn("101 Pley News HD", asked.asked[0][1])
        self.assertEqual(self.guides, [{"from": NOW, "to": NOW + 6 * HOUR}])
        # What's on watches the channel.
        with test_window.Dialogs(picks=[0]):
            self.screen.info(L.LIVE_LIST)
        self.assertEqual(xbmc.PlayList.entries[xbmc.PLAYLIST_VIDEO][xbmc.played[-1][2]][0], "plugin://plugin.video.pley/?route=live_play&id=1")

    def test_number_keys_go_to_a_channel(self):
        self.screen.show(False)
        self.win.post = lambda work: work()
        self.screen.switch("favorites")
        for digit in (1, 0, 4):
            self.screen.number(digit)
        self.screen.typed_timer.cancel()
        self.assertEqual(self.win.toasts[-1], "104")
        self.screen.go_to_typed()
        # Not among the favourites: on All.
        self.assertEqual(self.screen.tab, "all")
        self.assertEqual(self.win.made[L.LIVE_LIST].selected, 2)
        self.screen.number(9)
        self.screen.typed_timer.cancel()
        self.screen.go_to_typed()
        self.assertEqual(self.win.toasts[-1], "No channel 9")

    def test_without_channels_it_says_so(self):
        self.channels = []
        self.screen.load()
        notice = self.screen.instead()
        self.assertEqual((notice.title, notice.body, notice.rail), ("No channels yet", window.T("no_channels_body"), "live"))
        self.win.me["is_admin"] = True
        self.assertIn("Settings → Live TV", self.screen.instead().body)

    def test_the_rail_has_live_tv_where_there_are_channels(self):
        self.win.libraries = [{"id": 1, "name": "Movies", "kind": "movies"}]
        self.win.setup_rail()
        self.assertTrue(self.win.made[L.rail_group(L.RAIL_LIVE)].visible)
        self.assertEqual(window.rail_entries(1, True), [L.RAIL_SEARCH, L.RAIL_HOME, L.RAIL_LIVE, L.RAIL_LIBRARIES[0], L.RAIL_PLAYLISTS, L.RAIL_COLLECTIONS])
        self.assertEqual(self.win.rail_entry("live"), L.RAIL_LIVE)
        self.win.live = {"available": False, "allowed": True}
        self.win.setup_rail()
        self.assertFalse(self.win.made[L.rail_group(L.RAIL_LIVE)].visible)
        self.assertEqual(self.win.rail_entry("live"), L.RAIL_HOME)


if __name__ == "__main__":
    unittest.main()
