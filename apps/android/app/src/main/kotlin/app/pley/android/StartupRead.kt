package app.pley.android

import android.content.Context
import androidx.tracing.trace
import app.pley.android.common.Addresses
import app.pley.android.common.Servers
import app.pley.android.common.TraceNames
import app.pley.android.common.perf.StartupSignals
import app.pley.android.data.LegacyPreferences
import app.pley.android.data.SavedServersJson
import app.pley.android.data.StartupFiles
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async

/** What the first screen is chosen by, read from this device only. */
data class StartupState(
    /** The server in use (its origin); `null` when none was opened. */
    val server: String?,
    /** Whether the app holds tokens for it (without opening the Keystore). */
    val signedIn: Boolean,
    /** Home as it was last shown, for the first frame; `null` when there is none. */
    val homeSnapshot: ByteArray?,
) {
    // Bytes compared by content would cost; nothing compares these.
    override fun equals(other: Any?): Boolean = this === other

    override fun hashCode(): Int = System.identityHashCode(this)
}

/**
 * The one disk read before the first frame, off the main thread: the
 * server in use, whether the app holds tokens for it, and Home's snapshot.
 * Never the network, never the Keystore. The splash screen waits for it
 * (300 ms at most).
 */
object StartupRead {

    fun start(context: Context): Deferred<StartupState> {
        val app = context.applicationContext
        return CoroutineScope(SupervisorJob() + Dispatchers.IO).async {
            try {
                trace(TraceNames.STARTUP_READ) { read(app) }
            } finally {
                StartupSignals.markStartupRead()
            }
        }
    }

    private fun read(context: Context): StartupState {
        // The data module writes the active server's origin beside Home's copy; until it has
        // (the first start after the web view app), the servers that app saved.
        val server = File(context.filesDir, StartupFiles.SERVER).takeIf { it.isFile }?.readText()?.trim()?.ifEmpty { null }
            ?: Servers.lastUsed(legacyServers(context))?.url?.let(Addresses::origin)
        // The sign-in module's shelf keeps each server's sealed tokens under "tokens:<issuer>".
        val signedIn = server != null &&
            context.getSharedPreferences("sign_in", Context.MODE_PRIVATE).contains("tokens:$server")
        val snapshot = File(context.filesDir, StartupFiles.HOME).takeIf { it.isFile }?.readBytes()
        return StartupState(server, signedIn, snapshot)
    }

    private fun legacyServers(context: Context) =
        SavedServersJson.decode(
            context.getSharedPreferences(LegacyPreferences.SERVERS_FILE, Context.MODE_PRIVATE).getString(LegacyPreferences.SERVERS_KEY, null),
        )
}
