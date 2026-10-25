package app.pley.android.common

import android.app.ActivityManager
import android.app.UiModeManager
import android.content.Context
import android.content.res.Configuration
import android.os.Build

/** Which interface the device gets. Tablets are phones with more room. */
enum class FormFactor { Phone, Tv }

/**
 * How much the device can hold, which sizes buffers, image caches and
 * library windows. T0 is the floor: the 1 GB Fire TV sticks and the 1.5 GB
 * Chromecast with Google TV HD.
 */
enum class Tier { T0, T1, T2 }

/**
 * What the app needs to know about the device it runs on, read once at
 * start. A TV is what Android says is in television mode, never what claims
 * the leanback feature (Fire TV sticks may not); a Fire TV is what has
 * Amazon's feature.
 */
data class DeviceProfile(
    val form: FormFactor,
    val fireTv: Boolean,
    val tier: Tier,
    /** Build.MODEL, which the quirk tables are keyed by (AFTSS, SHIELD Android TV…). */
    val model: String,
    val totalRamBytes: Long,
    val lowRam: Boolean,
) {
    val isTv: Boolean get() = form == FormFactor.Tv

    companion object {
        private const val GIB = 1L shl 30

        /** Below this, T0: 1.6 GiB. */
        val T0_BELOW = 16 * GIB / 10

        /** Below this, T1: 2.6 GiB. */
        val T1_BELOW = 26 * GIB / 10

        /** T0 when Android calls the device low on memory or it has under 1.6 GiB, T1 under 2.6 GiB, else T2. */
        fun tierOf(lowRam: Boolean, totalRamBytes: Long): Tier = when {
            lowRam || totalRamBytes < T0_BELOW -> Tier.T0
            totalRamBytes < T1_BELOW -> Tier.T1
            else -> Tier.T2
        }

        /** This device's profile. Cheap: no disk or network. */
        fun of(context: Context): DeviceProfile {
            val uiMode = context.getSystemService(UiModeManager::class.java)
            val tv = uiMode?.currentModeType == Configuration.UI_MODE_TYPE_TELEVISION
            val activities = context.getSystemService(ActivityManager::class.java)
            val memory = ActivityManager.MemoryInfo().also { activities?.getMemoryInfo(it) }
            val lowRam = activities?.isLowRamDevice ?: false
            return DeviceProfile(
                form = if (tv) FormFactor.Tv else FormFactor.Phone,
                fireTv = context.packageManager.hasSystemFeature(LocalNetwork.FIRE_TV_FEATURE),
                tier = tierOf(lowRam, memory.totalMem),
                model = Build.MODEL.orEmpty(),
                totalRamBytes = memory.totalMem,
                lowRam = lowRam,
            )
        }
    }
}
