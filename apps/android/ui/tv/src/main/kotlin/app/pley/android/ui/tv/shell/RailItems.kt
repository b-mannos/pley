package app.pley.android.ui.tv.shell

import androidx.compose.runtime.Immutable
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.Library
import app.pley.android.model.LibraryKind
import app.pley.android.screens.PleyKey

/** The kinds of place the rail goes to. */
enum class RailSlot { Search, Home, Library, Libraries, Playlists, Collections, Settings }

/** One rail entry: where it goes, and for a library its name and kind. */
@Immutable
data class RailItem(
    /** Stable across library changes: "home", "library:3", "settings". */
    val id: String,
    val slot: RailSlot,
    val key: PleyKey,
    /** A library's name, as the server has it; the other slots have their own strings. */
    val name: String? = null,
    val libraryKind: LibraryKind? = null,
) {
    val icon: PleyIcon
        get() = when (slot) {
            RailSlot.Search -> PleyIcons.Search
            RailSlot.Home -> PleyIcons.House
            RailSlot.Library -> when (libraryKind) {
                LibraryKind.Movies -> PleyIcons.Clapperboard
                LibraryKind.Shows -> PleyIcons.Tv
                LibraryKind.Music -> PleyIcons.Music
                else -> PleyIcons.Film
            }
            RailSlot.Libraries -> PleyIcons.Layers
            RailSlot.Playlists -> PleyIcons.ListMusic
            RailSlot.Collections -> PleyIcons.Layers
            RailSlot.Settings -> PleyIcons.Settings
        }
}

/**
 * The rail's entries above Settings (design spec §2.1): Search, Home, the
 * first [MAX_LIBRARIES] libraries in the server's order, Playlists and
 * Collections. Between the avatar and Settings there is room for seven
 * entries, so with more libraries a "Libraries" entry, which lists them
 * all, takes Collections' place (collections stay on each library's
 * Collections tab).
 */
fun railItems(libraries: List<Library>): List<RailItem> = buildList {
    add(RailItem("search", RailSlot.Search, PleyKey.Search()))
    add(RailItem("home", RailSlot.Home, PleyKey.Home))
    libraries.take(MAX_LIBRARIES).forEach { library ->
        add(RailItem("library:${library.id}", RailSlot.Library, PleyKey.Library(library.id), library.name, library.kind))
    }
    if (libraries.size > MAX_LIBRARIES) add(RailItem("libraries", RailSlot.Libraries, PleyKey.Libraries))
    add(RailItem("playlists", RailSlot.Playlists, PleyKey.Playlists))
    if (libraries.size <= MAX_LIBRARIES) {
        // A library's collections; the first video library's, as the web's Collections link.
        val first = libraries.firstOrNull { it.kind != LibraryKind.Music } ?: libraries.firstOrNull()
        if (first != null) add(RailItem("collections", RailSlot.Collections, PleyKey.Collections(first.id)))
    }
}

/** Settings, pinned at the rail's foot. */
val SettingsItem = RailItem("settings", RailSlot.Settings, PleyKey.Settings)

/** The avatar's id: it opens Who's watching?. */
const val PROFILE_ITEM = "profile"

/**
 * Which entry is current for a back stack whose first page is [root]: the
 * entry that opened it, "libraries" for a library not in the rail, or
 * `null` (Who's watching?, sign-in).
 */
fun railCurrent(root: PleyKey, items: List<RailItem>): String? = when (root) {
    is PleyKey.Library -> items.firstOrNull { it.key == PleyKey.Library(root.libraryId) }?.id
        ?: items.firstOrNull { it.slot == RailSlot.Libraries }?.id
    is PleyKey.Collections -> items.firstOrNull { it.slot == RailSlot.Collections }?.id
        ?: railCurrent(PleyKey.Library(root.libraryId), items)
    is PleyKey.Search -> "search"
    PleyKey.Settings -> SettingsItem.id
    PleyKey.Profiles -> PROFILE_ITEM
    else -> items.firstOrNull { it.key == root }?.id
}

/** At most this many libraries in the rail (design spec §2.1). */
const val MAX_LIBRARIES = 3
