package app.pley.android.designsystem.tile

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.image.ArtworkBox
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.designsystem.image.ArtworkLoad
import app.pley.android.designsystem.image.artworkPlaceholder

/**
 * A Live TV channel's logo (the web's ChannelLogo), fitted in its box
 * [inset] from the edges, over whatever well [modifier] paints. A channel
 * without a logo, or whose logo doesn't load, shows [label] (its name, or
 * on small tiles its number) on the gradient of [label] instead. While the
 * logo loads, only the well shows: no gradient flashes before it.
 */
@Composable
fun ChannelLogo(
    image: ArtworkImage?,
    label: String,
    textStyle: TextStyle,
    modifier: Modifier = Modifier,
    inset: Dp = 6.dp,
) {
    val load = remember(image) { ArtworkLoad() }
    val never = remember { ArtworkLoad() }
    val fallback = image == null || image.isEmpty || load.failed
    Box(if (fallback) modifier.artworkPlaceholder(label, textStyle, never) else modifier) {
        if (!fallback) ArtworkBox(image, load, Modifier.matchParentSize().padding(inset), ContentScale.Fit)
    }
}
