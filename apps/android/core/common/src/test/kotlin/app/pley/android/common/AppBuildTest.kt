package app.pley.android.common

import org.junit.Assert.assertEquals
import org.junit.Test

class AppBuildTest {
    @Test
    fun theCommitGoesBesideTheVersion() {
        assertEquals("0.2.0 (1a2b3c4d5)", AppBuild.label("0.2.0", "1a2b3c4d5"))
    }

    @Test
    fun anUnknownCommitIsLeftOut() {
        assertEquals("0.2.0", AppBuild.label("0.2.0", "unknown"))
        assertEquals("0.2.0", AppBuild.label("0.2.0", ""))
        assertEquals("", AppBuild.label("", "1a2b3c4d5"))
    }
}
