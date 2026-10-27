package app.pley.android.ui.phone.kit

import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Dragging a row of [list] by its handle (a playlist's entries, the
 * libraries): the row follows the finger ([offset], read while drawing)
 * and takes the place of the row it passes the middle of ([onMove], in
 * lazy positions); rows are keyed by their `Long` ids, and rows with other
 * keys (a header) stay put. The list scrolls when the row is held near its
 * top or bottom. [onDrop] gets the row's key at the end.
 */
@Stable
internal class RowDrag(private val list: LazyListState, private val scope: CoroutineScope) {
    var key by mutableStateOf<Long?>(null)
        private set
    var offset by mutableFloatStateOf(0f)
        private set
    var onMove: (from: Int, to: Int) -> Unit = { _, _ -> }
    var onDrop: (Long) -> Unit = {}

    /** After a move, the lazy position the row is waiting to be laid out at. */
    private var awaiting = -1
    private var scroller: Job? = null

    fun start(key: Long) {
        this.key = key
        offset = 0f
        awaiting = -1
        scroller = scope.launch {
            while (isActive) {
                val speed = edgeSpeed()
                if (speed != 0f) {
                    val moved = list.scrollBy(speed)
                    offset += moved
                    swap()
                }
                withFrameNanos { }
            }
        }
    }

    fun by(dy: Float) {
        offset += dy
        swap()
    }

    fun end() {
        val dropped = key
        scroller?.cancel()
        key = null
        offset = 0f
        if (dropped != null) onDrop(dropped)
    }

    private fun swap() {
        val k = key ?: return
        val items = list.layoutInfo.visibleItemsInfo
        val current = items.firstOrNull { it.key == k } ?: return
        if (awaiting >= 0 && current.index != awaiting) return
        awaiting = -1
        val middle = current.offset + offset + current.size / 2f
        val target = items.firstOrNull { it.key is Long && it.key != k && middle >= it.offset && middle <= it.offset + it.size } ?: return
        onMove(current.index, target.index)
        offset += current.offset - target.offset
        awaiting = target.index
    }

    /** Pixels a frame to scroll while the row is held within 64 dp of the list's ends. */
    private fun edgeSpeed(): Float {
        val k = key ?: return 0f
        val info = list.layoutInfo
        val current = info.visibleItemsInfo.firstOrNull { it.key == k } ?: return 0f
        val top = current.offset + offset
        val bottom = top + current.size
        val zone = EDGE_PX
        return when {
            top < info.viewportStartOffset + zone && list.canScrollBackward -> -SCROLL_PX
            bottom > info.viewportEndOffset - zone && list.canScrollForward -> SCROLL_PX
            else -> 0f
        }
    }

    companion object {
        const val EDGE_PX = 160f
        const val SCROLL_PX = 18f
    }
}
