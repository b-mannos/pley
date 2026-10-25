package app.pley.android.data

/**
 * Something the screens show, as far as pley knows it. A repository's flow
 * gives what was saved first (Ready, not fresh), then what the server
 * says, so a screen draws at once and never waits on the network for what
 * it showed before.
 */
sealed interface Loadable<out T> {
    /** Nothing to show yet. */
    data object Loading : Loadable<Nothing>

    /** [value], from the server when [fresh], else from what was saved. */
    data class Ready<T>(val value: T, val fresh: Boolean) : Loadable<T>

    /** It failed: [error] (its message is for people), with what was saved, if anything. */
    data class Failed<T>(val error: Throwable, val stale: T? = null) : Loadable<T>
}

/** The value to show, fresh or saved; `null` while there is none. */
val <T> Loadable<T>.valueOrNull: T?
    get() = when (this) {
        is Loadable.Ready -> value
        is Loadable.Failed -> stale
        Loadable.Loading -> null
    }
