package app.pley.android.fixtures

import java.util.concurrent.Callable

/**
 * What debug builds offer as "Use demo server": a FixtureServer in the
 * app's own process. Release builds don't have this module, so the app
 * finds it with ServiceLoader (META-INF/services) rather than by name, and
 * finds nothing in a release.
 *
 * It is a [Callable] of the server's address, so code that can't see this
 * module (the app's main source set) can use it all the same:
 *
 * ```
 * val type = runCatching { Class.forName("app.pley.android.fixtures.DemoServerFactory") }.getOrNull()
 * val demo = type?.let { ServiceLoader.load(it).firstOrNull() as? Callable<*> }
 * val address = demo?.call() as String? // off the main thread
 * ```
 */
interface DemoServerFactory : Callable<String> {
    /** Starts the demo server, once per process, and gives its address (`http://127.0.0.1:<port>`). */
    fun start(): String

    override fun call(): String = start()
}

/** The demo server: the FixtureServer with its defaults, signing in for real. */
class FixtureDemoServerFactory : DemoServerFactory {
    override fun start(): String = synchronized(FixtureDemoServerFactory::class.java) {
        val running = server ?: FixtureServer().also {
            it.start()
            server = it
        }
        running.url
    }

    private companion object {
        var server: FixtureServer? = null
    }
}
