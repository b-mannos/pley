package app.pley.android.data

import java.io.File
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class FileSnapshotStoreTest {
    @get:Rule
    val folder = TemporaryFolder()

    private val alex = ProfileScope("http://192.168.1.20:7539", 2)
    private val sam = ProfileScope("http://192.168.1.20:7539", 3)
    private val elsewhere = ProfileScope("https://pley.example.com", 2)

    private fun TestScope.store(root: File = File(folder.root, "snapshots"), max: Long = FileSnapshotStore.MAX_BYTES) =
        FileSnapshotStore({ root }, StandardTestDispatcher(testScheduler), max)

    @Test
    fun `a snapshot reads back as written, per profile`() = runTest {
        val store = store()
        store.write(alex, "home", "alex".toByteArray())
        store.write(sam, "home", "sam".toByteArray())

        assertArrayEquals("alex".toByteArray(), store.read(alex, "home"))
        assertArrayEquals("sam".toByteArray(), store.read(sam, "home"))
        assertNull(store.read(elsewhere, "home"))
        assertNull(store.read(alex, "libraries"))
    }

    @Test
    fun `files live under the server's hash and the profile's id`() = runTest {
        val root = File(folder.root, "snapshots")
        store(root).write(alex, "library:1:title.asc:head", "x".toByteArray())

        val dir = File(root, "${ProfileScope.serverKey(alex.issuer)}/2")
        assertEquals(listOf(FileSnapshotStore.fileName("library:1:title.asc:head")), dir.list()!!.toList())
        assertEquals(40, ProfileScope.serverKey(alex.issuer).length)
    }

    @Test
    fun `writing replaces the whole file and leaves no temporary file`() = runTest {
        val root = File(folder.root, "snapshots")
        val store = store(root)
        store.write(alex, "home", ByteArray(5000) { 1 })
        store.write(alex, "home", "short".toByteArray())

        assertArrayEquals("short".toByteArray(), store.read(alex, "home"))
        val names = root.walkTopDown().filter { it.isFile }.map { it.name }.toList()
        assertTrue(names.none { it.endsWith(".tmp") })
    }

    @Test
    fun `keys become safe, distinct file names`() {
        val keys = listOf(
            "home",
            "library:1:title.asc:head",
            "library:1:title.asc.g=Science Fiction:head",
            "library:1:title.asc.g=Comédie:head",
            "library:1:title.asc.g=Science%20Fiction:head",
            "search:a/b",
            "../escape",
        )
        val names = keys.map(FileSnapshotStore::fileName)
        assertEquals(keys.size, names.toSet().size)
        for (name in names) assertTrue(name, name.all { it.isLetterOrDigit() || it in "-_.~" })
        assertFalse(names.any { it.contains("/") || it == ".." })
        val long = FileSnapshotStore.fileName("library:1:" + "x".repeat(500))
        assertTrue(long.length <= 120)
        assertNotEquals(long, FileSnapshotStore.fileName("library:1:" + "x".repeat(501)))
    }

    @Test
    fun `the least recently used snapshots go when the store is full`() = runTest {
        val store = store(max = 300)
        store.write(alex, "a", ByteArray(100))
        store.write(alex, "b", ByteArray(100))
        store.write(alex, "c", ByteArray(100))
        // Read: now the most recently used.
        store.read(alex, "a")
        store.write(alex, "d", ByteArray(100))

        assertTrue(store.read(alex, "a") != null)
        assertNull(store.read(alex, "b"))
        assertTrue(store.read(alex, "c") != null)
        assertTrue(store.read(alex, "d") != null)
        assertTrue(store.size() <= 300)
    }

    @Test
    fun `the size limit holds across restarts`() = runTest {
        val root = File(folder.root, "snapshots")
        val first = store(root, max = 250)
        first.write(alex, "a", ByteArray(100))
        first.write(alex, "b", ByteArray(100))

        val second = store(root, max = 250)
        assertEquals(200, second.size())
        second.write(alex, "c", ByteArray(100))
        assertTrue(second.size() <= 250)
        assertTrue(second.read(alex, "c") != null)
    }

    @Test
    fun `the active profile's Home is the startup copy`() = runTest {
        val root = File(folder.root, "snapshots")
        val store = store(root)
        val startup = File(root, "startup/home.json")
        store.write(alex, SnapshotKeys.HOME, "alex home".toByteArray())
        store.write(sam, SnapshotKeys.HOME, "sam home".toByteArray())
        assertFalse(startup.exists())

        store.activate(alex)
        assertEquals("alex home", startup.readText())

        // Only the active profile's Home is copied.
        store.write(sam, SnapshotKeys.HOME, "sam again".toByteArray())
        assertEquals("alex home", startup.readText())
        store.write(alex, SnapshotKeys.HOME, "alex again".toByteArray())
        assertEquals("alex again", startup.readText())

        store.activate(sam)
        assertEquals("sam again", startup.readText())

        store.activate(null)
        assertFalse(startup.exists())
    }

    @Test
    fun `the startup copy is StartupRead's file`() {
        assertEquals("snapshots/startup/home.json", StartupFiles.HOME)
    }

    @Test
    fun `clearing a server deletes its snapshots and startup copy only`() = runTest {
        val root = File(folder.root, "snapshots")
        val store = store(root)
        store.write(alex, SnapshotKeys.HOME, "alex".toByteArray())
        store.write(sam, "me", "sam".toByteArray())
        store.write(elsewhere, SnapshotKeys.HOME, "elsewhere".toByteArray())
        store.activate(alex)

        store.clear(alex.issuer)

        assertNull(store.read(alex, SnapshotKeys.HOME))
        assertNull(store.read(sam, "me"))
        assertArrayEquals("elsewhere".toByteArray(), store.read(elsewhere, SnapshotKeys.HOME))
        assertFalse(File(root, "startup/home.json").exists())
        assertNull(store.active)
        assertEquals("elsewhere".length.toLong(), store.size())
    }

    @Test
    fun `clearing another server keeps the startup copy`() = runTest {
        val root = File(folder.root, "snapshots")
        val store = store(root)
        store.write(alex, SnapshotKeys.HOME, "alex".toByteArray())
        store.activate(alex)

        store.clear(elsewhere.issuer)

        assertEquals("alex", File(root, "startup/home.json").readText())
    }

    @Test
    fun `a server keeps the snapshots of its last three profiles`() = runTest {
        val root = File(folder.root, "snapshots")
        val store = store(root)
        val profiles = (1L..5L).map { ProfileScope(alex.issuer, it) }
        for ((i, p) in profiles.withIndex()) {
            store.write(p, "me", "profile $i".toByteArray())
            store.activate(p)
            // Folders' times are whole seconds on some file systems.
            File(root, "${p.server}/${p.userId}").setLastModified(1_000_000L * (i + 1))
        }
        store.activate(profiles[4])

        val kept = File(root, profiles[0].server).list()!!.map { it.toLong() }.toSet()
        assertEquals(setOf(3L, 4L, 5L), kept)
        assertNull(store.read(profiles[0], "me"))
        assertTrue(store.read(profiles[4], "me") != null)
        // Another server's profiles aren't counted.
        store.write(elsewhere, "me", "x".toByteArray())
        store.activate(elsewhere)
        assertEquals(3, File(root, profiles[0].server).list()!!.size)
    }

    @Test
    fun `reads and writes without a scope act on the active profile`() = runTest {
        val store = store()
        store.write("home", "nobody".toByteArray())
        assertNull(store.read("home"))

        store.activate(alex)
        store.write("home", "alex".toByteArray())
        assertArrayEquals("alex".toByteArray(), store.read("home"))
        assertArrayEquals("alex".toByteArray(), store.read(alex, "home"))
    }
}
