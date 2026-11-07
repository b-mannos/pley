package app.pley.android.model.admin

import app.pley.android.model.PleyJson
import kotlinx.serialization.builtins.ListSerializer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Reading what the server writes for Settings → Server (apps/server/src/settings.rs, api/admin.rs, schedule.rs, backup.rs). */
class ServerSettingsTest {
    @Test
    fun theSettingsReadAsTheServerWritesThemWithEveryOtherPagesFieldsSkipped() {
        val settings = PleyJson.decodeFromString(
            ServerSettings.serializer(),
            """
            {"server_name":"Den","language":"fr","tmdb_api_key":"abc","metadata_language":"fr-FR","tv_metadata":"tvdb",
             "tvdb_api_key":"k","tvdb_pin":"1234","music_metadata":false,"read_nfo":true,"hwaccel":"auto",
             "scan_interval_minutes":0,"watch_libraries":false,"detect_markers":true,"seek_previews":true,
             "previews_in_background":false,"library_work":"hours","library_work_from":22,"library_work_to":6,
             "library_work_max_cpu":75,"third_party_apps":true,"require_two_step":false,"proxy_addresses":[],
             "opensubtitles_api_key":"os","opensubtitles_username":"ann","opensubtitles_password_set":true,
             "requests":true,"radarr_api_key_set":false}
            """,
        )
        assertEquals("Den", settings.serverName)
        assertEquals("fr", settings.language)
        assertEquals(TvMetadataSource.Tvdb, settings.tvMetadata)
        assertEquals("1234", settings.tvdbPin)
        assertFalse(settings.musicMetadata)
        assertEquals(true, settings.readNfo)
        assertEquals(0, settings.scanIntervalMinutes)
        assertEquals(LibraryWorkMode.Hours, settings.libraryWork)
        assertEquals(22, settings.libraryWorkFrom)
        assertEquals(6, settings.libraryWorkTo)
        assertEquals(75, settings.libraryWorkMaxCpu)
        assertTrue(settings.thirdPartyApps)
        assertEquals(false, settings.requireTwoStep)
        assertEquals("ann", settings.opensubtitlesUsername)
        assertTrue(settings.opensubtitlesPasswordSet)
    }

    @Test
    fun aServerBeforeTheNewerSettingsLeavesThemUnknown() {
        val settings = PleyJson.decodeFromString(ServerSettings.serializer(), """{"server_name":"Den","tv_metadata":"anidb"}""")
        assertNull(settings.readNfo)
        assertNull(settings.libraryWork)
        assertNull(settings.requireTwoStep)
        assertEquals(TvMetadataSource.Unknown, settings.tvMetadata)
        assertEquals(50, settings.libraryWorkMaxCpu)
    }

    @Test
    fun theSystemSaysWhetherItsTranscoderIsKnownAndFindsIntros() {
        val system = PleyJson.decodeFromString(
            ServerSystem.serializer(),
            """{"name":"Den","secret_key_ok":false,"transcoder":{"available":true,"encoders":["libx264"],"accels":[],"chromaprint":false},"counts":{}}""",
        )
        assertEquals(false, system.secretKeyOk)
        val transcoder = system.transcoder!!
        assertTrue(transcoder.known)
        assertFalse(transcoder.chromaprint)
        val none = PleyJson.decodeFromString(ServerSystem.serializer(), """{"transcoder":{"available":false,"encoders":[]}}""")
        assertFalse(none.transcoder!!.known)
    }

    @Test
    fun aTestSaysHowItWent() {
        val result = PleyJson.decodeFromString(ServiceTestResult.serializer(), """{"ok":true,"message":"TMDB accepted the key.","ms":212}""")
        assertEquals(ServiceTestResult(true, "TMDB accepted the key.", 212), result)
    }

    @Test
    fun theWeekOfLibraryWorkIsTheStatusFlattenedWithItsHours() {
        val week = PleyJson.decodeFromString(
            LibraryWorkWeek.serializer(),
            """{"mode":"auto","waiting":"hours","resumes_at":1700000000,"learned":true,"now":30,
               "hours":[{"playing":null,"open":false},{"playing":0.25,"open":true}]}""",
        )
        assertEquals(LibraryWorkMode.Auto, week.mode)
        assertEquals(LibraryWorkWait.Hours, week.waiting)
        assertEquals(1_700_000_000L, week.resumesAt)
        assertEquals(30, week.now)
        assertEquals(listOf(WatchingHour(null, false), WatchingHour(0.25, true)), week.hours)
        val idle = PleyJson.decodeFromString(LibraryWorkWeek.serializer(), """{"mode":"idle","waiting":null,"resumes_at":null,"learned":false,"now":0,"hours":[]}""")
        assertNull(idle.waiting)
    }

    @Test
    fun backupsReadWithTheDatabasesHealth() {
        val backups = PleyJson.decodeFromString(
            DatabaseBackups.serializer(),
            """{"health":{"ok":false,"problem":"page 4 is never used","checked_at":1700000000},
               "backups":[{"name":"pley-2026-11-07.db","path":"/data/backups/pley-2026-11-07.db","bytes":2048,"created_at":1700000000}]}""",
        )
        assertEquals(false, backups.health.ok)
        assertEquals("page 4 is never used", backups.health.problem)
        assertEquals(listOf(DatabaseBackup("pley-2026-11-07.db", 2048, 1_700_000_000)), backups.backups)
        assertNull(PleyJson.decodeFromString(DatabaseBackups.serializer(), """{"health":{"ok":null,"checked_at":null},"backups":[]}""").health.ok)
    }

    @Test
    fun accountsSayWhetherTheySignInWithTwoSteps() {
        val accounts = PleyJson.decodeFromString(
            ListSerializer(SignInAccount.serializer()),
            """[{"id":1,"username":"ann","name":"Ann","has_password":true,"two_step":true,"disabled":false},
                {"id":2,"username":"kid","name":"Kid","managed":true,"has_password":false,"two_step":false}]""",
        )
        assertEquals(listOf(SignInAccount(1, true, true, false), SignInAccount(2, false, false, false)), accounts)
    }
}
