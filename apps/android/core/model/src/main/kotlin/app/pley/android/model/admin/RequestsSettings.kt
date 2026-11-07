package app.pley.android.model.admin

import app.pley.android.model.ArrService
import app.pley.android.model.LenientEnumSerializer
import app.pley.android.model.QualityProfile
import app.pley.android.model.RootFolder
import app.pley.android.model.WireEnum
import kotlinx.serialization.Serializable

/*
 * Settings → Requests (apps/web/src/pages/settings/RequestsSettings.tsx):
 * the fields of GET /api/settings that page edits (apps/server/src/settings.rs),
 * and what Radarr or Sonarr offers (POST /api/settings/arr). Changes go out
 * as partial bodies of PUT /api/settings.
 */

/** The server's settings for requests; [requests] is `null` from a server before them. */
@Serializable
data class RequestsSettings(
    val requests: Boolean? = null,
    /** Requests need one: titles are found on TMDB. */
    val tmdbApiKey: String = "",
    /** Requests an account may make in 7 days, 0 (no limit) to 1000. */
    val requestLimitMovies: Int = 0,
    /** As many seasons: a show counts its seasons. */
    val requestLimitSeasons: Int = 0,
    /** "http://localhost:7878"; empty: no Radarr, and approved movies are added by hand. */
    val radarrUrl: String = "",
    /** A key is saved; the server never sends it back. */
    val radarrApiKeySet: Boolean = false,
    /** Radarr's id; 0: none chosen, and approving fails until one is. */
    val radarrQualityProfile: Long = 0,
    /** Empty: none chosen. */
    val radarrRootFolder: String = "",
    /** For anime; 0: [radarrQualityProfile]. */
    val radarrAnimeQualityProfile: Long = 0,
    /** For anime; empty: [radarrRootFolder]. */
    val radarrAnimeRootFolder: String = "",
    val radarrMinimumAvailability: RadarrAvailability = RadarrAvailability.Released,
    val radarrSearch: Boolean = true,
    val sonarrUrl: String = "",
    val sonarrApiKeySet: Boolean = false,
    val sonarrQualityProfile: Long = 0,
    val sonarrRootFolder: String = "",
    /** Sonarr 3's language profile; 0: its first (Sonarr 4 has none). */
    val sonarrLanguageProfile: Long = 0,
    val sonarrAnimeQualityProfile: Long = 0,
    val sonarrAnimeRootFolder: String = "",
    val sonarrSeasonFolders: Boolean = true,
    val sonarrSearch: Boolean = true,
) {
    /** [service]'s fields, as one. */
    fun of(service: ArrService): ArrSettings = when (service) {
        ArrService.Sonarr -> ArrSettings(
            url = sonarrUrl,
            keySet = sonarrApiKeySet,
            qualityProfile = sonarrQualityProfile,
            rootFolder = sonarrRootFolder,
            animeQualityProfile = sonarrAnimeQualityProfile,
            animeRootFolder = sonarrAnimeRootFolder,
            search = sonarrSearch,
        )
        else -> ArrSettings(
            url = radarrUrl,
            keySet = radarrApiKeySet,
            qualityProfile = radarrQualityProfile,
            rootFolder = radarrRootFolder,
            animeQualityProfile = radarrAnimeQualityProfile,
            animeRootFolder = radarrAnimeRootFolder,
            search = radarrSearch,
        )
    }
}

/** The fields Radarr and Sonarr both have, of one of them. */
data class ArrSettings(
    val url: String,
    val keySet: Boolean,
    val qualityProfile: Long,
    val rootFolder: String,
    val animeQualityProfile: Long,
    val animeRootFolder: String,
    val search: Boolean,
)

/** When Radarr may take a release of a movie it was sent. */
@Serializable(with = RadarrAvailability.Serializer::class)
enum class RadarrAvailability(override val wire: String) : WireEnum {
    Announced("announced"),
    InCinemas("in_cinemas"),
    Released("released"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<RadarrAvailability>("RadarrAvailability", RadarrAvailability.entries, RadarrAvailability.Unknown)
}

/** POST /api/settings/arr: what Radarr or Sonarr offers requests. */
@Serializable
data class ArrOffer(
    val version: String = "",
    val qualityProfiles: List<QualityProfile> = emptyList(),
    val rootFolders: List<RootFolder> = emptyList(),
    /** Sonarr 3's; empty otherwise. */
    val languageProfiles: List<QualityProfile> = emptyList(),
)
