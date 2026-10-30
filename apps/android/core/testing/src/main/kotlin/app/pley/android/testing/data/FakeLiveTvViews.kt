package app.pley.android.testing.data

import app.pley.android.data.LiveTvView
import app.pley.android.data.LiveTvViews

/** How Live TV was left, in memory: [kept] is what a restart would read. */
class FakeLiveTvViews(var kept: LiveTvView = LiveTvView()) : LiveTvViews {
    override suspend fun of(): LiveTvView = kept

    override suspend fun keep(category: String?, view: String?, channel: Long?) {
        kept = LiveTvView(category ?: kept.category, view ?: kept.view, channel ?: kept.channel)
    }
}
