package app.pley.android.common

import java.io.IOException
import java.net.SocketException
import java.net.SocketTimeoutException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalNetworkTest {
    @Test
    fun android17AndFireOs16AskFirst() {
        assertEquals(emptyList<String>(), LocalNetwork.permissions(sdk = 36, fireTv = false))
        assertEquals(emptyList<String>(), LocalNetwork.permissions(sdk = 30, fireTv = true))
        assertEquals(listOf(LocalNetwork.ACCESS_LOCAL_NETWORK), LocalNetwork.permissions(sdk = 37, fireTv = false))
        assertEquals(listOf(LocalNetwork.NEARBY_WIFI_DEVICES), LocalNetwork.permissions(sdk = 36, fireTv = true))
        assertEquals(
            listOf(LocalNetwork.ACCESS_LOCAL_NETWORK, LocalNetwork.NEARBY_WIFI_DEVICES),
            LocalNetwork.permissions(sdk = 37, fireTv = true),
        )
    }

    @Test
    fun aRefusalIsToldFromANetworkThatFails() {
        assertTrue(LocalNetwork.isPermissionBlock(SocketException("sendto failed: EPERM (Operation not permitted)")))
        assertTrue(LocalNetwork.isPermissionBlock(IOException("discovery", SocketException("socket failed: EACCES (Permission denied)"))))
        assertFalse(LocalNetwork.isPermissionBlock(SocketTimeoutException("connect timed out")))
        assertFalse(LocalNetwork.isPermissionBlock(SocketException("Network is unreachable")))
    }
}
