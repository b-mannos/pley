package app.pley.android.screens.admin.requests

import app.pley.android.model.ArrService
import app.pley.android.model.PleyJson
import app.pley.android.model.QualityProfile
import app.pley.android.model.admin.ArrOffer
import app.pley.android.model.admin.RadarrAvailability
import app.pley.android.model.admin.RequestsSettings
import app.pley.android.network.ApiException
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.testing.MainDispatcherRule
import app.pley.android.testing.data.FakeServerAdmin
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

/**
 * Settings → Requests on a phone: taking requests and the limits, and a
 * service's page: asking what the saved Radarr offers, Connect with what is
 * typed, saving the address and key together, and each choice alone.
 */
class RequestsAdminViewModelTest {
    @get:Rule val main = MainDispatcherRule()

    private val saved = """
        {"tmdb_api_key":"abc","requests":false,"request_limit_movies":0,"request_limit_seasons":0,
         "radarr_url":"http://nas:7878","radarr_api_key_set":true,"radarr_quality_profile":0,"radarr_root_folder":"",
         "radarr_anime_quality_profile":0,"radarr_anime_root_folder":"","radarr_minimum_availability":"released",
         "radarr_search":true,"sonarr_url":"","sonarr_api_key_set":false,"sonarr_quality_profile":0,
         "sonarr_root_folder":"","sonarr_language_profile":0,"sonarr_anime_quality_profile":0,
         "sonarr_anime_root_folder":"","sonarr_season_folders":true,"sonarr_search":true}
    """

    private val offer = """{"version":"5.2.0","quality_profiles":[{"id":4,"name":"HD-1080p"}],"root_folders":[{"path":"/movies","free_bytes":1024}],"language_profiles":[]}"""

    /** A server whose PUT /api/settings merges what it is sent, keeping keys out of what it answers, as the real one does. */
    private fun server(): FakeServerAdmin {
        val server = FakeServerAdmin()
        var now = PleyJson.parseToJsonElement(saved).jsonObject
        server.answer("GET /api/settings") { now }
        server.answer("PUT /api/settings") { body ->
            val sent = body!!.jsonObject
            val keys = sent.keys.filter { it.endsWith("_api_key") && it != "tmdb_api_key" }
            now = JsonObject(now + sent.filterKeys { it !in keys } + keys.associate { "${it}_set" to JsonPrimitive(sent[it]!!.jsonPrimitive.content.isNotEmpty()) })
            now
        }
        server.answer("POST /api/settings/arr", offer)
        return server
    }

    private fun sent(server: FakeServerAdmin): List<JsonElement?> = server.callsTo("PUT /api/settings").map { it.body }

    private fun json(text: String): JsonElement = PleyJson.parseToJsonElement(text)

    private fun RequestsAdminViewModel.now(): RequestsSettings = settings.state.value.valueOrNull!!

    @Test
    fun thePageOfRequestsTurnsThemOnAndSetsTheLimits() = runTest {
        val server = server()
        val page = RequestsAdminViewModel(server, service = null)
        page.setRequests(true).join()
        page.setMovieLimit(5).join()
        page.setSeasonLimit(0).join()
        assertEquals(
            listOf(json("""{"requests":true}"""), json("""{"request_limit_movies":5}"""), json("""{"request_limit_seasons":0}""")),
            sent(server),
        )
        assertEquals(true, page.now().requests)
        assertEquals(5, page.now().requestLimitMovies)
        // Only a service's page asks Radarr or Sonarr.
        assertTrue(server.callsTo("POST /api/settings/arr").isEmpty())
    }

    @Test
    fun aRefusalIsSaidAndWhatTheServerHasShown() = runTest {
        val server = server()
        val refused = ApiException(400, "bad_request", "requests need a TMDB key: titles are found on TMDB")
        server.fail("PUT /api/settings", refused)
        val page = RequestsAdminViewModel(server, service = null)
        val errors = mutableListOf<Throwable>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { page.errors.collect { errors += it } }
        page.setRequests(true).join()
        assertEquals(listOf<Throwable>(refused), errors)
        assertEquals(false, page.now().requests)
    }

