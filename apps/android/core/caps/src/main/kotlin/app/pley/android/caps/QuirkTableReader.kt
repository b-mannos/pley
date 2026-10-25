package app.pley.android.caps

import app.pley.android.model.DynamicRange
import org.json.JSONArray
import org.json.JSONObject

/**
 * Reads the shipped quirk table with org.json, which Android has warm:
 * the probe runs before anything else in the app needs these classes'
 * serializers. Same format as [QuirkTable.parse]; names it doesn't know
 * are left out rather than failing, since the file ships with the app.
 */
internal object QuirkTableReader {
    fun read(json: String): QuirkTable {
        val root = JSONObject(json)
        return QuirkTable(
            version = root.optInt("version", 1),
            quirks = root.optJSONArray("quirks").objects().map(::quirk),
        )
    }

    private fun quirk(o: JSONObject) = Quirk(
        id = o.getString("id"),
        why = o.optString("why", ""),
        models = o.optJSONArray("models").strings(),
        manufacturers = o.optJSONArray("manufacturers").strings(),
        tvOnly = o.optBoolean("tv_only", false),
        disabled = o.optBoolean("disabled", false),
        add = change(o.optJSONObject("add")),
        remove = change(o.optJSONObject("remove")),
        flags = flags(o.optJSONObject("flags")),
    )

    private fun change(o: JSONObject?): CapsChange {
        o ?: return CapsChange()
        val formats = PassthroughFormat.entries.associateBy { it.serialName() }
        val ranges = DynamicRange.entries.associateBy { it.wire }
        return CapsChange(
            dolbyVisionProfiles = o.optJSONArray("dolby_vision_profiles").ints(),
            passthrough = o.optJSONArray("passthrough").strings().mapNotNull { formats[it] },
            hdrTypes = o.optJSONArray("hdr_types").strings().mapNotNull { ranges[it] },
            videoCodecs = o.optJSONArray("video_codecs").strings(),
        )
    }

    private fun flags(o: JSONObject?): QuirkFlags {
        o ?: return QuirkFlags()
        fun flag(name: String): Boolean? = if (o.has(name) && !o.isNull(name)) o.getBoolean(name) else null
        return QuirkFlags(
            dvHdr10PlusHybridsBroken = flag("dv_hdr10_plus_hybrids_broken"),
            asyncQueueing = flag("async_queueing"),
            tunneling = flag("tunneling"),
            dvModeSwitch = flag("dv_mode_switch"),
        )
    }

    /** The name the JSON uses for a format (its @SerialName). */
    private fun PassthroughFormat.serialName(): String = when (this) {
        PassthroughFormat.DolbyDigital -> "dolby_digital"
        PassthroughFormat.DolbyDigitalPlus -> "dolby_digital_plus"
        PassthroughFormat.Atmos -> "atmos"
        PassthroughFormat.TrueHd -> "truehd"
        PassthroughFormat.Dts -> "dts"
        PassthroughFormat.DtsHd -> "dts_hd"
    }

    private fun JSONArray?.objects(): List<JSONObject> = this?.let { a -> (0 until a.length()).map { a.getJSONObject(it) } }.orEmpty()

    private fun JSONArray?.strings(): List<String> = this?.let { a -> (0 until a.length()).map { a.getString(it) } }.orEmpty()

    private fun JSONArray?.ints(): List<Int> = this?.let { a -> (0 until a.length()).map { a.getInt(it) } }.orEmpty()
}
