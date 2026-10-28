@file:OptIn(ExperimentalSerializationApi::class)

package app.pley.android.model

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonNamingStrategy

/**
 * How the app reads the server's JSON and writes its own: one instance for
 * every answer, request body and event.
 *
 * - Keys are snake_case on the wire and camelCase in Kotlin
 *   ([JsonNamingStrategy.SnakeCase]): `position_ms` is `positionMs`.
 * - Keys this app doesn't know are skipped, so a newer server can add
 *   fields.
 * - A field left out reads as its default: the server leaves optional
 *   fields out of big listings (docs/API.md, "Types"), so every optional
 *   property has one, `null` or an empty list.
 * - A `null` where the app expects a value reads as that property's
 *   default (coerceInputValues).
 * - Request bodies carry every value they hold, defaults included
 *   (encodeDefaults), so what the app builds is what the server gets; but
 *   never a `null` (explicitNulls off): the server reads a missing field as
 *   `None`, the same as `null`, for every body an app sends, but one:
 *   [StreamChoice.audioIndex] left out keeps the audio chosen before, and
 *   its `null` ([StreamChoice.automaticAudio]) forgets it.
 */
val PleyJson: Json = Json {
    namingStrategy = JsonNamingStrategy.SnakeCase
    ignoreUnknownKeys = true
    explicitNulls = false
    coerceInputValues = true
    encodeDefaults = true
}
