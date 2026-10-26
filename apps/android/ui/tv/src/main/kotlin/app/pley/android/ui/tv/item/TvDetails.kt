package app.pley.android.ui.tv.item

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.PleyTv
import app.pley.android.designsystem.PleyType
import app.pley.android.model.CreditRole
import app.pley.android.screens.item.ItemPage
import app.pley.android.ui.tv.R

/**
 * Details at the end of the page (design spec §4.3): who made it and the
 * facts, one row per version in its long form ("4K · HEVC · Dolby Vision
 * P8.1 · TrueHD Atmos 7.1 · 62.0 GB", the one that plays ticked; Select
 * plays another), and the media info of each file. Only the version rows
 * take focus; the text is read by scrolling on with Down.
 */
@Composable
internal fun TvDetails(page: ItemPage, onChooseVersion: (Long) -> Unit, modifier: Modifier = Modifier) {
    val details = page.details
    Column(modifier.padding(start = PleyTv.Gutter, end = DetailsEnd)) {
        BasicText(stringResource(R.string.tv_item_details), style = PleyType.Tv.Section.copy(color = PleyColors.Fg))
        Spacer(Modifier.height(PleyTv.HeaderToTiles))
        val facts = buildList {
            details.crew.forEach { line ->
                val label = when (line.role) {
                    CreditRole.Creator -> stringResource(R.string.tv_item_created_by)
                    CreditRole.Director -> stringResource(if (line.plural) R.string.tv_item_directors else R.string.tv_item_director)
                    else -> stringResource(if (line.plural) R.string.tv_item_writers else R.string.tv_item_writer)
                }
                val more = if (line.more > 0) " " + pluralStringResource(R.plurals.tv_item_and_more, line.more, line.more) else ""
                add(label to line.people.joinToString(", ") { it.name } + more)
            }
            details.originalTitle?.let { add(stringResource(R.string.tv_item_original_title) to it) }
            details.released?.let { add(stringResource(R.string.tv_item_released) to it) }
            details.studio?.let { add(stringResource(if (details.network) R.string.tv_item_network else R.string.tv_item_studio) to it) }
            details.plays?.let { add(stringResource(R.string.tv_item_plays) to it.toString()) }
        }
        if (facts.isNotEmpty()) {
            FocusCard {
                facts.forEach { (label, value) -> Fact(label, value) }
            }
        }
        if (details.versions.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            BasicText(stringResource(R.string.tv_item_versions), style = PleyType.Tv.Title.copy(color = PleyColors.Fg), modifier = Modifier.padding(bottom = 8.dp))
            details.versions.forEach { version ->
                PanelRow(version.label, onClick = { onChooseVersion(version.id) }, checked = version.selected)
            }
        }
        if (page.mediaInfo.isNotEmpty()) {
            Spacer(Modifier.height(20.dp))
            BasicText(stringResource(R.string.tv_item_media_info), style = PleyType.Tv.Title.copy(color = PleyColors.Fg), modifier = Modifier.padding(bottom = 8.dp))
            TvMediaInfo(page.mediaInfo)
        }
    }
}

/** A card of text. It never takes focus: Down past the page's last focusable scrolls to it (the page's readOn). */
@Composable
private fun FocusCard(content: @Composable () -> Unit) {
    Column(
        Modifier
            .background(PleyColors.Surface2.copy(alpha = 0.6f), PleyShape.Card)
            .semantics(mergeDescendants = true) {}
            .padding(16.dp),
    ) { content() }
}

@Composable
private fun Fact(label: String, value: String) {
    Row(Modifier.padding(vertical = 3.dp)) {
        BasicText(label, style = PleyType.Tv.Body.copy(color = PleyColors.Subtle), modifier = Modifier.width(LabelWidth))
        BasicText(value, style = PleyType.Tv.Body.copy(color = PleyColors.Fg.copy(alpha = 0.85f)), maxLines = 2, overflow = TextOverflow.Ellipsis)
    }
}

/** Details end well inside the safe area: long lines are easier to read narrower. */
private val DetailsEnd = 200.dp

private val LabelWidth = 160.dp
