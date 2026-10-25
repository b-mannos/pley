package app.pley.android.ui.tv.shell

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import app.pley.android.auth.AuthEvent
import app.pley.android.data.valueOrNull
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.model.Library
import app.pley.android.screens.ScreensGraph
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map

/**
 * What the TV's frame reads: how to start, what the rail lists, and who
 * is watching. The app's data in the app ([of]), fakes in tests.
 */
interface TvShellSource {
    /** The start gate's facts, from this device only (a disk read, no network). */
    suspend fun startFacts(): StartFacts

    /** The libraries this profile sees, in the server's order; empty while unknown. */
    val libraries: Flow<List<Library>>

    /** Who is watching, for the rail's avatar; `null` while unknown. */
    val profile: Flow<RailProfile?>

    /**
     * Keeps what screens show in step with the server (server events) for
     * as long as it runs: the shell runs it while the app is started.
     */
    suspend fun keepUpdated()

    /** The servers the app was signed out of without being asked (a refused sign-in): sign-in shows again. */
    val signedOut: Flow<String> get() = emptyFlow()

    /** pley's own "Reduce motion" switch (the system's setting counts as well). */
    val reduceMotion: Flow<Boolean> get() = flowOf(false)

    companion object {
        /**
         * The app's: the startup read's server and tokens ([startup]),
         * "Always start as" from the settings, and the household's size as
         * the data module last saw it ([householdSize]; 1 until it says).
         */
        fun of(
            graph: ScreensGraph,
            startup: suspend () -> Pair<String?, Boolean>,
            householdSize: suspend () -> Int = { 1 },
        ): TvShellSource = GraphShellSource(graph, startup, householdSize)
    }
}

/** The rail's avatar: the profile's name, colour and picture. */
@Immutable
data class RailProfile(
    val name: String,
    /** The circle behind the initial; `null`: pley's default. */
    val color: Color? = null,
    val picture: ArtworkImage? = null,
) {
    val initial: String get() = name.trim().firstOrNull()?.uppercase() ?: "?"

    companion object {
        /** "#e07a5f" as a colour; `null` when it isn't one. */
        fun parseColor(hex: String?): Color? {
            val digits = hex?.removePrefix("#") ?: return null
            if (digits.length != 6) return null
            val rgb = digits.toLongOrNull(16) ?: return null
            return Color(0xFF000000 or rgb)
        }
    }
}

private class GraphShellSource(
    private val graph: ScreensGraph,
    private val startup: suspend () -> Pair<String?, Boolean>,
    private val householdSize: suspend () -> Int,
) : TvShellSource {
    override suspend fun startFacts(): StartFacts {
        val (server, signedIn) = startup()
        val settings = graph.data.settings.settings.first()
        return StartFacts(server, signedIn, profiles = householdSize(), alwaysStartAs = settings.alwaysStartAs)
    }

    override val libraries: Flow<List<Library>> =
        graph.data.libraries.libraries().map { it.valueOrNull.orEmpty() }.distinctUntilChanged()

    override val profile: Flow<RailProfile?> = graph.data.session.session.map { session ->
        val me = session?.me ?: return@map null
        RailProfile(
            name = me.name,
            color = RailProfile.parseColor(me.avatarColor),
            picture = ArtworkImage.of(me.avatarUrl, session.issuer),
        )
    }.distinctUntilChanged()

    override suspend fun keepUpdated() {
        // Server events flow while something listens to them (the data module's LiveUpdates).
        graph.data.liveUpdates.scans.collect { }
    }

    override val signedOut: Flow<String> = graph.auth.events.filterIsInstance<AuthEvent.SignedOut>().map { it.issuer }

    override val reduceMotion: Flow<Boolean> = graph.data.settings.settings.map { it.reduceMotion }.distinctUntilChanged()
}
