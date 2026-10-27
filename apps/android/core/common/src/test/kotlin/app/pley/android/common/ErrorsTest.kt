package app.pley.android.common

import java.io.EOFException
import java.io.IOException
import java.net.ConnectException
import java.net.SocketTimeoutException
import org.junit.Assert.assertEquals
import org.junit.Test

/** What people read of a failure: the app's words, never OkHttp's. */
class ErrorsTest {
    @Test
    fun aKnownFailureHasItsOwnWords() {
        assertEquals("nothing answers there", describe(ConnectException("Failed to connect to /192.168.1.20:7539")))
        assertEquals("no answer in time", describe(SocketTimeoutException("timeout")))
        assertEquals("No answer in time.", sentenceOf(SocketTimeoutException("timeout")))
    }

    @Test
    fun anyOtherNetworkFailureIsOneSentence() {
        assertEquals(CONNECTION_FAILED, describe(IOException("unexpected end of stream on http://192.168.1.20:7539/...")))
        assertEquals(CONNECTION_FAILED, describe(EOFException("\\n not found: limit=0 content=…")))
        assertEquals("The connection to the server failed.", sentenceOf(IOException("Canceled")))
    }

    @Test
    fun wordsWrittenForPeopleStayAsTheyAre() {
        val written = object : IOException("you were signed out: sign in again"), UserFacing {}
        assertEquals("you were signed out: sign in again", describe(written))
        assertEquals("downloads isn't built yet", describe(NotBuiltYet("downloads")))
        assertEquals("not signed in", describe(IllegalStateException("not signed in")))
    }
}
