package app.pley.android.ui.tv.item

import androidx.activity.ComponentActivity
import androidx.compose.ui.test.isFocusable
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import app.pley.android.model.DynamicRange
import app.pley.android.model.MediaFile
import app.pley.android.model.MediaStream
import app.pley.android.model.StreamKind
import app.pley.android.screens.item.FileInfo
import app.pley.android.ui.tv.theme.TvTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/** Media info on the item page: every stream in full, as text that never takes focus. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w960dp-h540dp-xhdpi")
class TvMediaInfoTest {
    @get:Rule
    val compose = createAndroidComposeRule<ComponentActivity>()

    private val file = MediaFile(
        id = 1, path = "/srv/media/Film (2024)/Film (2024).mkv", size = 62_000_000_000, container = "mkv", durationMs = 7_200_000, bitrate = 62_000_000,
        streams = listOf(
            MediaStream(
                0, StreamKind.Video, "hevc", profile = "Main 10", level = 153, width = 3840, height = 1608, frameRate = 23.976, bitDepth = 10,
                pixFmt = "yuv420p10le", dynamicRange = DynamicRange.DolbyVision, dvProfile = 7, dvLevel = 6, dvBlCompat = 6,
                colorPrimaries = "bt2020", colorTransfer = "smpte2084", colorSpace = "bt2020nc",
            ),
            MediaStream(1, StreamKind.Audio, "truehd", language = "eng", title = "Dolby Atmos", channels = 8, sampleRate = 48_000, default = true),
            MediaStream(2, StreamKind.Audio, "ac3", language = "fre", channels = 6),
            MediaStream(3, StreamKind.Subtitle, "hdmv_pgs_subtitle", language = "eng", textBased = false, forced = true),
        ),
    )

    @Test
    fun `each version lists its file and every stream, and nothing takes focus`() {
        compose.setContent { TvTheme { TvMediaInfo(listOf(FileInfo(1, null, emptyList(), emptyList(), file = file))) } }
        compose.onNodeWithTag(TAG_MEDIA_INFO).assertExists()
        compose.onNodeWithText("Film (2024).mkv").assertExists()
        compose.onNodeWithText("3840×1608 (4K)").assertExists()
        compose.onNodeWithText("Main 10 · Level 5.1").assertExists()
        compose.onNodeWithText("Profile 7 · Level 6 · Blu-ray HDR10-compatible base layer").assertExists()
        compose.onNodeWithText("PQ (SMPTE ST 2084)").assertExists()
        compose.onNodeWithText("Audio 2").assertExists()
        compose.onNodeWithText("Dolby Atmos").assertExists()
        compose.onNodeWithText("PGS, picture").assertExists()
        compose.onNodeWithText("Forced").assertExists()
        assertEquals(0, compose.onAllNodes(isFocusable()).fetchSemanticsNodes().size)
        assertEquals(2, compose.onAllNodesWithText("English").fetchSemanticsNodes().size)
    }
}
