package app.pley.android.common

/**
 * An error whose message is already written for people: the server's own
 * sentence, or one of the app's. [describe] shows it as it is.
 */
interface UserFacing

/** Why a request failed, in a few words for people. */
fun describe(error: Throwable): String = when (error) {
    is java.net.ConnectException -> "nothing answers there"
    is java.net.SocketTimeoutException -> "no answer in time"
    is java.net.UnknownHostException -> "that address isn't known on this network"
    is javax.net.ssl.SSLException -> "its secure connection failed"
    is UserFacing -> error.message ?: error.javaClass.simpleName
    is org.json.JSONException -> "that isn't a pley server"
    else -> error.message ?: error.javaClass.simpleName
}
