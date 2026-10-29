package app.pley.android.player.session

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** Subtitle files on the device are read for a download's own session only. */
class SavedSubtitleTest {
    @Test
    fun aDownloadReadsItsSavedSubtitles() {
        assertTrue(isSavedSubtitle("file:///data/user/0/app.pley.android/files/downloads/7/subs/3.vtt", "file:///data/user/0/app.pley.android/files/downloads/7/media.mkv"))
    }

    @Test
    fun aStreamNeverReadsAFileTheServerNames() {
        val stream = "/api/playback/abc/master.m3u8"
        assertFalse(isSavedSubtitle("file:///data/user/0/app.pley.android/shared_prefs/sign_in.xml", stream))
        assertFalse(isSavedSubtitle("file:///data/user/0/app.pley.android/shared_prefs/sign_in.xml", "https://pley.example.com/api/media/k/file.mkv"))
        assertFalse(isSavedSubtitle("/api/files/7/subtitles/3.vtt", stream))
    }
}
