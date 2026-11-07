package app.pley.android.model.admin

import app.pley.android.model.ArrService
import app.pley.android.model.PleyJson
import app.pley.android.model.QualityProfile
import app.pley.android.model.RootFolder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Reading what the server writes for Settings → Requests (apps/server/src/settings.rs, requests/arr.rs). */
class RequestsSettingsTest {
    @Test
    fun theSettingsReadAsTheServerWritesThemWithoutTheKeys() {
        val settings = PleyJson.decodeFromString(
            RequestsSettings.serializer(),
            """
            {"server_name":"Den","tmdb_api_key":"abc","requests":true,"request_limit_movies":5,"request_limit_seasons":0,
             "radarr_url":"http://nas:7878","radarr_quality_profile":4,"radarr_root_folder":"/movies",
             "radarr_anime_quality_profile":0,"radarr_anime_root_folder":"","radarr_minimum_availability":"in_cinemas",
             "radarr_search":false,"sonarr_url":"","sonarr_quality_profile":0,"sonarr_root_folder":"",
             "sonarr_language_profile":2,"sonarr_anime_quality_profile":0,"sonarr_anime_root_folder":"",
             "sonarr_season_folders":false,"sonarr_search":true,"opensubtitles_password_set":false,
             "radarr_api_key_set":true,"sonarr_api_key_set":false}
            """,
        )
        assertEquals(true, settings.requests)
        assertEquals(5, settings.requestLimitMovies)
        assertEquals(RadarrAvailability.InCinemas, settings.radarrMinimumAvailability)
        assertEquals(2L, settings.sonarrLanguageProfile)
        assertFalse(settings.sonarrSeasonFolders)
        val radarr = settings.of(ArrService.Radarr)
        assertEquals("http://nas:7878", radarr.url)
        assertTrue(radarr.keySet)
        assertEquals(4L, radarr.qualityProfile)
        assertEquals("/movies", radarr.rootFolder)
        assertFalse(radarr.search)
        val sonarr = settings.of(ArrService.Sonarr)
        assertEquals("", sonarr.url)
        assertFalse(sonarr.keySet)
        assertTrue(sonarr.search)
    }

    @Test
    fun aServerBeforeRequestsTakesNone() {
        assertNull(PleyJson.decodeFromString(RequestsSettings.serializer(), """{"server_name":"Den"}""").requests)
    }

    @Test
    fun whatRadarrOrSonarrOffersReadsWithItsFoldersSpace() {
        val offer = PleyJson.decodeFromString(
            ArrOffer.serializer(),
            """{"version":"4.0.1","quality_profiles":[{"id":1,"name":"HD-1080p"}],
               "root_folders":[{"path":"/tv","free_bytes":1024},{"path":"/anime","free_bytes":null}],
               "language_profiles":[]}""",
        )
        assertEquals("4.0.1", offer.version)
        assertEquals(listOf(QualityProfile(1, "HD-1080p")), offer.qualityProfiles)
        assertEquals(listOf(RootFolder("/tv", 1024), RootFolder("/anime", null)), offer.rootFolders)
        assertTrue(offer.languageProfiles.isEmpty())
    }
}
