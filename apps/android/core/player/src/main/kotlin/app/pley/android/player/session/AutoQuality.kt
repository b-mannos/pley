package app.pley.android.player.session

import android.content.Context
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import kotlin.math.min

/** Why Automatic plays below the original. */
enum class LoweredFor {
    /** The video didn't arrive fast enough. */
    Network,

    /** The device dropped frames: it fell behind decoding. */
    Device,
}

/**
 * What Automatic plays a file at, as docs/QUALITY.md says: the file's
 * offered qualities ([Qualities.forFile]), what the original plays at
 * ([originalKbps]: the file's bitrate, or the server's quality when it
 * converts the original), its lines and the server's own quality for it
 * ([cap]: the highest Automatic ever goes below the original).
 */
data class AutoFile(
    val offered: List<QualityPreset>,
    val originalKbps: Int?,
    val lines: Int?,
    val cap: QualityPreset,
)

/** One look at the engine, about every half second. */
data class AutoSample(
    val atMs: Long,
    /**
     * The video is meant to play and shows: not paused, not ended, not
     * waiting for a server out of reach, not in the background.
     */
    val watching: Boolean,
    /** The engine waits for data. */
    val buffering: Boolean,
    val positionMs: Long,
    /** How far the media is loaded. */
    val bufferedMs: Long,
    /** 0 while unknown. */
    val durationMs: Long,
    /** The network's speed as the engine measured it from what it downloaded; `null` when it can't tell. */
    val bandwidthKbps: Long?,
)

/** A change Automatic asks for: play [to] from where it is. */
data class AutoStep(
    val to: QualityPreset,
    /** Why it went down; `null` for a step back up. */
    val reason: LoweredFor?,
) {
    val up: Boolean get() = reason == null
}

/**
 * The network's ceiling found last, per device and server, so the next
 * title (Up Next included) starts at what the network carried.
 */
interface CeilingMemory {
    /** The ceiling in kbps and when it was found (on [AutoQuality]'s clock); `null` for none. */
    fun read(): Pair<Int, Long>?

    fun write(kbps: Int, atMs: Long)

    fun forget()

    companion object {
        /** Kept in memory only (tests, previews). */
        fun memory(): CeilingMemory = object : CeilingMemory {
            private var held: Pair<Int, Long>? = null

            override fun read(): Pair<Int, Long>? = held

            override fun write(kbps: Int, atMs: Long) {
                held = kbps to atMs
            }

            override fun forget() {
                held = null
            }
        }

    }
}

/**
 * The ceilings of every server, in the app's preferences: read once off the
 * main thread when made (pressing play never waits on the disk), held in
 * memory, and written behind. The clock is elapsedRealtime: a ceiling from
 * before the device restarted reads as from the future, and is forgotten.
 */
class PreferencesCeilings(context: Context) {
    private val app = context.applicationContext
    private val held = ConcurrentHashMap<String, Pair<Int, Long>>()
    private val disk = Executors.newSingleThreadExecutor { r -> Thread(r, "pley-ceilings").apply { isDaemon = true } }
    private val prefs by lazy { app.getSharedPreferences(PREFS, Context.MODE_PRIVATE) }

    init {
        disk.execute {
            for ((key, value) in prefs.all) {
                val server = key.removeSuffix(KBPS)
                if (key.endsWith(KBPS) && value is Int) held.putIfAbsent(server, value to prefs.getLong(server + AT, 0))
            }
        }
    }

    /** Waits for what was asked of the disk so far (tests). */
    internal fun settle() {
        disk.submit {}.get()
    }

    /** The ceiling of the server at [server] (its address). */
    fun forServer(server: String): CeilingMemory = object : CeilingMemory {
        override fun read(): Pair<Int, Long>? = held[server]

        override fun write(kbps: Int, atMs: Long) {
            held[server] = kbps to atMs
            disk.execute { prefs.edit().putInt(server + KBPS, kbps).putLong(server + AT, atMs).apply() }
        }

        override fun forget() {
            held.remove(server)
            disk.execute { prefs.edit().remove(server + KBPS).remove(server + AT).apply() }
        }
    }

    private companion object {
        const val PREFS = "auto_quality"
        const val KBPS = "|kbps"
        const val AT = "|at"
    }
}

