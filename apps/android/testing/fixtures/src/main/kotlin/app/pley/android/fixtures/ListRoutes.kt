package app.pley.android.fixtures

import app.pley.android.model.ItemKind
import app.pley.android.model.ItemList
import app.pley.android.model.ServerEvent
import mockwebserver3.MockResponse

/**
 * My List and leaving Continue Watching (docs/API.md): GET, PUT and DELETE
 * /api/watchlist, and DELETE /api/continue-watching/{id}. My List changes
 * tell the profile's event streams (`watchlist_changed`).
 */
internal class ListRoutes(private val s: State) {
    private val catalog get() = s.catalog
    private val views get() = s.views

    fun routes(): List<Route> = listOf(
        route("GET", "/api/watchlist", LIBRARY) { _, c, _ ->
            Reply.json(ItemList.serializer(), ItemList(views.listed(c.profile).map { views.summary(it, c.profile) }))
        },
        route("PUT", "/api/watchlist/*", PLAY) { _, c, v -> list(c, v[0], add = true) },
        route("DELETE", "/api/watchlist/*", PLAY) { _, c, v -> list(c, v[0], add = false) },
        route("DELETE", "/api/continue-watching/*", PLAY) { _, c, v -> hide(c, v[0]) },
    )

    /** Adds (again is fine) or removes a movie or show. */
    private fun list(caller: Caller, id: String, add: Boolean): MockResponse {
        val item = id.toLongOrNull()?.let(catalog::item) ?: return Reply.notFound("no such item")
        if (item.kind != ItemKind.Movie && item.kind != ItemKind.Show) return Reply.badRequest("My List takes movies and shows")
        val profile = caller.profile
        val changed = if (add) {
            if (item.id in profile.watchlist) {
                false
            } else {
                profile.watchlist[item.id] = Listed(item.id, s.now(), watchedThen = views.state(item, profile).watched)
                true
            }
        } else {
            profile.watchlist.remove(item.id) != null
        }
        if (changed) {
            profile.version++
            s.events.emitTo(profile.id, ServerEvent.WatchlistChanged)
        }
        return Reply.noContent()
    }

    /** Takes a movie, or an episode's whole show, out of Continue Watching until it is played again. */
    private fun hide(caller: Caller, id: String): MockResponse {
        val item = id.toLongOrNull()?.let(catalog::item) ?: return Reply.notFound("no such item")
        val owner = when (item.kind) {
            ItemKind.Movie, ItemKind.Show -> item.id
            ItemKind.Episode -> item.grandparentId ?: item.id
            else -> return Reply.badRequest("only movies and episodes are in Continue Watching")
        }
        caller.profile.hiddenFromContinue[owner] = s.now()
        caller.profile.version++
        return Reply.noContent()
    }
}
