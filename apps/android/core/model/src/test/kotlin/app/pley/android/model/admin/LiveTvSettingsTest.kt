package app.pley.android.model.admin

import app.pley.android.model.PleyJson
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Live TV's administration, read as apps/server/src/api/livetv.rs and livetv/mod.rs write it (docs/API.md, "Live TV"). */
class LiveTvSettingsTest {
    @Test
    fun settingsReadWithTheServersDefaults() {
        val settings = PleyJson.decodeFromString(
            LiveTvSettings.serializer(),
            """{"access":"home","age_limited":true,"deinterlace":false,"max_height":720,"timeshift_minutes":5,
               "refresh_hours":12,"guide_days":3,"hide_offline":false,"allowed_users":[4],"denied_users":[5,6]}""",
        )
        assertEquals(LiveTvAccess.Home, settings.access)
        assertTrue(settings.ageLimited)
        assertFalse(settings.deinterlace)
        assertEquals(720, settings.maxHeight)
        assertEquals(12, settings.refreshHours)
        assertEquals(3, settings.guideDays)
        assertFalse(settings.hideOffline)
        assertEquals(listOf(4L), settings.allowedUsers)
        assertEquals(listOf(5L, 6L), settings.deniedUsers)

        val defaults = PleyJson.decodeFromString(LiveTvSettings.serializer(), """{"access":"friends","max_height":null}""")
        assertEquals(LiveTvAccess.Unknown, defaults.access)
        assertNull(defaults.maxHeight)
        assertTrue(defaults.deinterlace)
        assertEquals(24, defaults.refreshHours)
        assertEquals(7, defaults.guideDays)
        assertTrue(defaults.hideOffline)
    }

    @Test
    fun aSourceReadsWithItsStateAndAccount() {
        val sources = PleyJson.decodeFromString(
            ListSerializer(LiveSource.serializer()),
            """[{"id":3,"uid":"k2x9","name":"Provider","kind":"xtream","url":"http://provider.example:8080","username":"ann",
                "password_set":true,"guide_url":"","user_agent":"","max_connections":2,"enabled":true,"created_at":1700000000,
                "channels_at":1700000100,"channels_error":null,"guide_at":null,"guide_error":"no answer",
                "account":{"status":"Active","expires_at":1800000000,"max_connections":2,"active_connections":1,"trial":false},
                "channel_count":1200,"hidden_count":200,"guided_count":900,"connections_in_use":1,"refreshing":false},
               {"id":4,"name":"Tuner","kind":"m3u","url":"http://tuner.local/lineup.m3u","username":"","password_set":false,
                "guide_url":"","user_agent":"","max_connections":0,"enabled":false,"created_at":1,"channels_at":null,
                "channels_error":"404","guide_at":null,"guide_error":null,"account":null,"channel_count":0,"hidden_count":0,
                "guided_count":0,"connections_in_use":0,"refreshing":true}]""",
        )
        val xtream = sources[0]
        assertEquals(LiveSourceKind.Xtream, xtream.kind)
        assertTrue(xtream.passwordSet)
        assertEquals(1700000100L, xtream.channelsAt)
        assertEquals("no answer", xtream.guideError)
        assertEquals(LiveSourceAccount("Active", 1800000000, 2, 1, false), xtream.account)
        assertEquals(1200, xtream.channelCount)
        assertEquals(200, xtream.hiddenCount)
        assertEquals(900, xtream.guidedCount)
        val m3u = sources[1]
        assertEquals(LiveSourceKind.M3u, m3u.kind)
        assertEquals(0, m3u.maxConnections)
        assertFalse(m3u.enabled)
        assertTrue(m3u.refreshing)
        assertNull(m3u.account)
        assertEquals("404", m3u.channelsError)
    }

    @Test
    fun aSourceIsSentAsTheServerReadsIt() {
        val kept = PleyJson.encodeToJsonElement(
            LiveSourceInput.serializer(),
            LiveSourceInput(name = "Provider", kind = LiveSourceKind.Xtream, url = "http://provider.example", username = "ann", maxConnections = 0),
        ).jsonObject
        assertEquals("xtream", kept["kind"]?.jsonPrimitive?.content)
        assertEquals("0", kept["max_connections"]?.jsonPrimitive?.content)
        assertEquals("true", kept["enabled"]?.jsonPrimitive?.content)
        // No password: the saved one is kept.
        assertFalse("password" in kept)
        val typed = PleyJson.encodeToJsonElement(
            LiveSourceInput.serializer(),
            LiveSourceInput(name = "Provider", kind = LiveSourceKind.M3u, url = "http://a", password = "secret"),
        ).jsonObject
        assertEquals("secret", typed["password"]?.jsonPrimitive?.content)
        assertEquals("", typed["guide_url"]?.jsonPrimitive?.content)
    }

