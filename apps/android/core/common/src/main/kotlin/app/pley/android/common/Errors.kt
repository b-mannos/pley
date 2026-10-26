package app.pley.android.common

/**
 * An error whose message is already written for people: the server's own
 * sentence, or one of the app's. [describe] shows it as it is.
 */
interface UserFacing

/**
 * What a stub of the native app answers: that part isn't built yet. It
 * says so in the app rather than failing quietly.
 */
class NotBuiltYet(what: String) : UnsupportedOperationException("$what isn't built yet"), UserFacing

/** Why [error] happened, as a sentence for people: "No answer in time." */
fun sentenceOf(error: Throwable): String = asSentence(describe(error))

/** [text] as a sentence: a capital first, a full stop last (the app's own texts come in lower case, without one). */
fun asSentence(text: String): String {
    val capital = text.trim().replaceFirstChar { it.uppercase() }
    return if (capital.isEmpty() || capital.last() in ".…!?") capital else "$capital."
}

/** Why a request failed, in a few words for people. */
fun describe(error: Throwable): String = when (error) {
    is java.net.ConnectException, is java.net.NoRouteToHostException -> "nothing answers there"
    is java.net.SocketTimeoutException -> "no answer in time"
    is java.net.UnknownHostException -> "that address isn't known on this network"
    is javax.net.ssl.SSLException -> "its secure connection failed"
    is UserFacing -> error.message ?: error.javaClass.simpleName
    is org.json.JSONException -> "that isn't a pley server"
    else -> error.message ?: error.javaClass.simpleName
}
