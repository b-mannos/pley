package app.pley.android.downloads

import app.pley.android.data.DownloadState
import app.pley.android.model.ItemKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The episode a download goes on to while the server is away. */
class NextEpisodeTest {
    private fun episode(id: Long, season: Int?, number: Int?, show: Long = 9, state: DownloadState = DownloadState.Done) = Saved(
        server = "http://nas:7539",
        userId = 2,
        itemId = id,
        fileId = id * 10,
        kind = ItemKind.Episode,
        title = "Episode $id",
        showId = show,
        showTitle = "The Expanse",
        seasonIndex = season,
        episodeIndex = number,
        state = state,
        addedAt = id,
    )

    @Test
    fun theNextEpisodeDownloadedWholeComesNextInOrder() {
        val s1e1 = episode(1, 1, 1)
        val s1e2 = episode(2, 1, 2, state = DownloadState.Downloading)
        val s1e3 = episode(3, 1, 3)
        val s2e1 = episode(4, 2, 1)
        val other = episode(5, 1, 2, show = 8)
        // Listed newest first, as downloads are.
        val mine = listOf(s2e1, other, s1e3, s1e2, s1e1)

        assertEquals("not downloaded whole yet: left out", 3L, AppDownloads.nextEpisode(mine, s1e1)?.itemId)
        assertEquals("on into the next season", 4L, AppDownloads.nextEpisode(mine, s1e3)?.itemId)
        assertNull("the last one downloaded", AppDownloads.nextEpisode(mine, s2e1))
    }

    @Test
    fun aMovieHasNoNextEpisode() {
        val movie = episode(1, null, null).copy(kind = ItemKind.Movie, showId = null)
        assertNull(AppDownloads.nextEpisode(listOf(movie, episode(2, 1, 1)), movie))
    }
}
