package app.pley.android.ui.tv.home

import androidx.compose.runtime.Immutable
import app.pley.android.common.Tier
import app.pley.android.designsystem.image.ArtworkCandidates
import app.pley.android.designsystem.image.ArtworkImage
import app.pley.android.designsystem.tile.ArtworkTileModel
import app.pley.android.designsystem.tile.TileKind
import app.pley.android.model.ItemSummary
import app.pley.android.screens.PleyKey
import app.pley.android.screens.home.HomeInfo
import app.pley.android.screens.home.HomeState
import app.pley.android.screens.home.RowShape

/**
 * Home as the TV draws it: [HomeState] with each row's tiles made
 * ([ArtworkTileModel.of]) and the hero's backdrop, worked out off the main
 * thread by the ViewModel ([app.pley.android.screens.home.HomeViewModel.present]).
 */
@Immutable
internal data class TvHomeModel(
    val state: HomeState,
    val rows: List<TvHomeRow> = emptyList(),
    val hero: TvHero? = null,
) {
    companion object {
        /** The shaping for a device of [tier] (its backdrops' width). One per tier, so the ViewModel keeps one flow. */
        fun shaping(tier: Tier): (HomeState) -> TvHomeModel = SHAPINGS.getValue(tier)

        private val SHAPINGS: Map<Tier, (HomeState) -> TvHomeModel> = Tier.entries.associateWith { tier ->
            { state: HomeState -> of(state, tier) }
        }

        fun of(state: HomeState, tier: Tier): TvHomeModel {
            if (state !is HomeState.Ready) return TvHomeModel(state)
            val rows = state.rows.map { row ->
                val kind = row.shape.tileKind
                TvHomeRow(
                    key = row.key,
                    title = row.title,
                    kind = kind,
                    tiles = row.items.map { ArtworkTileModel.of(it, kind, state.origin) },
                    items = row.items,
                    seeAll = row.seeAll,
                )
            }
            return TvHomeModel(state, rows, state.hero?.let { TvHero(it, backdropOf(it.item, state.origin, tier)) })
        }

        /** The backdrop behind [item]: its own, else its show's or parent's, at most 1280 wide (960 on T0). */
        fun backdropOf(item: ItemSummary, origin: String, tier: Tier): ArtworkImage =
            ArtworkCandidates.backdrop(item, origin, tier)
    }
}

/** A row of tiles, with the items they stand for (by position). */
@Immutable
internal data class TvHomeRow(
    val key: String,
    val title: String,
    val kind: TileKind,
    val tiles: List<ArtworkTileModel>,
    val items: List<ItemSummary>,
    val seeAll: PleyKey?,
) {
    /** The item of [tile]: by its id, in this row. */
    fun itemOf(tile: ArtworkTileModel): ItemSummary? = items.firstOrNull { it.id == tile.id }

    /** The [count] items after [item] in the row, whose details are asked for ahead. */
    fun after(item: ItemSummary, count: Int = 2): List<ItemSummary> {
        val at = items.indexOfFirst { it.id == item.id }
        return if (at < 0) emptyList() else items.drop(at + 1).take(count)
    }
}

/** The hero: what it says, and its backdrop. */
@Immutable
internal data class TvHero(val info: HomeInfo, val backdrop: ArtworkImage)

/** The tile a row shape draws on TV. */
internal val RowShape.tileKind: TileKind
    get() = when (this) {
        RowShape.Landscape -> TileKind.Landscape
        RowShape.Poster -> TileKind.Poster
        RowShape.Square -> TileKind.Square
    }
