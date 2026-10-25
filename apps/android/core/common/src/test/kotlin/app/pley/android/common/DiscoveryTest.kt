package app.pley.android.common

import java.net.Inet4Address
import java.net.InetAddress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DiscoveryTest {
    private fun answer(json: String, from: String): FoundServer? {
        val bytes = json.toByteArray()
        return Discovery.readAnswer(bytes, bytes.size, InetAddress.getByName(from))
    }

    @Test
    fun answersBecomeAddresses() {
        val json = """{"pley":1,"id":"ab","name":"Den","version":"0.1.0","port":7539}"""
        assertEquals(FoundServer("http://192.168.1.20:7539", "Den", "ab", "0.1.0"), answer(json, "192.168.1.20"))
        assertEquals("http://[fd00:0:0:0:0:0:0:5]:7539", answer(json, "fd00::5")?.url)
        assertNull(answer("hello", "192.168.1.20"))
        assertNull(answer("""{"pley":2,"id":"ab","name":"Den","version":"9","port":7539}""", "192.168.1.20"))
        assertNull(answer("""{"pley":1,"id":"ab","name":"Den","version":"0.1.0","port":0}""", "192.168.1.20"))
    }

    @Test
    fun networksHaveBroadcastAddresses() {
        fun broadcast(ip: String, prefix: Int) =
            Discovery.broadcastOf(InetAddress.getByName(ip) as Inet4Address, prefix).hostAddress
        assertEquals("192.168.1.255", broadcast("192.168.1.20", 24))
        assertEquals("10.0.255.255", broadcast("10.0.3.4", 16))
        assertEquals("172.16.7.255", broadcast("172.16.6.1", 23))
        assertEquals("255.255.255.255", broadcast("10.0.0.1", 0))
    }
}
