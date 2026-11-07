package app.pley.android.model.admin

import app.pley.android.model.LenientEnumSerializer
import app.pley.android.model.LivePlan
import app.pley.android.model.WireEnum
import kotlinx.serialization.Serializable

/*
 * Live TV's administration (docs/API.md, "Live TV"): its settings, the
 * sources channels come from, the lineup as administrators see it, and
 * the connections in use. The app's own Live TV types (LiveChannel,
 * LiveGroup…) are what people watch; these are what administrators manage.
 */

/** Who Live TV is open to ([LiveTvSettings.access]). */
@Serializable(with = LiveTvAccess.Serializer::class)
enum class LiveTvAccess(override val wire: String) : WireEnum {
    Everyone("everyone"),

    /** Accounts in the household, not friends given access. */
    Home("home"),
    Admins("admins"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<LiveTvAccess>("LiveTvAccess", entries, Unknown)
}

/** GET and PUT /api/live/settings, with the server's defaults. */
@Serializable
data class LiveTvSettings(
    val access: LiveTvAccess = LiveTvAccess.Everyone,
    /** Profiles with an age limit see Live TV too. */
    val ageLimited: Boolean = false,
    val deinterlace: Boolean = true,
    /** The most lines a transcoded channel has; `null`: as many as it has. */
    val maxHeight: Int? = null,
    val refreshHours: Int = 24,
    val guideDays: Int = 7,
    /** Channels that don't play, and a provider's divider rows, are left out. */
    val hideOffline: Boolean = true,
    /** People who watch whatever [access] says, and people who never do. */
    val allowedUsers: List<Long> = emptyList(),
    val deniedUsers: List<Long> = emptyList(),
)

/** An account, as Live TV's "People" lists them (GET /api/users). */
@Serializable
data class LiveTvPerson(
    val id: Long,
    val name: String = "",
    val isAdmin: Boolean = false,
    /** In the household. */
    val home: Boolean = true,
)

/** Where a source's channels come from. */
@Serializable(with = LiveSourceKind.Serializer::class)
enum class LiveSourceKind(override val wire: String) : WireEnum {
    /** A playlist's address. */
    M3u("m3u"),

