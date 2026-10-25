package app.pley.android.fixtures

import app.pley.android.model.Added
import app.pley.android.model.EntryIds
import app.pley.android.model.ItemDetail
import app.pley.android.model.ItemIds
import app.pley.android.model.NewPlaylist
import app.pley.android.model.PlaylistDetail
import app.pley.android.model.PlaylistKind
import app.pley.android.model.PlaylistList
import app.pley.android.model.PlaylistPatch
import mockwebserver3.MockResponse

/**
 * The profile's playlists (docs/API.md, "Playlists"): listed and read with
 * `library`, made and changed with `playlists`. Another profile's answer
 * 404; a smart one's entries can't be added, removed or reordered.
 */
internal class PlaylistRoutes(private val s: State) {
    private val catalog get() = s.catalog
    private val views get() = s.views

    fun routes(): List<Route> = listOf(
        route("GET", "/api/playlists", LIBRARY) { _, c, _ ->
            val list = s.household.playlists.values.filter { it.owner == c.profile.id }
                .map { views.playlistDetail(it, c.profile).toSummary() }
                .sortedBy { it.title.lowercase() }
            Reply.json(PlaylistList.serializer(), PlaylistList(list))
        },
        route("GET", "/api/playlists/*", LIBRARY) { _, c, v ->
            val playlist = own(v[0], c) ?: return@route Reply.notFound("no such playlist")
            detail(playlist, c)
        },
        route("POST", "/api/playlists", PLAYLISTS) { call, c, _ -> create(call, c) },
        route("PATCH", "/api/playlists/*", PLAYLISTS) { call, c, v ->
            val playlist = own(v[0], c) ?: return@route Reply.notFound("no such playlist")
            val patch = call.body(PlaylistPatch.serializer()) ?: return@route Reply.badRequest("the body isn't a playlist change")
            if (patch.smartFilter != null && playlist.smartFilter == null) return@route Reply.badRequest("only smart playlists have a filter")
            patch.title?.let { title ->
                if (title.isBlank()) return@route Reply.badRequest("a playlist needs a title")
                playlist.title = title.trim()
            }
            patch.smartFilter?.let { playlist.smartFilter = it }
            playlist.updatedAt = s.now()
            detail(playlist, c)
        },
        route("DELETE", "/api/playlists/*", PLAYLISTS) { _, c, v ->
            val playlist = own(v[0], c) ?: return@route Reply.notFound("no such playlist")
            s.household.playlists.remove(playlist.id)
            Reply.noContent()
        },
        route("POST", "/api/playlists/*/items", PLAYLISTS) { call, c, v ->
            val playlist = own(v[0], c) ?: return@route Reply.notFound("no such playlist")
            if (playlist.smartFilter != null) return@route Reply.badRequest("a smart playlist's entries come from its filter")
            val ids = call.body(ItemIds.serializer()) ?: return@route Reply.badRequest("the body isn't item_ids")
            val leaves = expand(ids.itemIds) ?: return@route Reply.notFound("no such item")
            if (leaves.any { !views.fits(playlist.kind, it.second) }) return@route Reply.badRequest("that doesn't fit a ${playlist.kind.wire} playlist")
            val room = (5_000 - playlist.entries.size).coerceAtLeast(0)
            val added = leaves.take(room)
            added.forEach { playlist.entries += s.household.newEntry() to it.first }
            playlist.updatedAt = s.now()
            Reply.json(Added.serializer(), Added(added.size))
        },
        route("DELETE", "/api/playlists/*/items/*", PLAYLISTS) { _, c, v ->
            val playlist = own(v[0], c) ?: return@route Reply.notFound("no such playlist")
            if (playlist.smartFilter != null) return@route Reply.badRequest("a smart playlist's entries come from its filter")
            val entry = v[1].toLongOrNull()
            if (!playlist.entries.removeAll { it.first == entry }) return@route Reply.notFound("no such entry")
            playlist.updatedAt = s.now()
            Reply.noContent()
        },
        route("PUT", "/api/playlists/*/order", PLAYLISTS) { call, c, v ->
            val playlist = own(v[0], c) ?: return@route Reply.notFound("no such playlist")
            if (playlist.smartFilter != null) return@route Reply.badRequest("a smart playlist's order is its filter's")
            val order = call.body(EntryIds.serializer()) ?: return@route Reply.badRequest("the body isn't entry_ids")
            val first = order.entryIds.mapNotNull { id -> playlist.entries.firstOrNull { it.first == id } }.distinct()
            val rest = playlist.entries.filter { it !in first }
            playlist.entries.clear()
            playlist.entries += first + rest
            playlist.updatedAt = s.now()
            detail(playlist, c)
        },
    )

    private fun own(id: String, caller: Caller): Playlist? =
        id.toLongOrNull()?.let { s.household.playlists[it] }?.takeIf { it.owner == caller.profile.id }

    private fun detail(playlist: Playlist, caller: Caller, code: Int = 200): MockResponse =
        Reply.json(PlaylistDetail.serializer(), views.playlistDetail(playlist, caller.profile), code)

    /** Items as playlist entries: shows, seasons, albums and artists as their episodes or tracks. `null` for an unknown id. */
    private fun expand(ids: List<Long>): List<Pair<Long, ItemDetail>>? = ids.flatMap { id ->
        catalog.item(id) ?: return null
        catalog.leaves(id).mapNotNull { leaf -> catalog.item(leaf)?.let { leaf to it } }
    }

    /** POST /api/playlists: of items (the first decides the kind when none is given), or smart. */
    private fun create(call: Call, caller: Caller): MockResponse {
        val request = call.body(NewPlaylist.serializer()) ?: return Reply.badRequest("the body isn't a playlist")
        val title = request.title.trim().ifEmpty { return Reply.badRequest("a playlist needs a title") }
        val now = s.now()
        if (request.smartFilter != null) {
            val library = request.libraryId?.takeIf { id -> catalog.libraries.any { it.id == id } }
                ?: return Reply.badRequest("a smart playlist needs its library")
            val kind = if (library == FixtureIds.MUSIC) PlaylistKind.Audio else PlaylistKind.Video
            val playlist = s.household.newPlaylist(caller.profile.id, title, kind, request.smartFilter, library, now)
            return detail(playlist, caller, 201)
        }
        val leaves = expand(request.itemIds) ?: return Reply.notFound("no such item")
        val kind = request.kind?.takeIf { it != PlaylistKind.Unknown }
            ?: leaves.firstOrNull()?.let { if (views.fits(PlaylistKind.Audio, it.second)) PlaylistKind.Audio else PlaylistKind.Video }
            ?: PlaylistKind.Video
        if (leaves.any { !views.fits(kind, it.second) }) return Reply.badRequest("that doesn't fit a ${kind.wire} playlist")
        val playlist = s.household.newPlaylist(caller.profile.id, title, kind, null, null, now)
        leaves.take(5_000).forEach { playlist.entries += s.household.newEntry() to it.first }
        return detail(playlist, caller, 201)
    }
}
