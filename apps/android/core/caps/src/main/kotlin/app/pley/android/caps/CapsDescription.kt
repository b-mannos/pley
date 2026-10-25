package app.pley.android.caps

import android.content.res.Resources
import app.pley.android.model.DynamicRange

/** Diagnostics' lines for a snapshot: what the device plays and why. */
object CapsDescription {
    fun lines(snapshot: CapsSnapshot, res: Resources): List<String> = buildList {
        val report = snapshot.report
        val details = snapshot.details
        val input = details.input
        val none = res.getString(R.string.caps_none)

        input?.device?.let { device ->
            val form = when {
                device.fireTv -> R.string.caps_form_fire_tv
                device.form == DeviceFacts.Form.Tv -> R.string.caps_form_tv
                device.form == DeviceFacts.Form.Tablet -> R.string.caps_form_tablet
                else -> R.string.caps_form_phone
            }
            add(res.getString(R.string.caps_device, device.model, device.manufacturer, device.sdk, res.getString(form), details.tier))
        }
        add(
            res.getString(
                when {
                    details.policy.allowSoftware -> R.string.caps_policy_any
                    details.policy.tv -> R.string.caps_policy_tv
                    else -> R.string.caps_policy_phone
                },
            ),
        )

        val video = report.video.orEmpty()
        if (video.isEmpty()) add(res.getString(R.string.caps_no_video))
        video.forEach { entry ->
            if (entry.maxWidth == null && entry.profiles.isEmpty()) {
                add(res.getString(R.string.caps_video_unlimited, entry.codec))
                return@forEach
            }
            val size = if (entry.maxWidth != null && entry.maxHeight != null) {
                res.getString(R.string.caps_size_rate, entry.maxWidth, entry.maxHeight, rate(entry.maxFps?.toFloat()))
            } else {
                res.getString(R.string.caps_unknown)
            }
            add(
                res.getString(
                    R.string.caps_video,
                    entry.codec,
                    res.getString(if (entry.hardware) R.string.caps_hardware else R.string.caps_software),
                    entry.profiles.joinToString().ifEmpty { none },
                    size,
                    entry.maxBitDepth ?: 8,
                    entry.maxLevel?.toString() ?: res.getString(R.string.caps_unknown),
                ),
            )
        }
        input?.decoders?.filter { it.isVideo }?.forEach { decoder ->
            add(
                res.getString(
                    R.string.caps_decoder,
                    decoder.name,
                    Codecs.VIDEO[decoder.mime] ?: decoder.mime,
                    res.getString(if (decoder.hardware) R.string.caps_hardware else R.string.caps_software),
                ),
            )
        }

        add(res.getString(R.string.caps_screen_hdr, ranges(details.screenHdrTypes).ifEmpty { none }))
        add(res.getString(R.string.caps_hdr_played, ranges(report.hdrTypes.orEmpty()).ifEmpty { none }))
        val dv = details.dolbyVisionProfiles.joinToString {
            res.getString(R.string.caps_profile_level, it.profile, it.maxLevel?.toString() ?: res.getString(R.string.caps_unknown))
        }.ifEmpty { none }
        val dvScreen = DynamicRange.DolbyVision in details.screenHdrTypes
        add(res.getString(if (dvScreen || details.dolbyVisionProfiles.isEmpty()) R.string.caps_dolby_vision else R.string.caps_dolby_vision_no_screen, dv))

        val display = report.display
        if (display == null) {
            add(res.getString(R.string.caps_no_screen))
        } else {
            val active = input?.display?.activeMode
            add(
                res.getString(
                    R.string.caps_screen,
                    display.width,
                    display.height,
                    rate(active?.refreshRate),
                    display.refreshRates.joinToString { rate(it) },
                ),
            )
            input?.display?.let { state ->
                add(
                    res.getString(
                        R.string.caps_modes,
                        state.modes.joinToString { mode ->
                            val hdr = mode.hdrTypes?.let { types -> " " + ranges(types.mapNotNull { CapsBuilder.dynamicRange(it) }).ifEmpty { "SDR" } }.orEmpty()
                            "${mode.width}×${mode.height}@${rate(mode.refreshRate)}$hdr"
                        },
                    ),
                )
                val match = when (state.matchContent) {
                    MatchContent.Never -> R.string.caps_match_never
                    MatchContent.SeamlessOnly -> R.string.caps_match_seamless
                    MatchContent.Always -> R.string.caps_match_always
                    MatchContent.Unknown -> R.string.caps_match_unknown
                }
                add(res.getString(R.string.caps_match_content, res.getString(match)))
            }
        }

        add(res.getString(R.string.caps_audio_decode, report.audioDecode.orEmpty().joinToString().ifEmpty { none }))
        add(
            res.getString(
                R.string.caps_passthrough,
                report.audioPassthrough.orEmpty().joinToString().ifEmpty { none },
                report.audioChannels ?: 2,
                input?.audio?.route ?: res.getString(R.string.caps_unknown),
            ),
        )
        add(res.getString(R.string.caps_quirks, details.quirks.joinToString().ifEmpty { none }))
        val on = res.getString(R.string.caps_on)
        val off = res.getString(R.string.caps_off)
        add(
            res.getString(
                R.string.caps_player_flags,
                if (details.forceAsyncQueueing) on else off,
                if (details.tunnelingAllowed) on else off,
                res.getString(if (details.dvHdr10PlusHybridsBroken) R.string.caps_hybrids_broken else R.string.caps_hybrids_fine),
            ),
        )
    }

    private fun ranges(types: List<DynamicRange>): String = types.joinToString { range ->
        when (range) {
            DynamicRange.Hdr10 -> "HDR10"
            DynamicRange.Hdr10Plus -> "HDR10+"
            DynamicRange.Hlg -> "HLG"
            DynamicRange.DolbyVision -> "Dolby Vision"
            DynamicRange.Sdr -> "SDR"
            DynamicRange.Unknown -> "?"
        }
    }

    /** 23.976, 24, 59.94: rates as people write them. */
    private fun rate(value: Float?): String {
        value ?: return "?"
        val rounded = CapsBuilder.roundRate(value)
        return if (rounded % 1f == 0f) rounded.toInt().toString() else rounded.toString()
    }
}
