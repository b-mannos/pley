package app.pley.android.designsystem.image

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.ModifierNodeElement
import androidx.compose.ui.node.currentValueOf
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.platform.InspectorInfo
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import coil3.ImageLoader
import coil3.SingletonImageLoader
import coil3.compose.AsyncImage
import coil3.compose.AsyncImagePainter
import coil3.compose.LocalPlatformContext

/**
 * The loader [Artwork] uses: Coil's singleton (the app's [PleyImageLoader][app.pley.android.designsystem.PleyImageLoader])
 * unless something provides another, as tests and the debug gallery do.
 */
val LocalArtworkImageLoader = staticCompositionLocalOf<ImageLoader?> { null }

/**
 * Whether a picture has loaded, read only while drawing: the placeholder
 * goes and the trace counters count without recomposing anything.
 */
@Stable
class ArtworkLoad internal constructor() {
    var loaded: Boolean by mutableStateOf(false)
        internal set

    internal fun onState(state: AsyncImagePainter.State) {
        loaded = state is AsyncImagePainter.State.Success
    }
}

/**
 * A picture filling its box: [image]'s first address that loads, at the
 * box's width. Until then, and when none does, the title's gradient
 * ([Placeholder]) with [title] centred on it in [titleStyle] (none for
 * backdrops: leave [titleStyle] null). Nothing here recomposes when the
 * picture arrives: the placeholder is drawn behind it, in the draw phase.
 */
@Composable
fun Artwork(
    image: ArtworkImage?,
    title: String,
    modifier: Modifier = Modifier,
    titleStyle: TextStyle? = null,
    contentScale: ContentScale = ContentScale.Crop,
    alignment: Alignment = Alignment.Center,
) {
    val load = remember(image) { ArtworkLoad() }
    ArtworkBox(image, load, modifier.artworkPlaceholder(title, titleStyle, load), contentScale, alignment)
}

/** The box and its picture; [modifier] carries the placeholder and whatever is drawn over it. */
@Composable
internal fun ArtworkBox(
    image: ArtworkImage?,
    load: ArtworkLoad,
    modifier: Modifier,
    contentScale: ContentScale = ContentScale.Crop,
    alignment: Alignment = Alignment.Center,
) {
    Box(modifier) {
        if (image != null && !image.isEmpty) {
            val loader = LocalArtworkImageLoader.current ?: SingletonImageLoader.get(LocalPlatformContext.current)
            AsyncImage(
                model = image,
                contentDescription = null,
                imageLoader = loader,
                modifier = Modifier.matchParentSize(),
                contentScale = contentScale,
                alignment = alignment,
                onState = load::onState,
            )
        }
    }
}

/** Draws the placeholder behind the content while [load] hasn't loaded. */
internal fun Modifier.artworkPlaceholder(title: String, titleStyle: TextStyle?, load: ArtworkLoad): Modifier =
    this then PlaceholderElement(title, titleStyle, load)

private data class PlaceholderElement(
    val title: String,
    val titleStyle: TextStyle?,
    val load: ArtworkLoad,
) : ModifierNodeElement<PlaceholderNode>() {
    override fun create() = PlaceholderNode(title, titleStyle, load)

    override fun update(node: PlaceholderNode) = node.update(title, titleStyle, load)

    override fun InspectorInfo.inspectableProperties() {
        name = "artworkPlaceholder"
        properties["title"] = title
    }
}

private class PlaceholderNode(
    var title: String,
    var titleStyle: TextStyle?,
    var load: ArtworkLoad,
) : Modifier.Node(), DrawModifierNode, CompositionLocalConsumerModifierNode {
    private var measurer: TextMeasurer? = null
    private var picture: ImageBitmap? = null
    private var layout: TextLayoutResult? = null
    private var layoutSize = Size.Unspecified

    override val shouldAutoInvalidate: Boolean get() = false

    fun update(title: String, titleStyle: TextStyle?, load: ArtworkLoad) {
        if (title != this.title || titleStyle != this.titleStyle) {
            picture = null
            layout = null
        }
        this.title = title
        this.titleStyle = titleStyle
        this.load = load
        invalidateDraw()
    }

    override fun ContentDrawScope.draw() {
        if (!load.loaded) {
            drawStretched(picture ?: Placeholder.picture(title).also { picture = it }, Offset.Zero, size)
            titleLayout()?.let { text ->
                drawText(
                    text,
                    color = PleyColors.Fg.copy(alpha = 0.7f),
                    topLeft = androidx.compose.ui.geometry.Offset(
                        (size.width - text.size.width) / 2f,
                        (size.height - text.size.height) / 2f,
                    ),
                )
            }
        }
        drawContent()
    }

    private fun ContentDrawScope.titleLayout(): TextLayoutResult? {
        val style = titleStyle ?: return null
        if (title.isEmpty()) return null
        if (layout != null && layoutSize == size) return layout
        val padding = 12.dp.roundToPx()
        val width = (size.width.toInt() - 2 * padding).coerceAtLeast(0)
        val height = (size.height.toInt() - 2 * padding).coerceAtLeast(0)
        val textMeasurer = measurer ?: TextMeasurer(
            defaultFontFamilyResolver = currentValueOf(LocalFontFamilyResolver),
            defaultDensity = currentValueOf(LocalDensity),
            defaultLayoutDirection = currentValueOf(LocalLayoutDirection),
            cacheSize = 0,
        ).also { measurer = it }
        layout = textMeasurer.measure(
            text = title,
            style = style.copy(textAlign = TextAlign.Center),
            overflow = TextOverflow.Ellipsis,
            maxLines = 4,
            constraints = Constraints(minWidth = width, maxWidth = width, maxHeight = height),
        )
        layoutSize = size
        return layout
    }
}