    @Test
    fun aServicesPageAsksWhatTheSavedAddressOffersOnce() = runTest {
        val server = server()
        val page = RequestsAdminViewModel(server, ArrService.Radarr)
        assertEquals(json("""{"service":"radarr"}"""), server.callsTo("POST /api/settings/arr").single().body)
        assertEquals(ArrAnswer.Ready(PleyJson.decodeFromString(ArrOffer.serializer(), offer)), page.offer.value)
        // A choice doesn't change the address: nothing is asked again.
        page.setQualityProfile(4).join()
        page.setRootFolder("/movies").join()
        page.setAnimeQualityProfile(0).join()
        page.setAnimeRootFolder("").join()
        page.setSearch(false).join()
        page.setMinimumAvailability(RadarrAvailability.InCinemas).join()
        assertEquals(1, server.callsTo("POST /api/settings/arr").size)
        assertEquals(
            listOf(
                json("""{"radarr_quality_profile":4}"""),
                json("""{"radarr_root_folder":"/movies"}"""),
                json("""{"radarr_anime_quality_profile":0}"""),
                json("""{"radarr_anime_root_folder":""}"""),
                json("""{"radarr_search":false}"""),
                json("""{"radarr_minimum_availability":"in_cinemas"}"""),
            ),
            sent(server),
        )
        assertEquals(4L, page.now().radarrQualityProfile)
        assertFalse(page.now().radarrSearch)
    }

    @Test
    fun aServiceWithoutAnAddressOrKeyIsNotAsked() = runTest {
        val server = server()
        val page = RequestsAdminViewModel(server, ArrService.Sonarr)
        assertEquals(ArrAnswer.None, page.offer.value)
        assertTrue(server.callsTo("POST /api/settings/arr").isEmpty())
        page.setSeasonFolders(false).join()
        page.setLanguageProfile(2).join()
        assertEquals(listOf(json("""{"sonarr_season_folders":false}"""), json("""{"sonarr_language_profile":2}""")), sent(server))
    }

    @Test
    fun connectTriesWhatIsTypedWithoutSavingIt() = runTest {
        val server = server()
        val page = RequestsAdminViewModel(server, ArrService.Sonarr)
        val refused = ApiException(400, "bad_request", "Sonarr rejected the API key")
        server.fail("POST /api/settings/arr", refused)
        page.connect(" HTTP://NAS:8989/ ", "k3y").join()
        assertEquals(json("""{"service":"sonarr","url":"http://nas:8989","api_key":"k3y"}"""), server.callsTo("POST /api/settings/arr").single().body)
        assertEquals(ArrTrial("http://nas:8989", "k3y", ArrAnswer.Failed(refused)), page.trial.value)
        assertTrue(sent(server).isEmpty())
    }

    @Test
    fun theAddressAndKeyAreSavedTogetherAndTheNewAddressAsked() = runTest {
        val server = server()
        val page = RequestsAdminViewModel(server, ArrService.Sonarr)
        page.saveConnection("http://nas:8989/", "k3y").join()
        assertEquals(json("""{"sonarr_url":"http://nas:8989","sonarr_api_key":"k3y"}"""), sent(server).single())
        assertEquals(json("""{"service":"sonarr"}"""), server.callsTo("POST /api/settings/arr").single().body)
        assertTrue(page.offer.value is ArrAnswer.Ready)
        assertEquals(listOf(QualityProfile(4, "HD-1080p")), (page.offer.value as ArrAnswer.Ready).offer.qualityProfiles)
    }

    @Test
    fun aNewKeyForTheSameAddressIsAskedAgainAndNoAddressForgetsTheKey() = runTest {
        val server = server()
        val page = RequestsAdminViewModel(server, ArrService.Radarr)
        page.saveConnection("http://nas:7878", "new").join()
        assertEquals(2, server.callsTo("POST /api/settings/arr").size)
        page.saveConnection("", "").join()
        assertEquals(json("""{"radarr_url":""}"""), sent(server).last())
    }

    @Test
    fun anotherAddressNeedsItsKeyTyped() {
        assertTrue(arrNeedsKey("http://other:7878", "", "http://nas:7878", keySet = true))
        assertFalse(arrNeedsKey("http://other:7878", "k", "http://nas:7878", keySet = true))
        assertFalse(arrNeedsKey("HTTP://NAS:7878/", "", "http://nas:7878", keySet = true))
        assertFalse(arrNeedsKey("", "", "http://nas:7878", keySet = true))
        assertFalse(arrNeedsKey("http://other:7878", "", "", keySet = false))
        assertEquals("http://nas:7878/radarr", arrAddress(" http://NAS:7878/radarr/ "))
        assertEquals("nas:7878", arrAddress("nas:7878"))
    }
}
