package app.pley.android.auth

import kotlinx.coroutines.flow.Flow

/**
 * A code for signing in a TV (RFC 8628): shown as [userCode] with
 * [verificationUri] and a QR code of [verificationUriComplete], and
 * approved on a phone or computer at the server's /link page.
 */
data class DeviceCode(
    val server: String,
    /** What the TV polls with. Never shown. */
    val deviceCode: String,
    /** As shown: `BCDF-GHJK`. */
    val userCode: String,
    /** `http://192.168.1.20:7539/link`, the address the TV used, with its port. */
    val verificationUri: String,
    /** The same with the code in it, for the QR code. */
    val verificationUriComplete: String?,
    /** When the code stops working, in milliseconds since the epoch. */
    val expiresAt: Long,
    /** How often to ask, in seconds; the server's own interval. */
    val intervalSeconds: Int,
) {
    // Never print the device code.
    override fun toString(): String = "DeviceCode(server=$server, userCode=$userCode)"
}

/** Where a code stands. */
sealed interface DevicePoll {
    /** Nobody answered yet. */
    data object Waiting : DevicePoll

    /** Signed in, as the profile the approver picked. */
    data class SignedIn(val issuer: String) : DevicePoll

    /** The approver said no. */
    data object Denied : DevicePoll

    /** The code ran out: get a new one. */
    data object Expired : DevicePoll

    data class Failed(val message: String) : DevicePoll
}

/**
 * Signing in without a browser, with the device authorization grant: the
 * TV's only way in (TVs may not open a browser), and the phone's when it
 * has none. Only viewer tokens come of it, whoever approves.
 */
interface DeviceSignIn {
    /** Asks [server] for a code to show. */
    suspend fun start(server: String): DeviceCode

    /**
     * Asks whether [code] was approved, every interval while collected
     * (slower when the server says so), until it is signed in, denied or
     * expired.
     */
    fun poll(code: DeviceCode): Flow<DevicePoll>
}