    /** An Xtream Codes server, with a user name and password. */
    Xtream("xtream"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<LiveSourceKind>("LiveSourceKind", entries, Unknown)
}

/** What an Xtream Codes server says of the account. */
@Serializable
data class LiveSourceAccount(
    /** "Active", "Expired", "Banned", "Disabled", as the provider writes it. */
    val status: String = "",
    /** When it runs out (unix seconds); `null`: never. */
    val expiresAt: Long? = null,
    val maxConnections: Int? = null,
    val activeConnections: Int? = null,
    val trial: Boolean = false,
)

/** A source as administrators see it (GET /api/live/sources), never its password. */
@Serializable
data class LiveSource(
    val id: Long,
    val name: String = "",
    val kind: LiveSourceKind = LiveSourceKind.Unknown,
    val url: String = "",
    val username: String = "",
    val passwordSet: Boolean = false,
    val guideUrl: String = "",
    val userAgent: String = "",
    /** 0: no limit. */
    val maxConnections: Int = 1,
    val enabled: Boolean = true,
    val channelsAt: Long? = null,
    val channelsError: String? = null,
    val guideAt: Long? = null,
    val guideError: String? = null,
    val account: LiveSourceAccount? = null,
    val channelCount: Int = 0,
    val hiddenCount: Int = 0,
    val guidedCount: Int = 0,
    val connectionsInUse: Int = 0,
    /** Being read now. */
    val refreshing: Boolean = false,
)

/**
 * A source to add or change (POST /api/live/sources, PUT …/{id}, and
 * POST …/test). [password] `null` is left out, which keeps the saved one.
 */
@Serializable
data class LiveSourceInput(
    val name: String,
    val kind: LiveSourceKind,
    val url: String,
    val username: String = "",
    val password: String? = null,
    val guideUrl: String = "",
    val userAgent: String = "",
    val maxConnections: Int = 1,
    val enabled: Boolean = true,
)

/** What testing a source's details found (POST /api/live/sources/test). */
@Serializable
data class LiveSourceTest(
    val channels: Int = 0,
    val groups: Int = 0,
    /** The guide it would read; `null`: none. */
    val guideUrl: String? = null,
    val account: LiveSourceAccount? = null,
)

/** A channel as administrators see it: as the source has it, and as set here. */
@Serializable
data class LineupChannel(
    val id: Long,
    val sourceId: Long = 0,
    val sourceName: String = "",
    val name: String = "",
    val number: String? = null,
    val group: String = "",
    val logo: String? = null,
    /** The guide channel it uses. */
    val guideId: String = "",
    val hidden: Boolean = false,
    val hasGuide: Boolean = false,
    val sourceNumber: String? = null,
    val sourceGuideId: String = "",
    /** The guide's channel of its name, when the source names none. */
    val matchedGuideId: String? = null,
    val customName: String? = null,
    val customNumber: String? = null,
    val customGuideId: String? = null,
    /** It didn't play when last tried, or it is a provider's divider row. */
    val offline: Boolean = false,
)

/** GET /api/live/lineup/page: [channels] from [offset] of [total], of which [hidden] are hidden. */
@Serializable
data class LineupPage(
    val total: Int = 0,
    val hidden: Int = 0,
    val offset: Int = 0,
    val channels: List<LineupChannel> = emptyList(),
)

/** A group as administrators see it (GET /api/live/lineup/groups): hidden and offline channels counted. */
@Serializable
data class LineupGroup(
    val name: String,
    /** The region its name begins with ("UK" of "UK| Sport"). */
    val region: String? = null,
    /** The name without its region ("Sport"). */
    val label: String = name,
    val count: Int = 0,
    val hidden: Int = 0,
    val offline: Int = 0,
)

/** Which of the lineup's channels are listed or changed. */
@Serializable(with = LineupState.Serializer::class)
enum class LineupState(override val wire: String) : WireEnum {
    All("all"),
    Shown("shown"),
    Hidden("hidden"),
    NoGuide("no-guide"),
    Offline("offline"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<LineupState>("LineupState", entries, Unknown)
}

/** The lineup's channels to list or change: of a source, of a group, in a state, matching [q]. */
@Serializable
data class LineupFilter(
    val sourceId: Long? = null,
    val group: String? = null,
    val state: LineupState = LineupState.All,
    /** A name, or a number's beginning. */
    val q: String? = null,
)

/**
 * PATCH /api/live/lineup: [ids], every channel of [groups] and every one
 * [filter] keeps. What is `null` is left out and stays; `""` gives back
 * the source's own name, number or guide channel.
 */
@Serializable
data class LineupPatch(
    val ids: List<Long> = emptyList(),
    val groups: List<String>? = null,
    val filter: LineupFilter? = null,
    val hidden: Boolean? = null,
    val name: String? = null,
    val number: String? = null,
    val guideId: String? = null,
)

/** Someone watching a channel a tuner pulls. */
@Serializable
data class TunerViewer(
    val sessionId: String = "",
    val username: String = "",
)

/** A connection in use (GET /api/live/tuners): the channel pulled and who watches it. */
@Serializable
data class TunerStatus(
    val channelId: Long,
    val channelName: String = "",
    val sourceName: String = "",
    val startedAt: Long = 0,
    /** The provider is sending. */
    val connected: Boolean = false,
    val error: String? = null,
    /** What the provider sends. */
    val kbps: Long = 0,
    val viewers: List<TunerViewer> = emptyList(),
    /** Nobody watches: it goes soon. */
    val idle: Boolean = false,
    /** What the transcoder makes of it, for HLS viewers. */
    val plan: LivePlan? = null,
)