/**
 * Automatic quality's decisions (docs/QUALITY.md), apart from the player
 * so they can be tested on their own: the original unless it doesn't play
 * well, then the quality that does.
 *
 * - [begin] says where a file starts: the original, or at most the
 *   server's ceiling for this network ([StartCeiling.Server]); when the
 *   server couldn't say, the quality the network carried when it was found
 *   too slow on this device in the last 30 minutes.
 * - [sample] watches rebuffering and delivery while the video plays, and
 *   asks to step down on a play issue, or back up after calm minutes.
 * - It settles: a step up that a step down undoes within 10 minutes isn't
 *   tried again during the title, and each one doubles the calm the next
 *   step up waits for (3, 6, then 12 minutes).
 * - [deviceFellBehind] is dropped frames at a transcode; at the original,
 *   the recovery ladder switches to the server's quality and tells
 *   [deviceFellBehindAtOriginal].
 * - [applied] is called once the player plays what was asked.
 *
 * Seeks, pauses, the buffering after a start or a seek, and a new session
 * ([jumped]) are never play issues. Not thread-safe: the main thread only.
 */
class AutoQuality(
    private val memory: CeilingMemory,
    /** The remembered ceiling changed (kbps, `null`: forgotten): the server is told ([NetworkStart.report]). */
    private val onCeiling: (Int?) -> Unit = {},
) {
    private var file: AutoFile? = null

    /** What plays: [Qualities.ORIGINAL] or one of the ladder's. */
    var playing: QualityPreset = Qualities.ORIGINAL
        private set

    /** Why it plays below the original; `null` at the original. */
    var loweredFor: LoweredFor? = null
        private set

    /** The device fell behind here: steps back up stop below it. */
    private var deviceTop: Int? = null
    private var originalBlocked = false
    private var originalConverted = false

    private var lastChangeAt: Long? = null
    private var lastDownAt: Long? = null

    // Rebuffering.
    private var settled = false
    private var waitFrom: Long? = null
    private var waitCounted = false
    private val shortWaits = ArrayDeque<Long>()

    // Delivery: (time, buffered, playhead) over the last 20 s of watching.
    private val deliveries = ArrayDeque<Triple<Long, Long, Long>>()
    private var lowSince: Long? = null

    // Calm: watched time without an issue, and the network measured meanwhile.
    private var calmMs = 0L
    private var lastWatchedAt: Long? = null
    private val bandwidths = ArrayDeque<Pair<Long, Long>>()
    private var lastBandwidth: Long? = null

    // Settling: the last step up (to, when), until a step down or [SETTLE_MS] says whether it held…
    private var lastUp: Pair<QualityPreset, Long>? = null

    /** …the qualities a step down undid soon after a step up to them, not tried again this title… */
    private val tooHigh = mutableSetOf<String>()

    /** …and the calm a step up waits for: [CALM_MS], doubled by each step up undone, up to [MAX_CALM_MS]. */
    private var calmNeededMs = CALM_MS

    /**
     * A new file in Automatic (a new title, Up Next's, or Automatic picked
     * in the menu): it starts afresh, at the original, or at most at the
     * ceiling when that is below the file's bitrate: the server's for this
     * network when it said ([start]), else the one remembered here.
     */
    fun begin(file: AutoFile, nowMs: Long, start: StartCeiling = StartCeiling.Device): QualityPreset {
        this.file = file
        deviceTop = null
        originalBlocked = false
        originalConverted = false
        lastDownAt = null
        lastChangeAt = null
        resetSettling()
        resetWatch()
        resetCalm()
        val ceiling = if (start is StartCeiling.Server) start.kbps else remembered(nowMs)
        val begins = if (ceiling != null && (file.originalKbps == null || ceiling < file.originalKbps)) {
            ladder().firstOrNull { kbps(it) <= ceiling } ?: ladder().lastOrNull()
        } else {
            null
        }
        playing = begins ?: Qualities.ORIGINAL
        loweredFor = if (begins != null) LoweredFor.Network else null
        if (begins != null) lastChangeAt = nowMs
        return playing
    }

    /** Automatic picked again in the menu: what was remembered of the network goes, here and on the server. */
    fun forgetCeiling() {
        memory.forget()
        onCeiling(null)
    }

    /** A file is in Automatic's hands ([begin]), until [stop]. */
    val active: Boolean get() = file != null

    /** Another quality chosen, or a download playing as it is: nothing is watched. */
    fun stop() {
        file = null
        playing = Qualities.ORIGINAL
        loweredFor = null
        resetSettling()
        resetWatch()
        resetCalm()
    }

    /**
     * The original plays at [kbps] now: the file as it is, or [converted]
     * by the server at its own quality (then dropped frames step down from it).
     */
    fun originalPlaysAt(kbps: Int?, converted: Boolean = false) {
        val f = file ?: return
        originalConverted = converted
        if (kbps != null && kbps != f.originalKbps) file = f.copy(originalKbps = kbps)
    }

    /** A seek, a new session, a track switched: the waits and the delivery count from where it lands. */
    fun jumped() = resetWatch()

    /** Takes one look at the engine: a step to take, or `null`. */
    fun sample(s: AutoSample): AutoStep? {
        if (file == null) return null
        if (!s.watching) {
            // Paused, in the background, or out of reach: nothing counts, the waits start over.
            waitFrom = null
            waitCounted = false
            deliveries.clear()
            lowSince = null
            lastWatchedAt = null
            return null
        }
        val watchedBefore = lastWatchedAt
        lastWatchedAt = s.atMs
        if (!s.buffering && !settled) {
            // The start's or the seek's buffering is over: from here, a wait is a play issue.
            settled = true
        }
        if (!settled) return null
        if (watchedBefore != null) calmMs += s.atMs - watchedBefore
        if (s.bandwidthKbps != null && s.bandwidthKbps > 0) lastBandwidth = s.bandwidthKbps

        rebuffering(s)?.let { return it }
        delivery(s)?.let { return it }

        if (s.bandwidthKbps != null && s.bandwidthKbps > 0) bandwidths.addLast(s.atMs to s.bandwidthKbps)
        while (bandwidths.isNotEmpty() && bandwidths.first().first < s.atMs - calmNeededMs) bandwidths.removeFirst()
        return stepUp(s.atMs)
    }

    private fun rebuffering(s: AutoSample): AutoStep? {
        if (s.buffering) {
            val from = waitFrom ?: s.atMs.also { waitFrom = it }
            if (!waitCounted && s.atMs - from >= LONG_WAIT_MS) {
                // Asked again while it waits, until a step is taken (one may be too soon after the last).
                return issue(s.atMs, LoweredFor.Network)?.also { waitCounted = true }
            }
            return null
        }
        val from = waitFrom ?: return null
        waitFrom = null
        if (waitCounted) {
            waitCounted = false
            return null
        }
        if (s.atMs - from <= 0 || s.atMs - from >= LONG_WAIT_MS) return null
        shortWaits.addLast(s.atMs)
        while (shortWaits.isNotEmpty() && shortWaits.first() < s.atMs - SHORT_WAITS_WINDOW_MS) shortWaits.removeFirst()
        if (shortWaits.size >= SHORT_WAITS) {
            shortWaits.clear()
            return issue(s.atMs, LoweredFor.Network)
        }
        return null
    }

    private fun delivery(s: AutoSample): AutoStep? {
        deliveries.addLast(Triple(s.atMs, s.bufferedMs, s.positionMs))
        while (deliveries.size > 2 && deliveries[1].first <= s.atMs - DELIVERY_WINDOW_MS) deliveries.removeFirst()
        val rate = deliveryRate()
        val (_, firstBuffered, firstPosition) = deliveries.first()
        val ahead = s.bufferedMs - s.positionMs
        val aheadBefore = firstBuffered - firstPosition
        // Loaded to the end: nothing more can arrive, and nothing is late.
        val toTheEnd = s.durationMs > 0 && s.bufferedMs >= s.durationMs - END_MARGIN_MS
        val low = rate != null && !toTheEnd && rate < LOW_DELIVERY && ahead < LOW_AHEAD_MS && ahead < aheadBefore - FALLING_MS
        if (!low) {
            lowSince = null
            return null
        }
        val since = lowSince ?: s.atMs.also { lowSince = it }
        if (s.atMs - since < LOW_FOR_MS) return null
        lowSince = null
        return issue(s.atMs, LoweredFor.Network)
    }

    /** Seconds of video that arrived per second, over the last 20 s; `null` before 5 s of them. */
    private fun deliveryRate(): Double? {
        if (deliveries.size < 2) return null
        val (fromAt, fromBuffered, _) = deliveries.first()
        val (toAt, toBuffered, _) = deliveries.last()
        val span = toAt - fromAt
        if (span < MIN_DELIVERY_SPAN_MS) return null
        return (toBuffered - fromBuffered).coerceAtLeast(0).toDouble() / span
    }

    /**
     * The device dropped frames at a transcode: a step down, to the next
     * quality of the ladder (fewer lines first), and no step back up to it.
     */
    fun deviceFellBehind(nowMs: Long): AutoStep? {
        // The original as it is: the recovery ladder's switch, as for every quality.
        if ((playing.original && !originalBlocked && !originalConverted) || file == null) return null
        return issue(nowMs, LoweredFor.Device)
    }

    /**
     * The device dropped frames at the original, and the recovery ladder
     * switched to the server's quality ([convertedKbps]): that counts as
     * this file's step down, and Automatic never steps back up to the original.
     */
    fun deviceFellBehindAtOriginal(nowMs: Long, convertedKbps: Int?) {
        if (file == null) return
        wentDown(nowMs)
        originalBlocked = true
        originalPlaysAt(convertedKbps, converted = true)
        loweredFor = LoweredFor.Device
        lastChangeAt = nowMs
        lastDownAt = nowMs
        resetCalm()
    }

    /** The server can't convert as fast as the video plays: a play issue like the network's. */
    fun serverTooSlow(nowMs: Long): AutoStep? = if (file == null) null else issue(nowMs, LoweredFor.Network)

    /** The player plays [step] now. */
    fun applied(step: AutoStep, nowMs: Long) {
        if (step.up) lastUp = step.to to nowMs else wentDown(nowMs)
        playing = step.to
        lastChangeAt = nowMs
        if (step.up) {
            if (step.to.original) {
                loweredFor = null
                memory.forget()
                onCeiling(null)
            } else {
                memory.write(kbps(step.to), nowMs)
                onCeiling(kbps(step.to))
            }
        } else {
            lastDownAt = nowMs
            loweredFor = step.reason
        }
        resetWatch()
        resetCalm()
    }

    /**
     * A step down from what plays: from where a step up went less than
     * [SETTLE_MS] ago, that step was one too far. It isn't tried again this
     * title, and the calm before any other step up doubles.
     */
    private fun wentDown(nowMs: Long) {
        val (to, at) = lastUp ?: return
        lastUp = null
        if (to.id != playing.id || nowMs - at >= SETTLE_MS) return
        tooHigh += to.id
        calmNeededMs = min(MAX_CALM_MS, calmNeededMs * 2)
    }

    private fun issue(nowMs: Long, reason: LoweredFor): AutoStep? {
        resetCalm()
        val last = lastChangeAt
        if (last != null && nowMs - last < CHANGE_GAP_MS) return null
        val ladder = ladder()
        val current = playingKbps()
        val target = if (reason == LoweredFor.Device) {
            val next = if (playing.original) {
                ladder.firstOrNull { current == null || kbps(it) < current }
            } else {
                ladder.getOrNull(ladder.indexOf(playing) + 1)
            }
            next?.also { deviceTop = ladder.indexOf(it) }
        } else {
            // Too early to measure delivery: the network's speed for this bitrate, else as much as plays.
            val rate = deliveryRate() ?: current?.let { c -> lastBandwidth?.let { (it.toDouble() / c).coerceAtMost(1.0) } } ?: 1.0
            val ceiling = current?.let { it * rate * SAFETY }
            val lower = ladder.filter { current == null || kbps(it) < current }
            val pick = lower.firstOrNull { ceiling == null || kbps(it) <= ceiling } ?: lower.lastOrNull()
            pick?.also {
                val kept = min(ceiling?.toInt() ?: kbps(it), kbps(it)).coerceAtLeast(kbps(Qualities.LOWEST))
                memory.write(kept, nowMs)
                onCeiling(kept)
            }
        }
        return target?.let { AutoStep(it, reason) }
    }

    private fun stepUp(nowMs: Long): AutoStep? {
        if (playing.original || calmMs < calmNeededMs) return null
        lastDownAt?.let { if (nowMs - it < AFTER_DOWN_MS) return null }
        lastChangeAt?.let { if (nowMs - it < calmNeededMs) return null }
        val ladder = ladder()
        val at = ladder.indexOf(playing)
        val next = if (at <= 0) Qualities.ORIGINAL else ladder[at - 1]
        // Settled: the quality above didn't hold this title.
        if (next.id in tooHigh) return null
        if (next.original && originalBlocked) return null
        deviceTop?.let { top -> if (!next.original && ladder.indexOf(next) < top) return null }
        if (next.original && deviceTop != null) return null
        val need = if (next.original) file?.originalKbps ?: return null else kbps(next)
        // A network it can't measure: it stays where it went down to.
        if (bandwidths.isEmpty() || bandwidths.first().first > nowMs - calmNeededMs + MEASURE_SLACK_MS) return null
        val measured = bandwidths.minOf { it.second }
        if (measured < need * HEADROOM) return null
        return AutoStep(next, reason = null)
    }

    /** The bitrate playing now, in kbps; `null` for an original of unknown bitrate. */
    private fun playingKbps(): Int? = if (playing.original) file?.originalKbps ?: file?.cap?.let(::kbps) else kbps(playing)

    /**
     * The qualities Automatic plays below the original, highest first: the
     * file's offered transcodes up to the server's own quality for it, by
     * the lines they give this file, then by bitrate.
     */
    fun ladder(): List<QualityPreset> {
        val f = file ?: return emptyList()
        fun lines(q: QualityPreset): Int = min(q.maxHeight ?: Int.MAX_VALUE, f.lines ?: Int.MAX_VALUE)
        val capLines = lines(f.cap)
        val capKbps = kbps(f.cap)
        return f.offered
            .filter { it.maxBitrateKbps != null && kbps(it) <= capKbps && lines(it) <= capLines }
            .sortedWith(compareByDescending<QualityPreset> { lines(it) }.thenByDescending { kbps(it) })
    }

    private fun remembered(nowMs: Long): Int? {
        val (kbps, at) = memory.read() ?: return null
        if (at > nowMs || nowMs - at > MEMORY_MS) {
            memory.forget()
            return null
        }
        return kbps
    }

    private fun resetWatch() {
        settled = false
        waitFrom = null
        waitCounted = false
        shortWaits.clear()
        deliveries.clear()
        lowSince = null
        lastWatchedAt = null
    }

    private fun resetSettling() {
        lastUp = null
        tooHigh.clear()
        calmNeededMs = CALM_MS
    }

    private fun resetCalm() {
        calmMs = 0
        bandwidths.clear()
    }

    private fun kbps(q: QualityPreset): Int = q.maxBitrateKbps ?: 0

    companion object {
        /** A wait this long is a play issue… */
        const val LONG_WAIT_MS = 2_000L

        /** …and so are two shorter ones within a minute. */
        const val SHORT_WAITS = 2
        const val SHORT_WAITS_WINDOW_MS = 60_000L

        /** Delivery is measured over this long… */
        const val DELIVERY_WINDOW_MS = 20_000L
        const val MIN_DELIVERY_SPAN_MS = 5_000L

        /** …and is a play issue under this for [LOW_FOR_MS], with less than [LOW_AHEAD_MS] ahead and falling. */
        const val LOW_DELIVERY = 1.1
        const val LOW_FOR_MS = 15_000L
        const val LOW_AHEAD_MS = 10_000L

        /** What ahead must have lost over the window to be falling. */
        const val FALLING_MS = 1_000L
        const val END_MARGIN_MS = 1_000L

        /** The share of what the network carried that a step down aims for. */
        const val SAFETY = 0.75

        /** The least time between two changes. */
        const val CHANGE_GAP_MS = 20_000L

        /** Watched without a play issue this long, it may step back up… */
        const val CALM_MS = 180_000L

        /** …but not this soon after a step down… */
        const val AFTER_DOWN_MS = 300_000L

        /** …and when the network carries this times the next quality up. */
        const val HEADROOM = 1.6

        /** A step up that a step down undoes this soon was one too far: it isn't tried again this title. */
        const val SETTLE_MS = 10 * 60_000L

        /** Each step up undone doubles the calm the next one waits for, up to this. */
        const val MAX_CALM_MS = 4 * CALM_MS

        /** The measurements must cover the calm minutes, give or take this. */
        const val MEASURE_SLACK_MS = 5_000L

        /** How long the ceiling is remembered. */
        const val MEMORY_MS = 30 * 60_000L
    }
}
