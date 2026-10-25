package app.pley.android.player.music

import androidx.annotation.OptIn
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi

/**
 * The player the media session shows (notification, lock screen, the
 * system's media controls, Bluetooth buttons): the output's one-track
 * player, with Previous and Next going through [playback]'s queue, and
 * Play after the queue's end starting its last track again.
 */
@OptIn(markerClass = [UnstableApi::class])
internal class QueuePlayer(player: Player, private val playback: MusicPlayback) : ForwardingPlayer(player) {
    override fun getAvailableCommands(): Player.Commands = super.getAvailableCommands().buildUpon()
        .addAll(*QUEUE_COMMANDS)
        .build()

    override fun isCommandAvailable(command: Int): Boolean = command in QUEUE_COMMANDS || super.isCommandAvailable(command)

    override fun play() = playback.play()

    override fun pause() = playback.pause()

    override fun setPlayWhenReady(playWhenReady: Boolean) {
        if (playWhenReady) playback.play() else playback.pause()
    }

    override fun seekToNext() = playback.next()

    override fun seekToNextMediaItem() = playback.next()

    override fun seekToPrevious() = playback.previous()

    override fun seekToPreviousMediaItem() = playback.previous()

    override fun hasNextMediaItem(): Boolean = playback.queueState.value.let { it.index + 1 < it.tracks.size }

    override fun hasPreviousMediaItem(): Boolean = playback.queueState.value.tracks.isNotEmpty()

    override fun stop() = playback.stop()

    private companion object {
        val QUEUE_COMMANDS = intArrayOf(
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
            Player.COMMAND_PLAY_PAUSE,
        )
    }
}
