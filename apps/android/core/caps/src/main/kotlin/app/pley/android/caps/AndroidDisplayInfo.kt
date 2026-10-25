package app.pley.android.caps

import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Build
import android.view.Display

/**
 * The player's screen, from DisplayManager: every mode in physical pixels,
 * with its seamless alternative rates (Android 12 and up) and its HDR types
 * (Android 14 and up), getHdrCapabilities for older versions, and the
 * person's choice for matching the content's frame rate (Android 12 and up).
 */
class AndroidDisplayInfo(
    context: Context,
    /** The display to read: the player activity's, else the default one. */
    private val displayId: () -> Int = { Display.DEFAULT_DISPLAY },
) : DisplayInfo {
    private val displays = context.applicationContext.getSystemService(DisplayManager::class.java)

    override fun read(): DisplayState? {
        val manager = displays ?: return null
        val display = manager.getDisplay(displayId()) ?: manager.getDisplay(Display.DEFAULT_DISPLAY) ?: return null
        val modes = display.supportedModes.map { mode ->
            DisplayModeInfo(
                id = mode.modeId,
                width = mode.physicalWidth,
                height = mode.physicalHeight,
                refreshRate = mode.refreshRate,
                alternativeRefreshRates = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) mode.alternativeRefreshRates.toList() else emptyList(),
                hdrTypes = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) mode.supportedHdrTypes.toList() else null,
            )
        }
        @Suppress("DEPRECATION")
        val capabilities = display.hdrCapabilities?.supportedHdrTypes?.toList().orEmpty()
        return DisplayState(
            modes = modes,
            activeModeId = display.mode.modeId,
            hdrCapabilities = capabilities,
            matchContent = matchContent(manager),
        )
    }

    private fun matchContent(manager: DisplayManager): MatchContent =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            when (manager.matchContentFrameRateUserPreference) {
                DisplayManager.MATCH_CONTENT_FRAMERATE_NEVER -> MatchContent.Never
                DisplayManager.MATCH_CONTENT_FRAMERATE_SEAMLESSS_ONLY -> MatchContent.SeamlessOnly
                DisplayManager.MATCH_CONTENT_FRAMERATE_ALWAYS -> MatchContent.Always
                else -> MatchContent.Unknown
            }
        } else {
            MatchContent.Unknown
        }
}
