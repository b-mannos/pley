package app.pley.android.ui.phone.kit

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import app.pley.android.designsystem.reducedMotion
import app.pley.android.designsystem.tile.ArtworkTile
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileCaption
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.designsystem.tile.TileStyle
import app.pley.android.ui.phone.R

/**
 * A card (design spec §3.2): [model]'s art as [kind] (radius 6, its
 * progress, tick and badges), with the title and subtitle 8 dp under it.
 * Its width comes from [modifier]; the art keeps [kind]'s shape.
 *
 * Tapping calls [onClick]. A long press calls [onLongClick] (the item's
 * action sheet) with a haptic tick. While pressed the art grows to 1.04
 * with a 2 dp accent ring, drawn without recomposing ([cardPress]).
 * Screen readers hear the whole card as one button saying the tile's
 * description, with "More actions" for the long press.
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun MediaCard(
    model: ArtworkTileModel,
    kind: TileKind,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    onLongClick: (() -> Unit)? = null,
) {
    KitProbe.cardComposed?.let { probe -> SideEffect { probe(model.id) } }
    val interactions = remember { MutableInteractionSource() }
    val reduced = reducedMotion()
    val shape = TileStyle.Phone.shape(kind)
    Column(
        modifier.combinedClickable(
            interactionSource = interactions,
            indication = null,
            role = Role.Button,
            onLongClickLabel = if (onLongClick != null) stringResource(R.string.kit_card_more) else null,
            onLongClick = onLongClick,
            onClick = onClick,
        ),
    ) {
        ArtworkTile(
            model = model,
            kind = kind,
            style = TileStyle.Phone,
            modifier = Modifier
                .cardPress(interactions, shape, reduced)
                .fillMaxWidth()
                .aspectRatio(kind.aspectRatio),
        )
        Spacer(Modifier.height(TileStyle.Phone.captionGap))
        TileCaption(model.title, model.subtitle, TileStyle.Phone)
    }
}

/** Counts compositions of cards and rows, for the tests that pressing a card recomposes neither. Unset in the app. */
internal object KitProbe {
    @Volatile
    var cardComposed: ((Long) -> Unit)? = null

    @Volatile
    var rowComposed: ((String) -> Unit)? = null
}
