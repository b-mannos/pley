package app.pley.android.screens.admin.livetv

import app.pley.android.model.PleyJson
import app.pley.android.model.admin.LiveSourceKind
import app.pley.android.model.admin.LiveTvAccess
import app.pley.android.network.ApiException
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.AdminLoad
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.testing.MainDispatcherRule
import app.pley.android.testing.data.FakeServerAdmin
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Live TV's pages: what each reads, the changes they send and their
 * bodies, and what the server's refusals leave shown.
 */
@OptIn(ExperimentalCoroutinesApi::class)
class LiveTvAdminViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private fun source(id: Long, refreshing: Boolean = false, enabled: Boolean = true) = """
        {"id":$id,"name":"Provider $id","kind":"xtream","url":"http://provider.example:8080","username":"ann",
         "password_set":true,"guide_url":"","user_agent":"VLC","max_connections":2,"enabled":$enabled,
         "channels_at":1700000000,"channel_count":10,"hidden_count":1,"guided_count":5,"connections_in_use":0,
         "refreshing":$refreshing}
    """.trimIndent()

    private fun body(call: FakeServerAdmin.Call): JsonObject = call.body!!.jsonObject

    private fun JsonObject.text(key: String): String? = (get(key) as? kotlinx.serialization.json.JsonPrimitive)?.content

    // ---------------------------------------------------------------- the first page

    @Test
    fun theFirstPageReadsTheSourcesAndTheConnectionsWhileItShows() = runTest {
        val server = FakeServerAdmin()
        var reading = true
        server.answer("GET /api/live/sources") { PleyJson.parseToJsonElement("[${source(1, refreshing = reading)}]") }
        server.answer("GET /api/live/tuners", """[{"channel_id":9,"channel_name":"News","source_name":"Provider 1","started_at":1,"kbps":800,"viewers":[]}]""")
        val page = LiveTvAdminViewModel(server)
        assertEquals(1, page.sources.state.value.valueOrNull?.size)
        assertNull(page.tuners.value)

        val watching = backgroundScope.launch { page.watch() }
        runCurrent()
        assertEquals(listOf(9L), page.tuners.value?.map { it.channelId })
        // A source being read is read again until it is done.
        advanceTimeBy(LiveTvAdminViewModel.READING_EVERY + 1)
        assertEquals(2, server.callsTo("GET /api/live/sources").size)
        reading = false
        advanceTimeBy(LiveTvAdminViewModel.READING_EVERY)
        assertEquals(3, server.callsTo("GET /api/live/sources").size)
        advanceTimeBy(LiveTvAdminViewModel.READING_EVERY * 3)
        assertEquals(3, server.callsTo("GET /api/live/sources").size)
        assertTrue(server.callsTo("GET /api/live/tuners").size >= 3)

        // Off screen, nothing is read.
        watching.cancel()
        val tunerReads = server.callsTo("GET /api/live/tuners").size
        advanceTimeBy(LiveTvAdminViewModel.TUNERS_EVERY * 3)
        assertEquals(tunerReads, server.callsTo("GET /api/live/tuners").size)
    }

    @Test
    fun aConnectionIsStoppedForEveryone() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/sources", "[${source(1)}]")
        var tuners = """[{"channel_id":9,"channel_name":"News"}]"""
        server.answer("GET /api/live/tuners") { PleyJson.parseToJsonElement(tuners) }
        server.answer("DELETE /api/live/tuners/9") {
            tuners = "[]"
            null
        }
        val page = LiveTvAdminViewModel(server)
        val said = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.confirmations.collect { said += it } }
        val tuner = PleyJson.decodeFromString(app.pley.android.model.admin.TunerStatus.serializer(), """{"channel_id":9,"channel_name":"News"}""")
        page.stop(tuner).join()
        assertEquals(1, server.callsTo("DELETE /api/live/tuners/9").size)
        assertEquals(emptyList<Any>(), page.tuners.value)
        assertEquals(listOf(AdminDone(LiveTvAdminViewModel.STOPPED, listOf("News"))), said)
    }

    // ---------------------------------------------------------------- a source

    @Test
    fun addingASourceTestsItThenSendsItsDetailsTogether() = runTest {
        val server = FakeServerAdmin()
        server.answer("POST /api/live/sources/test", """{"channels":1200,"groups":40,"guide_url":"http://provider.example/xmltv.php","account":{"status":"Active","max_connections":3}}""")
        server.answer("POST /api/live/sources") { PleyJson.parseToJsonElement(source(7)) }
        val page = LiveSourceViewModel(server, null)
        assertTrue(page.adding)
        assertEquals(AdminLoad.Ready(null), page.source.state.value)
        assertEquals(SourceDraft(), page.draft.value)
        val finished = mutableListOf<Unit>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.finished.collect { finished += it } }

        // Nothing goes before it has an address, and an Xtream account's user name.
        page.edit { it.copy(kind = LiveSourceKind.Xtream, url = "http://provider.example:8080") }
        page.test()
        page.save()
        assertTrue(server.calls.isEmpty())

        page.edit { it.copy(username = "ann", password = "secret") }
        page.test()
        val tested = body(server.callsTo("POST /api/live/sources/test").single())
        assertEquals("provider.example", tested.text("name"))
        assertEquals("secret", tested.text("password"))
        assertFalse("source_id" in tested)
        // The account says how many connections it has.
        assertEquals(3, page.draft.value?.maxConnections)
        val found = page.testing.value as SourceTesting.Found
        assertEquals(page.draft.value, found.draft)
        assertEquals(1200, found.result.channels)

        page.save()
        val sent = body(server.callsTo("POST /api/live/sources").single())
        assertEquals("xtream", sent.text("kind"))
        assertEquals("ann", sent.text("username"))
        assertEquals("secret", sent.text("password"))
        assertEquals("3", sent.text("max_connections"))
        assertEquals("true", sent.text("enabled"))
        assertEquals(1, finished.size)
    }

    @Test
    fun aFailedTestSaysWhyForTheDetailsTested() = runTest {
        val server = FakeServerAdmin()
        val refused = ApiException(400, "bad_request", "the playlist answered 404")
        server.fail("POST /api/live/sources/test", refused)
        val page = LiveSourceViewModel(server, null)
        page.edit { it.copy(url = "http://tuner.local/lineup.m3u") }
        page.test()
        assertEquals(SourceTesting.Failed(page.draft.value!!, refused), page.testing.value)
    }

    @Test
    fun aSavedSourceKeepsItsPasswordAndLendsItToATest() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/sources", "[${source(3)},${source(4)}]")
        server.answer("POST /api/live/sources/test", """{"channels":1,"groups":1}""")
        server.answer("PUT /api/live/sources/4") { sent ->
            PleyJson.parseToJsonElement(source(4).replace("\"Provider 4\"", (sent as JsonObject)["name"].toString()))
        }
        val page = LiveSourceViewModel(server, 4)
        val draft = page.draft.value!!
        assertEquals("Provider 4", draft.name)
        assertEquals("", draft.password)
        assertEquals("VLC", draft.userAgent)
        assertFalse(page.changed(draft))

        page.edit { it.copy(name = "Renamed") }
        assertTrue(page.changed(page.draft.value!!))
        page.test()
        val tested = body(server.callsTo("POST /api/live/sources/test").single())
        assertEquals(4L, tested["source_id"]?.jsonPrimitive?.long)
        assertFalse("password" in tested)

        val said = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.confirmations.collect { said += it } }
        page.save()
        val sent = body(server.callsTo("PUT /api/live/sources/4").single())
        assertEquals("Renamed", sent.text("name"))
        assertFalse("password" in sent)
        assertEquals("true", sent.text("enabled"))
        assertEquals("Renamed", page.source.state.value.valueOrNull?.name)
        assertFalse(page.changed(page.draft.value!!))
        assertEquals(listOf(AdminDone(LiveSourceViewModel.SAVED)), said)
    }

    @Test
    fun aRefusedSaveKeepsWhatWasTyped() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/sources", "[${source(4)}]")
        val refused = ApiException(400, "bad_request", "max_connections must be between 0 and 100")
        server.fail("PUT /api/live/sources/4", refused)
        val page = LiveSourceViewModel(server, 4)
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.errors.collect { errors += it } }
        page.edit { it.copy(guideUrl = "http://guide.example/xmltv.xml") }
        page.save()
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals("http://guide.example/xmltv.xml", page.draft.value?.guideUrl)
        assertEquals(0, page.busy.value)
    }

    @Test
    fun aSourceIsTurnedOffReadAgainAndRemoved() = runTest {
        val server = FakeServerAdmin()
        var enabled = true
        server.answer("GET /api/live/sources") { PleyJson.parseToJsonElement("[${source(4, enabled = enabled)}]") }
        server.answer("PUT /api/live/sources/4") { sent ->
            enabled = (sent as JsonObject)["enabled"]!!.jsonPrimitive.content.toBoolean()
            PleyJson.parseToJsonElement(source(4, enabled = enabled))
        }
        server.answer("POST /api/live/sources/4/refresh", null)
        server.answer("DELETE /api/live/sources/4", null)
        val page = LiveSourceViewModel(server, 4)
        val said = mutableListOf<AdminDone>()
        val finished = mutableListOf<Unit>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.confirmations.collect { said += it } }
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.finished.collect { finished += it } }

        // What is typed and not saved stays typed; the switch sends the saved details.
        page.edit { it.copy(name = "Typed") }
        page.setEnabled(false)
        val off = body(server.callsTo("PUT /api/live/sources/4").single())
        assertEquals("false", off.text("enabled"))
        assertEquals("Provider 4", off.text("name"))
        assertFalse("password" in off)
        assertEquals(false, page.source.state.value.valueOrNull?.enabled)
        assertEquals("Typed", page.draft.value?.name)

        page.refresh()
        assertEquals(1, server.callsTo("POST /api/live/sources/4/refresh").size)

        page.remove()
        assertEquals(1, server.callsTo("DELETE /api/live/sources/4").size)
        assertEquals(listOf(AdminDone(LiveSourceViewModel.REMOVED, listOf("Provider 4"))), said)
        assertEquals(1, finished.size)
    }

    @Test
    fun aSourceGoneFromTheServerReadsAsNone() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/sources", "[${source(3)}]")
        val page = LiveSourceViewModel(server, 9)
        assertEquals(AdminLoad.Ready(null), page.source.state.value)
        assertNull(page.draft.value)
    }

    // ---------------------------------------------------------------- options

    private val settings = """{"access":"everyone","age_limited":false,"deinterlace":true,"max_height":720,"timeshift_minutes":9,
        "refresh_hours":24,"guide_days":7,"hide_offline":true,"allowed_users":[2],"denied_users":[]}"""

    private fun echo(server: FakeServerAdmin) = server.answer("PUT /api/live/settings") { it }

    @Test
    fun anOptionIsSentWithEveryOtherSettingAsRead() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/settings", settings)
        server.answer("GET /api/users", """[{"id":1,"name":"Admin","is_admin":true},{"id":2,"name":"Sam"},{"id":3,"name":"Kim","home":false}]""")
        echo(server)
        val page = LiveTvOptionsViewModel(server)
        assertEquals(listOf(2L, 3L), page.people.state.value.valueOrNull?.map { it.id })

        page.setAccess(LiveTvAccess.Home)
        val sent = body(server.callsTo("PUT /api/live/settings").single())
        assertEquals("home", sent.text("access"))
        // A setting this app doesn't show goes back as it was, not to its default.
        assertEquals("9", sent.text("timeshift_minutes"))
        assertEquals("720", sent.text("max_height"))
        assertEquals(LiveTvAccess.Home, page.options.state.value.valueOrNull?.settings?.access)

        // As many lines as the channel has is an explicit null.
        page.setMaxHeight(null)
        val asSent = body(server.callsTo("PUT /api/live/settings")[1])
        assertEquals(JsonNull, asSent["max_height"])
        assertEquals("home", asSent.text("access"))
        assertNull(page.options.state.value.valueOrNull?.settings?.maxHeight)

        page.setRefreshHours(6)
        page.setGuideDays(14)
        page.setDeinterlace(false)
        page.setHideOffline(false)
        page.setAgeLimited(true)
        val last = body(server.callsTo("PUT /api/live/settings").last())
        assertEquals("6", last.text("refresh_hours"))
        assertEquals("14", last.text("guide_days"))
        assertEquals("false", last.text("deinterlace"))
        assertEquals("false", last.text("hide_offline"))
        assertEquals("true", last.text("age_limited"))
    }

    @Test
    fun aPersonWatchesAsTheRuleSaysAlwaysOrNever() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/settings", settings)
        server.answer("GET /api/users", "[]")
        echo(server)
        val page = LiveTvOptionsViewModel(server)
        fun lists(): Pair<JsonElement?, JsonElement?> = body(server.calls.last()).let { it["allowed_users"] to it["denied_users"] }
        assertEquals(PersonAccess.Always, personAccess(page.options.state.value.valueOrNull!!.settings, 2))

        page.setPerson(2, PersonAccess.Never)
        assertEquals(JsonArray(emptyList()) to PleyJson.parseToJsonElement("[2]"), lists())
        page.setPerson(5, PersonAccess.Always)
        assertEquals(PleyJson.parseToJsonElement("[5]") to PleyJson.parseToJsonElement("[2]"), lists())
        page.setPerson(2, PersonAccess.AsOpenTo)
        assertEquals(PleyJson.parseToJsonElement("[5]") to JsonArray(emptyList()), lists())
        assertEquals(PersonAccess.AsOpenTo, personAccess(page.options.state.value.valueOrNull!!.settings, 2))
    }

    @Test
    fun aRefusedOptionShowsWhatTheServerHas() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/settings", settings)
        server.answer("GET /api/users", "[]")
        val refused = ApiException(400, "bad_request", "guide_days must be between 1 and 14")
        server.fail("PUT /api/live/settings", refused)
        val page = LiveTvOptionsViewModel(server)
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.errors.collect { errors += it } }
        page.setGuideDays(30)
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals(2, server.callsTo("GET /api/live/settings").size)
        assertEquals(7, page.options.state.value.valueOrNull?.settings?.guideDays)
    }

    // ---------------------------------------------------------------- groups

    @Test
    fun groupsAndRegionsAreHiddenAndShownAtOnce() = runTest {
        val server = FakeServerAdmin()
        server.answer(
            "GET /api/live/lineup/groups",
            """[{"name":"UK| News","region":"UK","label":"News","count":10,"hidden":0},
               {"name":"UK| Films","region":"UK","label":"Films","count":8,"hidden":3},
               {"name":"","region":null,"label":"","count":2,"hidden":0}]""",
        )
        server.answer("PATCH /api/live/lineup", null)
        val page = LineupGroupsViewModel(server)
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.sections.collect {} }
        val said = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.confirmations.collect { said += it } }
        val sections = page.sections.first { it != null }!!
        assertEquals(listOf("UK", null), sections.map { it.region })

        page.setHidden(sections[0], true)
        val region = body(server.callsTo("PATCH /api/live/lineup").single())
        assertEquals(PleyJson.parseToJsonElement("""["UK| News","UK| Films"]"""), region["groups"])
        assertEquals("true", region.text("hidden"))
        assertEquals(JsonArray(emptyList()), region["ids"])

        page.setHidden(sections[1].groups.single(), false)
        assertEquals(PleyJson.parseToJsonElement("""[""]"""), body(server.callsTo("PATCH /api/live/lineup")[1])["groups"])
        assertEquals(
            listOf(AdminDone(LineupGroupsViewModel.REGION_HIDDEN, listOf("UK")), AdminDone(LineupGroupsViewModel.UNNAMED_SHOWN)),
            said,
        )

        page.toggle("UK")
        assertEquals(setOf("UK"), page.open.value)
        page.find("films")
        page.show(GroupsShown.Shown)
        assertEquals(listOf("UK| Films"), page.sections.first { it?.size == 1 }!!.single().groups.map { it.name })
    }

    // ---------------------------------------------------------------- the lineup

    private fun lineup(total: Int, hidden: Int = 0) = PleyJson.parseToJsonElement(
        """{"total":$total,"hidden":$hidden,"offset":0,"channels":[""" +
            (1..minOf(total, LINEUP_PAGE)).joinToString(",") { """{"id":$it,"name":"Channel $it","hidden":false}""" } + "]}",
    )

    @Test
    fun theLineupIsReadAPageAtATimeAsItScrolls() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/lineup/page") { lineup(250) }
        server.answer("GET /api/live/sources", "[]")
        server.answer("GET /api/live/lineup/groups", "[]")
        val page = LineupViewModel(server, "http://nas:7539")
        // The counts, and the first page of the list.
        assertEquals(2, server.callsTo("GET /api/live/lineup/page").size)
        assertEquals(250, page.list.value.total)
        assertEquals(1L, page.list.value.at(0)?.id)
        assertNull(page.list.value.at(100))

        page.visible(0, 12)
        assertEquals(2, server.callsTo("GET /api/live/lineup/page").size)
        page.visible(90, 102)
        assertEquals(3, server.callsTo("GET /api/live/lineup/page").size)
        assertEquals(1L, page.list.value.at(100)?.id)
        page.visible(240, 249)
        assertEquals(setOf(0, 1, 2), page.list.value.pages.keys)

        // Another filter lists from its first page, and chooses nothing.
        page.choose(5)
        page.setState(app.pley.android.model.admin.LineupState.Hidden)
        assertEquals(app.pley.android.model.admin.LineupState.Hidden, page.list.value.filter.state)
        assertEquals(setOf(0), page.list.value.pages.keys)
        assertEquals(LineupChoice(), page.choice.value)
    }

    @Test
    fun channelsAreHiddenOneAtATimeChosenOrAllThatTheFilterKeeps() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/lineup/page") { lineup(150) }
        server.answer("GET /api/live/sources", "[]")
        server.answer("GET /api/live/lineup/groups", "[]")
        server.answer("PATCH /api/live/lineup", null)
        val page = LineupViewModel(server, "")
        val said = mutableListOf<AdminDone>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.confirmations.collect { said += it } }

        page.setHidden(page.list.value.at(0)!!, true)
        val one = body(server.callsTo("PATCH /api/live/lineup").single())
        assertEquals(PleyJson.parseToJsonElement("[1]"), one["ids"])
        assertEquals("true", one.text("hidden"))
        assertFalse("filter" in one)

        page.choose(2)
        page.choose(3)
        page.choose(2)
        assertEquals(LineupChoice(ids = setOf(3)), page.choice.value)
        page.choose(4)
        page.setChosenHidden(false)
        val chosen = body(server.callsTo("PATCH /api/live/lineup")[1])
        assertEquals(PleyJson.parseToJsonElement("[3,4]"), chosen["ids"])
        assertEquals("false", chosen.text("hidden"))
        assertEquals(LineupChoice(), page.choice.value)

        page.find("news")
        advanceTimeBy(LineupViewModel.SEARCH_WAIT + 1)
        assertEquals("news", page.list.value.filter.q)
        page.chooseAll()
        assertEquals(150, page.choice.value.count(150))
        page.setChosenHidden(true)
        val all = body(server.callsTo("PATCH /api/live/lineup")[2])
        assertEquals(JsonArray(emptyList()), all["ids"])
        assertEquals("news", (all["filter"] as JsonObject).text("q"))
        assertEquals("all", (all["filter"] as JsonObject).text("state"))

        assertEquals(
            listOf(AdminDone(LineupViewModel.SHOWN, listOf(2)), AdminDone(LineupViewModel.HIDDEN, listOf(150))),
            said,
        )
    }

    @Test
    fun aChannelsNameNumberAndGuideAreSavedTogether() = runTest {
        val server = FakeServerAdmin()
        server.answer("GET /api/live/lineup/page") { lineup(3) }
        server.answer("GET /api/live/sources", "[]")
        server.answer("GET /api/live/lineup/groups", "[]")
        server.answer("PATCH /api/live/lineup", null)
        val page = LineupViewModel(server, "")
        val channel = page.list.value.at(1)!!
        page.edit(channel)
        assertEquals(channel, page.editing.value)
        page.save(channel, " News ", "", "news.uk")
        val sent = body(server.callsTo("PATCH /api/live/lineup").single())
        assertEquals(PleyJson.parseToJsonElement("[2]"), sent["ids"])
        assertEquals("News", sent.text("name"))
        assertEquals("", sent.text("number"))
        assertEquals("news.uk", sent.text("guide_id"))
        assertFalse("hidden" in sent)
        assertNull(page.editing.value)
    }

    @Test
    fun aFirstPageThatCantBeReadIsThePagesErrorUntilRetried() = runTest {
        val server = FakeServerAdmin()
        val down = ApiException(503, "unavailable", "busy")
        server.fail("GET /api/live/lineup/page", down)
        server.answer("GET /api/live/sources", "[]")
        server.answer("GET /api/live/lineup/groups", "[]")
        val page = LineupViewModel(server, "")
        assertEquals(down, page.list.value.error)
        server.answer("GET /api/live/lineup/page") { lineup(1) }
        page.retry()
        assertNull(page.list.value.error)
        assertEquals(1, page.list.value.total)
    }
}
