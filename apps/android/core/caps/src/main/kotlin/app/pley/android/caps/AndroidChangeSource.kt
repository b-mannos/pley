package app.pley.android.caps

import android.content.Context
import android.hardware.display.DisplayManager
import android.os.Handler
import android.os.Looper
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.audio.AudioCapabilitiesReceiver

/**
 * Tells the probe when the screen changes (a display listener) or the audio
 * output does: Media3's AudioCapabilitiesReceiver covers HDMI plugs
 * (ACTION_HDMI_AUDIO_PLUG), audio devices coming and going
 * (AudioDeviceCallback) and Fire TV's surround setting. Callbacks arrive on
 * the main thread and only ask for a new probe, which runs elsewhere.
 */
@OptIn(markerClass = [UnstableApi::class])
class AndroidChangeSource(context: Context) : ChangeSource {
    private val context = context.applicationContext
    private val displays = this.context.getSystemService(DisplayManager::class.java)
    private val main = Handler(Looper.getMainLooper())
    private var displayListener: DisplayManager.DisplayListener? = null
    private var audioReceiver: AudioCapabilitiesReceiver? = null

    @Synchronized
    override fun start(onChange: () -> Unit) {
        if (displayListener != null) return
        val listener = object : DisplayManager.DisplayListener {
            override fun onDisplayAdded(displayId: Int) = onChange()

            override fun onDisplayRemoved(displayId: Int) = onChange()

            override fun onDisplayChanged(displayId: Int) = onChange()
        }
        displays?.registerDisplayListener(listener, main)
        displayListener = listener
        // Constructed off the main thread, it still calls back on the main looper.
        audioReceiver = AudioCapabilitiesReceiver(context) { onChange() }.also { it.register() }
    }

    @Synchronized
    override fun stop() {
        displayListener?.let { displays?.unregisterDisplayListener(it) }
        displayListener = null
        audioReceiver?.unregister()
        audioReceiver = null
    }
}
