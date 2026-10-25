package app.pley.android.model

import kotlinx.serialization.Serializable

/** A part someone had in a title (apps/server/src/people.rs, `Role`). */
@Serializable(with = CreditRole.Serializer::class)
enum class CreditRole(override val wire: String) : WireEnum {
    Actor("actor"),
    Creator("creator"),
    Director("director"),
    Writer("writer"),
    Producer("producer"),
    Unknown("unknown"),
    ;

    object Serializer : LenientEnumSerializer<CreditRole>("CreditRole", CreditRole.entries, CreditRole.Unknown)
}

/** Someone credited in an item: [ItemDetail.cast] and [ItemDetail.crew]. */
@Serializable
data class Credit(
    /** The person: GET /api/people/{id}. */
    val id: Long,
    val name: String,
    val role: CreditRole,
    /** Actors: the part played. */
    val character: String? = null,
    /** "/api/people/7/photo?v=…": add `&w=<px>` for a smaller one. */
    val photoUrl: String? = null,
)

/** A person in search results. */
@Serializable
data class PersonSummary(
    val id: Long,
    val name: String,
    val photoUrl: String? = null,
)

/**
 * GET /api/people/{id}: a person and what they are credited in among the
 * titles this profile may see, newest first.
 */
@Serializable
data class PersonDetail(
    val id: Long,
    val name: String,
    val photoUrl: String? = null,
    val credits: List<PersonCredit> = emptyList(),
)

/** A movie or show a person is credited in; episode credits count towards their show. */
@Serializable
data class PersonCredit(
    val item: ItemSummary,
    val roles: List<PersonRole> = emptyList(),
)

/** One of a person's parts in a title. */
@Serializable
data class PersonRole(
    val role: CreditRole,
    val character: String? = null,
    /** Credits from single episodes (guest parts, episode directors): how many. */
    val episodes: Int? = null,
)
