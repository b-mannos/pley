package app.pley.android.ui.phone.livetv

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.LiveCategory
import app.pley.android.model.LiveGroups
import app.pley.android.screens.livetv.GroupEntry
import app.pley.android.screens.livetv.LiveTv
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.kit.PleySheet

/**
 * The categories, in a sheet (the web's CategoryRail, a picker on
 * phones): Favourites, Recently watched and All channels with their
 * counts, then the provider's groups, those of a region gathered under it
 * (a tap opens it), found by name when there are many.
 */
@Composable
internal fun CategorySheet(
    groups: LiveGroups?,
    selected: LiveCategory,
    onSelect: (LiveCategory) -> Unit,
    onDismiss: () -> Unit,
) {
    var search by rememberSaveable { mutableStateOf("") }
    val all = groups?.groups.orEmpty()
    val tree = remember(all) { LiveTv.groupTree(all) }
    val selectedRegion = (selected as? LiveCategory.Group)?.let { s -> all.firstOrNull { it.name == s.name }?.region }
    var open by remember { mutableStateOf(setOfNotNull(selectedRegion)) }
    val q = search.trim().lowercase()
    PleySheet(onDismiss = onDismiss, title = stringResource(R.string.livetv_categories), modifier = Modifier.testTag(LIVE_TV_CATEGORIES)) {
        LazyColumn(Modifier.fillMaxWidth().heightIn(max = 560.dp), contentPadding = PaddingValues(bottom = 16.dp)) {
            item(key = "favorites") {
                Entry(PleyIcons.Star, stringResource(R.string.livetv_category_favorites), groups?.favorites, selected == LiveCategory.Favorites) {
                    onSelect(LiveCategory.Favorites)
                }
            }
            item(key = "recent") {
                Entry(PleyIcons.History, stringResource(R.string.livetv_category_recent), groups?.recent, selected == LiveCategory.Recent) {
                    onSelect(LiveCategory.Recent)
                }
            }
            item(key = "all") {
                Entry(PleyIcons.LayoutGrid, stringResource(R.string.livetv_category_all), groups?.total, selected == LiveCategory.All) {
                    onSelect(LiveCategory.All)
                }
            }
            if (all.size > 1) {
                item(key = "groups") {
                    Row(Modifier.fillMaxWidth().padding(start = 24.dp, end = 24.dp, top = 16.dp, bottom = 4.dp)) {
                        Text(stringResource(R.string.livetv_groups).uppercase(), style = MaterialTheme.typography.labelSmall, color = PleyColors.Subtle, modifier = Modifier.weight(1f))
                        Text("%,d".format(all.size), style = MaterialTheme.typography.labelSmall, color = PleyColors.Subtle)
                    }
                }
                if (all.size > FIND_FROM) {
                    item(key = "find") { FindGroup(search) { search = it } }
                }
                if (q.isNotEmpty()) {
                    val found = all.filter { it.name.lowercase().contains(q) }
                    if (found.isEmpty()) {
                        item(key = "none") {
                            Text(
                                stringResource(R.string.livetv_no_group, search.trim()),
                                style = MaterialTheme.typography.bodyMedium,
                                color = PleyColors.Subtle,
                                modifier = Modifier.padding(horizontal = 24.dp, vertical = 16.dp),
                            )
                        }
                    }
                    items(found, key = { "found:${it.name}" }) { g ->
                        Entry(null, LiveTv.groupName(g, g.name), g.count, selected == LiveCategory.Group(g.name), indent = GROUP_INSET) { onSelect(LiveCategory.Group(g.name)) }
                    }
                } else {
                    for (entry in tree) {
                        when (entry) {
                            is GroupEntry.Single -> item(key = "group:${entry.group.name}") {
                                val category = LiveCategory.Group(entry.group.name)
                                Entry(null, entry.group.name, entry.group.count, selected == category, indent = GROUP_INSET) { onSelect(category) }
                            }
                            is GroupEntry.Region -> {
                                val expanded = entry.region in open
                                item(key = "region:${entry.region}") {
                                    Entry(
                                        if (expanded) PleyIcons.ChevronDown else PleyIcons.ChevronRight,
                                        entry.region,
                                        entry.count,
                                        selected = false,
                                    ) { open = if (expanded) open - entry.region else open + entry.region }
                                }
                                if (expanded) {
                                    items(entry.groups, key = { "group:${it.name}" }) { g ->
                                        val category = LiveCategory.Group(g.name)
                                        Entry(null, g.label, g.count, selected == category, indent = GROUP_INSET + 20.dp) { onSelect(category) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/** One category: its icon (none for a group), its name and count; the chosen one in the accent. */
@Composable
private fun Entry(icon: PleyIcon?, name: String, count: Int?, selected: Boolean, indent: Dp = 24.dp, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .background(if (selected) PleyColors.PrimaryContainer else PleyColors.Surface2)
            .clickable(role = Role.Button, onClick = onClick)
            .padding(start = indent, end = 24.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            Icon(icon, contentDescription = null, size = 18.dp, tint = if (selected) PleyColors.AccentBright else PleyColors.Muted)
            Spacer(Modifier.width(14.dp))
        }
        Text(
            name,
            style = MaterialTheme.typography.bodyLarge,
            color = if (selected) PleyColors.AccentBright else PleyColors.Fg,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (count != null) Text("%,d".format(count), style = MaterialTheme.typography.bodySmall, color = PleyColors.Subtle)
    }
}

/** Finds a group by name. */
@Composable
private fun FindGroup(value: String, onValue: (String) -> Unit) {
    val label = stringResource(R.string.livetv_find_group)
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 6.dp)
            .height(40.dp)
            .clip(PleyShape.Pill)
            .background(PleyColors.Surface3)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(PleyIcons.Search, contentDescription = null, size = 16.dp, tint = PleyColors.Muted)
        Spacer(Modifier.width(8.dp))
        Box(Modifier.weight(1f), contentAlignment = Alignment.CenterStart) {
            if (value.isEmpty()) Text(label, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted)
            BasicTextField(
                value = value,
                onValueChange = onValue,
                singleLine = true,
                textStyle = MaterialTheme.typography.bodyMedium.copy(color = PleyColors.Fg),
                cursorBrush = SolidColor(PleyColors.AccentBright),
                modifier = Modifier.fillMaxWidth().semantics { contentDescription = label },
            )
        }
    }
}

/** A group's name starts where the categories' names do, after their icons. */
private val GROUP_INSET = 56.dp

/** With more groups than this, a field finds one by name. */
private const val FIND_FROM = 12

internal const val LIVE_TV_CATEGORIES = "pley.live.categories"
