package app.pley.android.screens.admin.livetv

import app.pley.android.model.admin.LineupFilter
import app.pley.android.model.admin.LineupGroup
import app.pley.android.model.admin.LineupState
import app.pley.android.model.admin.LiveSourceAccount
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Live TV's pages and the rules they share with the web's (apps/web/src/pages/settings/livetv). */
class LiveTvAdminTest {
    @Test
    fun aPagesIdNamesItAndComesBack() {
        val pages = listOf(
            LiveTvAdminPage.Front,
            LiveTvAdminPage.Options,
            LiveTvAdminPage.Groups,
            LiveTvAdminPage.Channels,
            LiveTvAdminPage.Source(null),
            LiveTvAdminPage.Source(12),
        )
        for (page in pages) assertEquals(page, LiveTvAdminPage.of(page.id))
        assertEquals("source:new", LiveTvAdminPage.Source(null).id)
        assertEquals("source:12", LiveTvAdminPage.Source(12).id)
        assertEquals(LiveTvAdminPage.Front, LiveTvAdminPage.of("source:abc"))
        assertEquals(LiveTvAdminPage.Front, LiveTvAdminPage.of("elsewhere"))
    }

    @Test
    fun aSourceWithoutANameIsCalledByItsHost() {
        assertEquals("provider.example", defaultSourceName(" http://provider.example:8080/get.php?x=1 "))
        assertEquals("", defaultSourceName("provider.example"))
        assertEquals("", defaultSourceName(""))
        assertEquals("", defaultSourceName("http://[bad"))
    }

    @Test
    fun anAccountExpiresSoonInTheLastTwoWeeks() {
        val now = 1_700_000_000L
        assertNull(expiresSoon(null, now))
        assertNull(expiresSoon(LiveSourceAccount("Active", expiresAt = null), now))
        assertNull(expiresSoon(LiveSourceAccount("Active", expiresAt = now + 15 * 86_400), now))
        assertEquals(14, expiresSoon(LiveSourceAccount("Active", expiresAt = now + 14 * 86_400), now))
        assertEquals(1, expiresSoon(LiveSourceAccount("Active", expiresAt = now + 60), now))
        assertEquals(-2, expiresSoon(LiveSourceAccount("Expired", expiresAt = now - 2 * 86_400), now))
    }

    @Test
    fun groupsGoTogetherByRegionAtTheirFirstPlace() {
        val groups = listOf(
            LineupGroup("UK| News", "UK", "News", count = 10),
            LineupGroup("Sport", null, "Sport", count = 5, hidden = 5),
            LineupGroup("FR| Films", "FR", "Films", count = 4),
            LineupGroup("UK| Films", "UK", "Films", count = 8, hidden = 3),
        )
        val sections = groupSections(groups)
        assertEquals(listOf("UK", null, null), sections.map { it.region })
        assertEquals(listOf("UK| News", "UK| Films"), sections[0].groups.map { it.name })
        assertEquals(18, sections[0].channels)
        // A region of one group is listed as the group alone.
        assertEquals("FR| Films", sections[2].groups.single().name)
        assertEquals(false, sections[0].allHidden)
        assertEquals(false, sections[0].allShown)

        assertEquals(listOf("Sport"), groupSections(groups, shown = GroupsShown.Hidden).flatMap { it.groups }.map { it.name })
        assertEquals(3, groupSections(groups, shown = GroupsShown.Shown).flatMap { it.groups }.size)
        // Found by name, a region with one group found is that group alone.
        val found = groupSections(groups, query = " films")
        assertEquals(listOf(null, null), found.map { it.region })
        assertEquals(listOf("FR| Films", "UK| Films"), found.map { it.groups.single().name })
    }

    @Test
    fun choicesKeepAValueTheWebDoesntOffer() {
        assertEquals(listOf(3, 6, 12, 24, 48, 168), refreshChoices(24))
        assertEquals(listOf(1, 3, 6, 12, 24, 48, 168), refreshChoices(1))
        assertEquals(listOf(1, 2, 3, 5, 7, 10, 14), guideDayChoices(7))
        assertEquals(listOf(1, 2, 3, 4, 5, 7, 10, 14), guideDayChoices(4))
        assertEquals(listOf(null, 1080, 720, 480), heightChoices(null))
        assertEquals(listOf(null, 2160, 1080, 720, 480), heightChoices(2160))
    }

    @Test
    fun codecsAndEncodersAreNamedAsPeopleKnowThem() {
        assertEquals("MPEG-2", codecName("mpeg2video"))
        assertEquals("Dolby Digital Plus", codecName("eac3"))
        assertEquals("vp9", codecName("vp9"))
        assertEquals("", codecName(null))
        assertNull(encoderName("libx264"))
        assertEquals("NVIDIA", encoderName("h264_nvenc"))
        assertEquals("h264_vaapi", encoderName("h264_vaapi"))
    }

    @Test
    fun aPageOfTheLineupIsAskedForWithItsFilter() {
        assertEquals(
            listOf("source_id" to null, "group" to null, "state" to "all", "q" to null, "offset" to "0", "limit" to "100"),
            lineupQuery(LineupFilter(), 0),
        )
        assertEquals(
            listOf("source_id" to "3", "group" to "UK| News", "state" to "no-guide", "q" to "bbc", "offset" to "200", "limit" to "100"),
            lineupQuery(LineupFilter(sourceId = 3, group = "UK| News", state = LineupState.NoGuide, q = " bbc "), 2),
        )
        assertEquals("q" to null, lineupQuery(LineupFilter(q = "  "), 0)[3])
    }
}
