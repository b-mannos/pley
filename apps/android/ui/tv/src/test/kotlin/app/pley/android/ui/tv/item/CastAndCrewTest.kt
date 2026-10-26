package app.pley.android.ui.tv.item

import app.pley.android.model.Credit
import app.pley.android.model.CreditRole
import app.pley.android.screens.item.CrewLine
import org.junit.Assert.assertEquals
import org.junit.Test

/** Cast & Crew on a TV item page: who made it follows the cast, each person once. */
class CastAndCrewTest {
    private val roles = mapOf(CreditRole.Director to "Director", CreditRole.Writer to "Writer", CreditRole.Creator to "Creator")

    @Test
    fun whoMadeItFollowsTheCastEachPersonOnce() {
        val eastwood = Credit(1, "Clint Eastwood", CreditRole.Actor, character = "Will Munny")
        val hackman = Credit(2, "Gene Hackman", CreditRole.Actor, character = "Little Bill")
        val crew = listOf(
            CrewLine(CreditRole.Director, listOf(eastwood.copy(role = CreditRole.Director, character = null)), more = 0),
            CrewLine(CreditRole.Writer, listOf(Credit(3, "David Webb Peoples", CreditRole.Writer, photoUrl = "/api/people/3/photo")), more = 0),
        )

        val tiles = castAndCrew(listOf(eastwood, hackman), crew, roles, "http://nas:7539")

        assertEquals(
            listOf("Clint Eastwood" to "Will Munny", "Gene Hackman" to "Little Bill", "David Webb Peoples" to "Writer"),
            tiles.map { it.title to it.subtitle },
        )
        assertEquals(listOf("http://nas:7539/api/people/3/photo"), tiles.last().image?.candidates)
    }

    @Test
    fun aShowWithoutCastStillListsItsCreators() {
        val crew = listOf(CrewLine(CreditRole.Creator, listOf(Credit(5, "Vince Gilligan", CreditRole.Creator)), more = 0))
        assertEquals(listOf("Vince Gilligan" to "Creator"), castAndCrew(emptyList(), crew, roles, "http://nas:7539").map { it.title to it.subtitle })
    }
}
