package app.pley.android.testing

import app.pley.android.model.PleyJson
import java.io.File
import java.net.JarURLConnection
import java.net.URL
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.serializer

/**
 * The server's answers and the app's request bodies as JSON files, for
 * tests: under `fixtures/` on the classpath (this module's
 * src/main/resources/fixtures). They are written by hand from docs/API.md
 * and the server's structs, never captured from a server, and keep to one
 * small household: libraries 1 (Movies), 2 (TV Shows) and 3 (Music);
 * Arrival is 101 with its trailer 102, Severance 201 with season 202 and
 * episodes 203 to 205; profiles 2 (Alex, whom the app acts as), 3, 4 and 1.
 *
 * Paths are relative to `fixtures/`: `"items/movie_detail.json"`.
 */
object Fixtures {
    const val ROOT: String = "fixtures"

    /** The fixture's text. Fails with the path it looked for when there's no such file. */
    fun text(path: String): String {
        val name = "$ROOT/${path.trimStart('/')}"
        val stream = loader().getResourceAsStream(name)
            ?: throw IllegalArgumentException(
                "no fixture \"$path\": looked for $name on the classpath (core/testing/src/main/resources/$name)",
            )
        return stream.use { it.readBytes().decodeToString() }
    }

    /** The fixture read with PleyJson, as the app reads the server's answers. */
    inline fun <reified T> decode(path: String): T = decode(path, serializer<T>())

    fun <T> decode(path: String, deserializer: DeserializationStrategy<T>): T =
        PleyJson.decodeFromString(deserializer, text(path))

    /** Every fixture's path, sorted: from a folder while tests run here, or from the jar elsewhere. */
    fun all(): List<String> {
        val found = sortedSetOf<String>()
        for (url in loader().getResources(ROOT)) {
            when (url.protocol) {
                "file" -> found += filesUnder(url)
                "jar" -> found += entriesUnder(url)
            }
        }
        return found.toList()
    }

    private fun filesUnder(url: URL): List<String> {
        val root = File(url.toURI())
        return root.walkTopDown()
            .filter { it.isFile && it.name.endsWith(".json") }
            .map { it.relativeTo(root).invariantSeparatorsPath }
            .toList()
    }

    private fun entriesUnder(url: URL): List<String> {
        val connection = url.openConnection() as JarURLConnection
        connection.useCaches = false
        return connection.jarFile.use { jar ->
            jar.entries().asSequence()
                .map { it.name }
                .filter { it.startsWith("$ROOT/") && it.endsWith(".json") }
                .map { it.removePrefix("$ROOT/") }
                .toList()
        }
    }

    private fun loader(): ClassLoader = Fixtures::class.java.classLoader ?: ClassLoader.getSystemClassLoader()
}
