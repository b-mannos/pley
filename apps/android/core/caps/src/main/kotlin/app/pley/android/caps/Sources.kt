package app.pley.android.caps

/**
 * Where the probe reads from. Each has an Android implementation and, in
 * tests, a fixture: a phone, a Fire TV stick, a real device's dump.
 */
fun interface CodecInventory {
    /** Every decoder of the types the probe asks about ([Codecs.PROBED_MIMES]); slow, so read once per system build. */
    fun decoders(): List<DecoderInfo>
}

fun interface DisplayInfo {
    /** The player's screen; `null` when there is none (a headless test). */
    fun read(): DisplayState?
}

fun interface AudioInfo {
    /** The audio output in use. */
    fun read(): AudioState
}

/** Tells the probe when the screen or the audio output changes. */
interface ChangeSource {
    fun start(onChange: () -> Unit)

    fun stop()
}
