package app.pley.android.screens.livetv

import app.pley.android.data.LiveTvRepository
import app.pley.android.model.LiveCategory
import app.pley.android.model.LiveChannel
import app.pley.android.model.LiveGroup
import app.pley.android.model.LiveGroups
import app.pley.android.model.LiveGuide
import app.pley.android.model.LiveProgram
import app.pley.android.testing.Fixtures
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Live TV's rules: categories, groups by region, names in a group, pages, the guide's chunks. */
class LiveTvTest {
    @Test
    fun `categories keep a key the player is given`() {
        for (c in listOf(LiveCategory.All, LiveCategory.Favorites, LiveCategory.Recent, LiveCategory.Group("UK| Sport"))) {
            assertEquals(c, LiveCategory.parse(c.key))
        }
        assertEquals(LiveCategory.All, LiveCategory.parse(null))
        assertEquals("group", LiveCategory.Group("News").wire)
    }

    @Test
    fun `a region's groups are gathered under it, a region of one stays a group`() {
        val groups = Fixtures.decode<LiveGroups>("live/groups.json").groups
        val tree = LiveTv.groupTree(groups)
        assertEquals(
            listOf(
                GroupEntry.Single(groups[0]),
                GroupEntry.Single(groups[1]),
                GroupEntry.Region("UK", listOf(groups[2], groups[3]), 101),
                GroupEntry.Single(groups[4]),
            ),
            tree,
        )
        assertEquals("UK · Sport", LiveTv.groupName(groups[2], groups[2].name))
        assertEquals("News", LiveTv.groupName(groups[0], "News"))
        assertEquals("Gone", LiveTv.groupName(null, "Gone"))
    }

    @Test
    fun `names leave out the region their group already says`() {
        assertEquals("UK" to "Sport", LiveTv.splitRegion("UK| Sport"))
        assertEquals("UK" to "Sport", LiveTv.splitRegion("[UK] Sport"))
        assertEquals("UK" to "Sport", LiveTv.splitRegion("UK: Sport"))
        assertEquals("BEE" to "SOLO SPORT F1 HEVC", LiveTv.splitRegion("|BEE| SOLO SPORT F1 HEVC"))
        assertNull(LiveTv.splitRegion("Sport 1080i"))
        assertNull(LiveTv.splitRegion("Sky: News at Ten"))
        val sky = LiveChannel(1, name = "UK| Sky Sports 1", group = "UK| Sport")
        assertEquals("Sky Sports 1", LiveTv.channelName(sky, LiveCategory.Group("UK| Sport")))
        assertEquals("UK| Sky Sports 1", LiveTv.channelName(sky, LiveCategory.All))
        assertEquals("FR| TF1", LiveTv.channelName(LiveChannel(2, name = "FR| TF1", group = "UK| Sport"), LiveCategory.Group("UK| Sport")))
    }

    @Test
    fun `pages find a channel by its place in the whole list`() {
        val page = LiveTvRepository.PAGE
        val pages = ChannelPages(
            total = page + 2,
            pages = mapOf(1 to listOf(LiveChannel(7, name = "Seven"), LiveChannel(8, name = "Eight")), 0 to listOf(LiveChannel(1, name = "One"))),
        )
        assertEquals(1L, pages[0]?.id)
        assertNull(pages[1])
        assertEquals(8L, pages[page + 1]?.id)
        assertEquals(listOf(1L, 7L, 8L), pages.loaded.map { it.id })
    }

    @Test
    fun `the guide starts half an hour before the half hour now is in, and asks for the chunks around what shows`() {
        val now = 1_761_752_400L
        val start = LiveTv.guideStart(now)
        assertEquals(1_761_750_000L, start)
        val chunk = LiveTvRepository.GUIDE_CHUNK_S
        val first = Math.floorDiv(start, chunk) * chunk
        // The one in view and one either side, none before the guide's start.
        assertEquals(listOf(first, first + chunk), LiveTv.chunks(start, start, start + 3600))
        assertEquals(emptyList<Long>(), LiveTv.chunks(start, start + LiveTv.GUIDE_SPAN_S + chunk * 2, start + LiveTv.GUIDE_SPAN_S + chunk * 3))
    }

    @Test
    fun `chunks merge into one list a channel, each programme once`() {
        val a = LiveProgram(1, 100, 200, "A")
        val b = LiveProgram(1, 200, 300, "B")
        val merged = LiveTv.merge(
            listOf(LiveGuide(0, 250, mapOf(1L to listOf(b, a))), LiveGuide(250, 500, mapOf(1L to listOf(b, LiveProgram(1, 300, 400, "C"))))),
        )
        assertEquals(listOf("A", "B", "C"), merged.getValue(1).map { it.title })
        assertEquals(listOf("B"), LiveTv.between(merged[1], 210, 290).map { it.title })
        assertEquals(1 to 5, LiveTv.left(LiveProgram(1, 0, 3900, "long"), 0))
    }

    @Test
    fun `a group's region comes with it`() {
        val g = LiveGroup("UK| News", "UK", "News", 21)
        assertEquals("UK · News", LiveTv.groupName(g, g.name))
    }
}
