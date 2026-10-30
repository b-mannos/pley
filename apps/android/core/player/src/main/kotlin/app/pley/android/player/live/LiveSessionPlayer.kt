package app.pley.android.player.live

import androidx.annotation.OptIn
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi

/**
 * The live player as its media session shows it: Live TV has no pause and
 * no going back, so remotes' media keys, headsets and assistants can't
 * pause, seek or skip it.
 */
@OptIn(markerClass = [UnstableApi::class])
class LiveSessionPlayer(player: Player) : ForwardingPlayer(player) {
    override fun getAvailableCommands(): Player.Commands = super.getAvailableCommands().buildUpon().removeAll(*NOT_LIVE).build()

    override fun isCommandAvailable(command: Int): Boolean = command !in NOT_LIVE && super.isCommandAvailable(command)

    private companion object {
        val NOT_LIVE = intArrayOf(
            Player.COMMAND_PLAY_PAUSE,
            Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_DEFAULT_POSITION,
            Player.COMMAND_SEEK_BACK,
            Player.COMMAND_SEEK_FORWARD,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_MEDIA_ITEM,
        )
    }
}
