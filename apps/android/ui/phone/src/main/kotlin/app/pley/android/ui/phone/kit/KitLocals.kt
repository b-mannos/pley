package app.pley.android.ui.phone.kit

import androidx.compose.runtime.staticCompositionLocalOf
import app.pley.android.common.Tier

/**
 * The device's memory tier, which sizes how far ahead rows and grids load
 * pictures (T0 only to disk beyond the first screen). The shell provides
 * it from the device profile; phones are almost always T2.
 */
val LocalPhoneDeviceTier = staticCompositionLocalOf { Tier.T2 }

/** Where the offline banner leads: [label] ("Downloads") opens what still works without the server. */
class OfflineAction(val label: String, val onClick: () -> Unit)

/** The offline banner's way on; the shell provides it where there are downloads. `null`: the banner only says so. */
val LocalOfflineAction = staticCompositionLocalOf<OfflineAction?> { null }
