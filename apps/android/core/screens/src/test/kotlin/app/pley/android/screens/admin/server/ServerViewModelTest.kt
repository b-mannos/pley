package app.pley.android.screens.admin.server

import app.pley.android.model.PleyJson
import app.pley.android.model.admin.DatabaseBackup
import app.pley.android.model.admin.LibraryWorkMode
import app.pley.android.model.admin.ServerSettings
import app.pley.android.model.admin.ServiceTestResult
import app.pley.android.model.admin.SignInAccount
import app.pley.android.model.admin.TestedService
import app.pley.android.model.admin.TvMetadataSource
import app.pley.android.network.ApiException
import app.pley.android.screens.admin.AdminLoad
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.testing.MainDispatcherRule
import app.pley.android.testing.data.FakeServerAdmin
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Settings → Server on a phone: what it reads, the partial bodies of
 * PUT /api/settings each value sends, the server's refusals, tests of the
 * saved keys and backing up.
 */
class ServerViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val saved = """
        {"server_name":"Den","language":"","tmdb_api_key":"","metadata_language":"en-US","tv_metadata":"auto",
         "tvdb_api_key":"k","tvdb_pin":"","music_metadata":true,"read_nfo":true,"scan_interval_minutes":360,
         "watch_libraries":true,"detect_markers":true,"seek_previews":true,"previews_in_background":true,
         "library_work":"auto","library_work_from":1,"library_work_to":7,"library_work_max_cpu":50,
         "third_party_apps":false,"require_two_step":false,"opensubtitles_api_key":"","opensubtitles_username":"",
         "opensubtitles_password_set":false}
    """

    /** A server whose PUT /api/settings merges what it is sent into what it has, as the real one does. */
    private fun server(settings: String = saved): FakeServerAdmin {
        val server = FakeServerAdmin()
        var now = PleyJson.parseToJsonElement(settings).jsonObject
        server.answer("GET /api/settings") { now }
        server.answer("PUT /api/settings") { body ->
            now = JsonObject(now + body!!.jsonObject)
            now
        }
        server.answer("GET /api/system", """{"secret_key_ok":true,"transcoder":{"available":true,"encoders":["libx264"],"chromaprint":true}}""")
        server.answer("GET /api/auth/me", """{"id":1,"username":"ann","name":"Ann","two_step":true}""")
        server.answer(
            "GET /api/users",
            """[{"id":1,"has_password":true,"two_step":true},{"id":2,"has_password":true,"two_step":false},
                {"id":3,"has_password":true,"two_step":false,"disabled":true},{"id":4,"has_password":false}]""",
        )
        server.answer("GET /api/system/library-work", """{"mode":"auto","learned":false,"now":3,"hours":[]}""")
        server.answer("GET /api/system/backups", """{"health":{"ok":true},"backups":[]}""")
        return server
    }

    private fun sent(server: FakeServerAdmin): List<JsonElement?> = server.callsTo("PUT /api/settings").map { it.body }

    private fun json(text: String): JsonElement = PleyJson.parseToJsonElement(text)

    private fun ServerViewModel.now(): ServerSettings = settings.state.value.valueOrNull!!

    @Test
    fun thePageReadsTheSettingsTheSystemWhoManagesTheAccountsTheWeekAndTheBackups() = runTest {
        val server = server()
        val page = ServerViewModel(server)
        assertEquals("Den", page.now().serverName)
        assertEquals(true, page.system.state.value.valueOrNull?.transcoder?.chromaprint)
        assertEquals(true, page.me.state.value.valueOrNull?.twoStep)
        assertEquals(1, withoutTwoStep(page.accounts.state.value.valueOrNull!!))
        assertEquals(LibraryWorkMode.Auto, page.week.state.value.valueOrNull?.mode)
        assertEquals(true, page.backups.state.value.valueOrNull?.health?.ok)
        for (route in listOf("GET /api/settings", "GET /api/system", "GET /api/auth/me", "GET /api/users", "GET /api/system/library-work", "GET /api/system/backups")) {
            assertEquals(route, 1, server.callsTo(route).size)
        }
    }

    @Test
    fun eachValueIsSentAloneAndTheAnswerShown() = runTest {
        val server = server()
        val page = ServerViewModel(server)
        page.rename("Living room").join()
        page.setLanguage("fr").join()
        page.setMusicMetadata(false).join()
        page.setMetadataLanguage("fr-FR").join()
        page.setScanInterval(0).join()
        page.setThirdPartyApps(true).join()
        assertEquals(
            listOf(
                json("""{"server_name":"Living room"}"""),
                json("""{"language":"fr"}"""),
                json("""{"music_metadata":false}"""),
                json("""{"metadata_language":"fr-FR"}"""),
                json("""{"scan_interval_minutes":0}"""),
                json("""{"third_party_apps":true}"""),
            ),
            sent(server),
        )
        val now = page.now()
        assertEquals("Living room", now.serverName)
        assertEquals("fr", now.language)
        assertFalse(now.musicMetadata)
        assertEquals(0, now.scanIntervalMinutes)
        assertTrue(now.thirdPartyApps)
    }

    @Test
    fun aSourceWhoseKeyIsRemovedGivesWayToAutomatic() = runTest {
        val server = server(saved.replace("\"tv_metadata\":\"auto\"", "\"tv_metadata\":\"tvdb\""))
        val page = ServerViewModel(server)
        page.setTmdbKey("abc").join()
        page.setTvdbKey("").join()
        assertEquals(
            listOf(json("""{"tmdb_api_key":"abc"}"""), json("""{"tvdb_api_key":"","tv_metadata":"auto"}""")),
            sent(server),
        )
        assertEquals(TvMetadataSource.Auto, page.now().tvMetadata)
        assertEquals(TvMetadataSource.Tmdb, automaticTvSource(page.now()))
        assertEquals(TvMetadataSource.Tvmaze, automaticTvSource(page.now().copy(tmdbApiKey = " ")))
    }

    @Test
    fun anOpenSubtitlesPasswordLeftEmptyKeepsTheSavedOne() = runTest {
        val server = server()
        val page = ServerViewModel(server)
        assertNull(page.setOpenSubtitlesPassword(""))
        page.setOpenSubtitlesUsername("ann").join()
        page.setOpenSubtitlesPassword("s3cret")!!.join()
        assertEquals(
            listOf(json("""{"opensubtitles_username":"ann"}"""), json("""{"opensubtitles_password":"s3cret"}""")),
            sent(server),
        )
    }

    @Test
    fun libraryWorkChangesReadTheWeekAgain() = runTest {
        val server = server()
        val page = ServerViewModel(server)
        page.setLibraryWork(LibraryWorkMode.Hours).join()
        page.setLibraryWorkFrom(22).join()
        page.setLibraryWorkTo(6).join()
        page.setLibraryWorkMaxCpu(100).join()
        page.setDetectMarkers(false).join()
        assertEquals(
            listOf(
                json("""{"library_work":"hours"}"""),
                json("""{"library_work_from":22}"""),
                json("""{"library_work_to":6}"""),
                json("""{"library_work_max_cpu":100}"""),
                json("""{"detect_markers":false}"""),
            ),
            sent(server),
        )
        assertEquals(6, server.callsTo("GET /api/system/library-work").size)
        val now = page.now()
        assertEquals(LibraryWorkMode.Hours, now.libraryWork)
        assertEquals(22, now.libraryWorkFrom)
        assertTrue(now.libraryWorkToDo)
        assertFalse(now.copy(seekPreviews = false).libraryWorkToDo)
        assertEquals(listOf(25, 50, 75, 100), maxCpuChoices(50))
        assertEquals(listOf(10, 25, 50, 75, 100), maxCpuChoices(10))
    }

    @Test
    fun aRefusedChangeIsSaidAndWhatTheServerHasIsShownAgain() = runTest {
        val server = server()
        val page = ServerViewModel(server)
        val refused = ApiException(400, "bad_request", "turn on two-step sign-in for your own account first (Account → Two-step sign-in)")
        server.fail("PUT /api/settings", refused)
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.errors.collect { errors += it } }
        page.setRequireTwoStep(true).join()
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals(json("""{"require_two_step":true}"""), sent(server).single())
        // Shown at once, then what the server has.
        assertEquals(false, page.now().requireTwoStep)
        assertEquals(2, server.callsTo("GET /api/settings").size)
        // The accounts are read again: who was signed out changed, if anyone.
        assertEquals(2, server.callsTo("GET /api/users").size)
    }

    @Test
    fun aTestUsesTheSavedKeyAndIsForgottenWhenTheKeyChanges() = runTest {
        val server = server()
        server.answer("POST /api/settings/test", """{"ok":false,"message":"TheTVDB rejected this key.","ms":120}""")
        val page = ServerViewModel(server)
        page.test(TestedService.Tvdb).join()
        assertEquals(json("""{"service":"tvdb"}"""), server.callsTo("POST /api/settings/test").single().body)
        assertEquals(ServiceTestState.Done(ServiceTestResult(false, "TheTVDB rejected this key.", 120)), page.tests.value[TestedService.Tvdb])
        page.setTvdbPin("1234").join()
        assertNull(page.tests.value[TestedService.Tvdb])

        val down = ApiException(503, "unavailable", "the server is busy")
        server.fail("POST /api/settings/test", down)
        page.test(TestedService.Musicbrainz).join()
        assertEquals(ServiceTestState.Failed(down), page.tests.value[TestedService.Musicbrainz])
    }

    @Test
    fun backingUpSaysHowBigAndReadsTheListAgain() = runTest {
        val server = server()
        server.answer("POST /api/system/backups", """{"name":"pley-1.db","path":"/data/pley-1.db","bytes":4096,"created_at":1700000000}""")
        val page = ServerViewModel(server)
        val made = mutableListOf<DatabaseBackup>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.backedUp.collect { made += it } }
        page.backUp().join()
        assertEquals(listOf(DatabaseBackup("pley-1.db", 4096, 1_700_000_000)), made)
        assertEquals(2, server.callsTo("GET /api/system/backups").size)
    }

    @Test
    fun theWeekIsReadAgainEveryMinuteWhileFollowed() = runTest {
        val server = server()
        val page = ServerViewModel(server)
        val following = backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.followWeek() }
        testScheduler.advanceTimeBy(ServerViewModel.WEEK_EVERY_MS * 2 + 1)
        assertEquals(3, server.callsTo("GET /api/system/library-work").size)
        following.cancel()
        testScheduler.advanceTimeBy(ServerViewModel.WEEK_EVERY_MS * 2)
        assertEquals(3, server.callsTo("GET /api/system/library-work").size)
    }

    @Test
    fun aPageThatFailsToReadSaysSo() = runTest {
        val server = FakeServerAdmin()
        val down = ApiException(500, "internal", "it broke")
        server.fail("GET /api/settings", down)
        val page = ServerViewModel(server)
        assertEquals(AdminLoad.Failed<ServerSettings>(down, null), page.settings.state.value)
    }

    @Test
    fun accountsWithoutTwoStepAreThoseWithAPasswordThatAreNotDisabled() {
        val accounts = listOf(SignInAccount(1, true, false), SignInAccount(2, true, true), SignInAccount(3, false, false), SignInAccount(4, true, false, disabled = true))
        assertEquals(1, withoutTwoStep(accounts))
        assertEquals("en-US", metadataLanguages("en-US").first())
        assertEquals("eo-EO", metadataLanguages("eo-EO").first())
    }
}