    @Test
    fun aTestSaysWhatTheDetailsGive() {
        val found = PleyJson.decodeFromString(
            LiveSourceTest.serializer(),
            """{"channels":1234,"groups":56,"guide_url":"http://provider.example/xmltv.php","account":null}""",
        )
        assertEquals(LiveSourceTest(1234, 56, "http://provider.example/xmltv.php", null), found)
        assertNull(PleyJson.decodeFromString(LiveSourceTest.serializer(), """{"channels":0,"groups":0,"guide_url":null,"account":null}""").guideUrl)
    }

    @Test
    fun theLineupReadsAPageAndItsGroups() {
        val page = PleyJson.decodeFromString(
            LineupPage.serializer(),
            """{"total":250,"hidden":12,"offset":100,"channels":[{"id":9,"source_id":3,"name":"News HD","number":"101",
                "group":"UK| News","logo":"/api/live/channels/9/logo","guide_id":"news.uk","hidden":true,"source_name":"News",
                "source_number":"5","source_guide_id":"","matched_guide_id":"news.uk","custom_name":"News HD","custom_number":"101",
                "custom_guide_id":null,"has_guide":true,"offline":false,"offline_since":null}]}""",
        )
        assertEquals(250, page.total)
        assertEquals(12, page.hidden)
        assertEquals(100, page.offset)
        val channel = page.channels.single()
        assertEquals(9L, channel.id)
        assertEquals("News", channel.sourceName)
        assertEquals("5", channel.sourceNumber)
        assertEquals("news.uk", channel.matchedGuideId)
        assertEquals("News HD", channel.customName)
        assertNull(channel.customGuideId)
        assertTrue(channel.hidden)
        assertTrue(channel.hasGuide)

        // An AdminGroup is a Group flattened, with its hidden and offline counts.
        val groups = PleyJson.decodeFromString(
            ListSerializer(LineupGroup.serializer()),
            """[{"name":"UK| News","region":"UK","label":"News","count":40,"hidden":40,"offline":2},
               {"name":"","region":null,"label":"","count":3,"hidden":0,"offline":0}]""",
        )
        assertEquals(LineupGroup("UK| News", "UK", "News", 40, 40, 2), groups[0])
        assertNull(groups[1].region)
    }

    @Test
    fun aLineupChangeLeavesOutWhatItDoesntChange() {
        val hide = PleyJson.encodeToJsonElement(LineupPatch.serializer(), LineupPatch(groups = listOf("UK| News"), hidden = true)).jsonObject
        assertEquals(setOf("ids", "groups", "hidden"), hide.keys)
        val filtered = PleyJson.encodeToJsonElement(
            LineupPatch.serializer(),
            LineupPatch(filter = LineupFilter(sourceId = 3, state = LineupState.NoGuide, q = "news"), hidden = false),
        ).jsonObject
        val filter = filtered["filter"] as JsonObject
        assertEquals("3", filter["source_id"]?.jsonPrimitive?.content)
        assertEquals("no-guide", filter["state"]?.jsonPrimitive?.content)
        assertEquals("news", filter["q"]?.jsonPrimitive?.content)
        assertFalse("group" in filter)
        // "" gives back the source's own value, so it is sent.
        val named = PleyJson.encodeToJsonElement(LineupPatch.serializer(), LineupPatch(ids = listOf(9), name = "", number = "7", guideId = "")).jsonObject
        assertEquals("", named["name"]?.jsonPrimitive?.content)
        assertEquals("", named["guide_id"]?.jsonPrimitive?.content)
        assertFalse("hidden" in named)
        assertFalse(named["name"] is JsonNull)
    }

    @Test
    fun aTunerSaysWhoWatchesAndWhatTheTranscoderMakes() {
        val tuners = PleyJson.decodeFromString(
            ListSerializer(TunerStatus.serializer()),
            """[{"channel_id":9,"channel_name":"News HD","source_id":3,"source_name":"Provider","started_at":1700000000,
                "connected":true,"error":null,"bytes_in":1000,"kbps":4200,
                "viewers":[{"session_id":"s1","user_id":2,"username":"ann","kind":"hls","started_at":1700000000}],
                "idle":false,"plan":{"video_codec":"mpeg2video","width":1920,"height":1080,"frame_rate":25.0,"interlaced":true,
                "audio_codec":"mp2","audio_channels":2,"video":"h264_nvenc","audio":"aac"}}]""",
        )
        val tuner = tuners.single()
        assertEquals(9L, tuner.channelId)
        assertEquals(4200L, tuner.kbps)
        assertEquals(listOf("ann"), tuner.viewers.map { it.username })
        assertEquals("h264_nvenc", tuner.plan?.video)
        assertTrue(tuner.plan?.interlaced == true)
    }

    @Test
    fun peopleAreAccountsWithTheirRoleAndHousehold() {
        val people = PleyJson.decodeFromString(
            ListSerializer(LiveTvPerson.serializer()),
            """[{"id":1,"username":"admin","name":"Admin","is_admin":true,"home":true},{"id":2,"username":"sam","name":"Sam","is_admin":false,"home":false}]""",
        )
        assertEquals(listOf(LiveTvPerson(1, "Admin", true, true), LiveTvPerson(2, "Sam", false, false)), people)
    }
}
