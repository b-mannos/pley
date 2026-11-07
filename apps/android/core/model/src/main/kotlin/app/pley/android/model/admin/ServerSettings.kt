package app.pley.android.model.admin

import app.pley.android.model.LenientEnumSerializer
import app.pley.android.model.WireEnum
import kotlinx.serialization.Serializable

/*
 * Settings → Server (apps/web/src/pages/settings/ServerSettings.tsx): the
 * fields of GET /api/settings that page edits (apps/server/src/settings.rs),
 * what the server and its transcoder can do (GET /api/system), tests of
 * metadata services (POST /api/settings/test), when library work runs
 * (GET /api/system/library-work) and the database's backups
 * (GET /api/system/backups). Changes go out as partial bodies of
 * PUT /api/settings, which answers with every setting.
 */

/** The server's settings this page edits; the rest are other pages'. */
@Serializable
data class ServerSettings(
    val serverName: String = "",
    /** "en", "fr", "es"; empty: not chosen, which reads as English. */
    val language: String = "",
    /** Sent back to administrators, as the web shows it when asked to. */
    val tmdbApiKey: String = "",
    /** "en-US". */
    val metadataLanguage: String = "en-US",
    val tvMetadata: TvMetadataSource = TvMetadataSource.Auto,
    val tvdbApiKey: String = "",
    /** Subscriber PIN, for TheTVDB keys that need one. */
    val tvdbPin: String = "",
    val musicMetadata: Boolean = true,
    /** Absent from servers before NFO files were read. */
    val readNfo: Boolean? = null,
    /** Minutes between full scans; 0: none. */
    val scanIntervalMinutes: Int = 360,
    val watchLibraries: Boolean = true,
    val detectMarkers: Boolean = true,
    val seekPreviews: Boolean = true,
    val previewsInBackground: Boolean = true,
    /** Absent from servers before library work was scheduled, as are the three after it. */
    val libraryWork: LibraryWorkMode? = null,
    /** The hours (0 to 23, the server's time) of [LibraryWorkMode.Hours]. */
    val libraryWorkFrom: Int = 1,
    val libraryWorkTo: Int = 7,
    /** Library work waits while other programs keep more of the processors busy (%, 10 to 100; 100: never). */
    val libraryWorkMaxCpu: Int = 50,
    val thirdPartyApps: Boolean = false,
    /** Absent from servers before two-step sign-in could be required. */
    val requireTwoStep: Boolean? = null,
    val opensubtitlesApiKey: String = "",
    val opensubtitlesUsername: String = "",
    /** A password is saved; the server never sends it back. */
    val opensubtitlesPasswordSet: Boolean = false,
)

/** Where shows' metadata comes from. */
@Serializable(with = TvMetadataSource.Serializer::class)
enum class TvMetadataSource(override val wire: String) : WireEnum {
    /** TMDB with a key, else TheTVDB with one, else TVmaze. */
    Auto("auto"),
    Tmdb("tmdb"),
    Tvdb("tvdb"),
    Tvmaze("tvmaze"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<TvMetadataSource>("TvMetadataSource", TvMetadataSource.entries, TvMetadataSource.Unknown)
}

/** When intro and credits detection and previews of whole libraries run, besides waiting while anything plays. */
@Serializable(with = LibraryWorkMode.Serializer::class)
enum class LibraryWorkMode(override val wire: String) : WireEnum {
    /** In the hours people rarely watch, as the server learns them. */
    Auto("auto"),

    /** Between set hours. */
    Hours("hours"),

    /** Whenever nothing plays. */
    Idle("idle"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<LibraryWorkMode>("LibraryWorkMode", LibraryWorkMode.entries, LibraryWorkMode.Unknown)
}

/** GET /api/system, the parts this page goes by. */
@Serializable
data class ServerSystem(
    /** The server can use its secret.key: without it, authenticator apps can't be set up or checked. */
    val secretKeyOk: Boolean? = null,
    val transcoder: ServerTranscoder? = null,
)

/** What the server's transcoder can do, as far as this page goes. */
@Serializable
data class ServerTranscoder(
    val available: Boolean = false,
    val encoders: List<String> = emptyList(),
    /** Audio fingerprints, which find intros and credits that chapters don't name. */
    val chromaprint: Boolean = false,
) {
    /** An FFmpeg that encodes nothing isn't there: what it can do is unknown (the web's `knownTranscoder`). */
    val known: Boolean get() = available && encoders.isNotEmpty()
}

/** A service POST /api/settings/test checks. */
@Serializable(with = TestedService.Serializer::class)
enum class TestedService(override val wire: String) : WireEnum {
    Tmdb("tmdb"),
    Tvdb("tvdb"),
    Tvmaze("tvmaze"),
    Musicbrainz("musicbrainz"),
    Opensubtitles("opensubtitles"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<TestedService>("TestedService", TestedService.entries, TestedService.Unknown)
}

/** How a test went, in the server's words, and how long the answer took. */
@Serializable
data class ServiceTestResult(
    val ok: Boolean = false,
    val message: String = "",
    val ms: Long = 0,
)

/** GET /api/system/library-work: the week as the server learned it, from Sunday midnight in its time. */
@Serializable
data class LibraryWorkWeek(
    val mode: LibraryWorkMode = LibraryWorkMode.Unknown,
    /** Why work on the rest of the libraries waits now, if it does. */
    val waiting: LibraryWorkWait? = null,
    /** When the hours it waits for begin, in Unix seconds. */
    val resumesAt: Long? = null,
    /** Whether the hours people watch are known yet. */
    val learned: Boolean = false,
    /** The hour of the week now: 0 is Sunday from midnight. */
    val now: Int = -1,
    /** The 168 hours of the week. */
    val hours: List<WatchingHour> = emptyList(),
)

/** One hour of the week. */
@Serializable
data class WatchingHour(
    /** The share of its minutes something played in, 0 to 1; `null` where it wasn't seen enough to tell. */
    val playing: Double? = null,
    /** Work on the rest of the libraries may begin then. */
    val open: Boolean = false,
)

/** Why library work waits. */
@Serializable(with = LibraryWorkWait.Serializer::class)
enum class LibraryWorkWait(override val wire: String) : WireEnum {
    Playing("playing"),
    Busy("busy"),
    Hours("hours"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<LibraryWorkWait>("LibraryWorkWait", LibraryWorkWait.entries, LibraryWorkWait.Unknown)
}

/** An account of GET /api/users, as far as requiring two-step sign-in goes. */
@Serializable
data class SignInAccount(
    val id: Long,
    val hasPassword: Boolean = false,
    val twoStep: Boolean = false,
    val disabled: Boolean = false,
)
