package app.pley.android.player.session

import app.pley.android.data.LocalMedia
import app.pley.android.model.ProgressReport
import app.pley.android.network.PleyApi

/**
 * The API as a downloaded file's playback uses it: its progress goes to
 * [local] without a session (the server never made one), which sends it
 * or keeps it until the server can be reached; there is no session to
 * delete. Everything else is [api]'s.
 */
internal class LocalReportsApi(private val api: PleyApi, private val local: LocalMedia) : PleyApi by api {
    override suspend fun reportProgress(itemId: Long, report: ProgressReport) {
        local.report(itemId, report.copy(sessionId = null))
    }

    override suspend fun stopPlayback(sessionId: String) = Unit
}
