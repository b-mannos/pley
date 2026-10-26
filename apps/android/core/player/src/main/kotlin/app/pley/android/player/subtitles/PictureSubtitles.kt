package app.pley.android.player.subtitles

import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.DataReader
import androidx.media3.common.Format
import androidx.media3.common.text.Cue
import androidx.media3.common.util.ParsableByteArray
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.DefaultExtractorInput
import androidx.media3.extractor.DiscardingTrackOutput
import androidx.media3.extractor.Extractor
import androidx.media3.extractor.ExtractorOutput
import androidx.media3.extractor.PositionHolder
import androidx.media3.extractor.SeekMap
import androidx.media3.extractor.TrackOutput
import androidx.media3.extractor.mkv.MatroskaExtractor
import androidx.media3.extractor.text.DefaultSubtitleParserFactory
import androidx.media3.extractor.text.SubtitleParser
import app.pley.android.network.ApiException
import app.pley.android.network.HttpPleyApi
import app.pley.android.network.OfflineException
import java.io.Closeable
import java.io.EOFException
import java.io.File
import java.io.IOException
import java.io.RandomAccessFile
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response

/** Where one sample of a picture track starts showing, and where its bytes are kept. */
internal data class PictureSample(val timeMs: Long, val offset: Long, val size: Int)

/** A picture track's sample bytes, in a file of the app's cache: a film's PGS is tens of MB. */
class SampleStore(private val file: File) : Closeable {
    private val raf = RandomAccessFile(file, "rw")

    /** Adds [length] bytes of [bytes] from [from]; where they start. */
    @Synchronized
    fun append(bytes: ByteArray, from: Int, length: Int): Long {
        val at = raf.length()
        raf.seek(at)
        raf.write(bytes, from, length)
        return at
    }

    @Synchronized
    fun read(offset: Long, size: Int): ByteArray {
        val out = ByteArray(size)
        raf.seek(offset)
        raf.readFully(out)
        return out
    }

    @Synchronized
    override fun close() {
        runCatching { raf.close() }
        file.delete()
    }
}

/**
 * A subtitle track shown as pictures (PGS, VobSub, DVB), read out of the
 * server's .mks: its samples by time, each decoded (Media3's own parsers)
 * only when it shows, on [work]. A sample shows until the next one, or for
 * as long as it says; an empty one clears the screen.
 */
@OptIn(UnstableApi::class)
class PictureTrack internal constructor(
    val format: Format,
    private val samples: List<PictureSample>,
    private val store: SampleStore,
    private val work: CoroutineDispatcher = Dispatchers.Default,
) : TimedCueTrack, Closeable {
    private val parser: SubtitleParser by lazy { DefaultSubtitleParserFactory().create(format) }

    /** The sample decoded last: its index, cues and end (`null`: until the next one). */
    private var decodedIndex = -1
    private var decodedCues: List<Cue> = emptyList()
    private var decodedEndMs: Long? = null

    /** Samples in all. */
    val size: Int get() = samples.size

    override suspend fun cuesAt(positionMs: Long): List<Cue> {
        val i = indexAt(positionMs)
        if (i < 0) return emptyList()
        if (i != decodedIndex) withContext(work) { decode(i) }
        val end = decodedEndMs
        return if (end != null && positionMs >= end) emptyList() else decodedCues
    }

    override fun nextChangeAfter(positionMs: Long): Long? {
        val i = indexAt(positionMs)
        val next = samples.getOrNull(i + 1)?.timeMs
        val end = if (i >= 0 && i == decodedIndex) decodedEndMs?.takeIf { it > positionMs } else null
        return listOfNotNull(next, end).minOrNull()
    }

    /** The last sample starting at or before [positionMs]; -1 before the first. */
    private fun indexAt(positionMs: Long): Int {
        var low = 0
        var high = samples.size - 1
        var found = -1
        while (low <= high) {
            val mid = (low + high) ushr 1
            if (samples[mid].timeMs <= positionMs) {
                found = mid
                low = mid + 1
            } else {
                high = mid - 1
            }
        }
        return found
    }

    private fun decode(i: Int) {
        val sample = samples[i]
        var cues: List<Cue> = emptyList()
        var end: Long? = null
        try {
            val bytes = store.read(sample.offset, sample.size)
            // Out of order (a seek): DVB's parser keeps pages and regions
            // from sample to sample, which belong to another place now.
            if (i != decodedIndex + 1) parser.reset()
            parser.parse(bytes, 0, bytes.size, SubtitleParser.OutputOptions.allCues()) { timed ->
                cues = timed.cues
                if (timed.durationUs != C.TIME_UNSET) {
                    val start = if (timed.startTimeUs == C.TIME_UNSET) 0 else timed.startTimeUs / 1000
                    end = sample.timeMs + start + timed.durationUs / 1000
                }
            }
        } catch (_: IOException) {
            // Closed meanwhile (another answer came): nothing shows.
        } catch (_: RuntimeException) {
            // A sample the parser can't read shows nothing.
        }
        decodedIndex = i
        decodedCues = cues
        decodedEndMs = end
    }

    override fun close() = store.close()

    companion object {
        /**
         * Reads the first subtitle track of the Matroska file [mks] with
         * Media3's extractor, its samples' bytes going to [store]. Throws
         * [IOException] for a file that holds none.
         */
        fun read(mks: File, store: SampleStore, work: CoroutineDispatcher = Dispatchers.Default): PictureTrack {
            val output = Collector(store)
            val extractor = MatroskaExtractor()
            extractor.init(output)
            try {
                RandomAccessFile(mks, "r").use { raf ->
                    val length = raf.length()
                    val holder = PositionHolder()
                    var position = 0L
                    while (true) {
                        raf.seek(position)
                        val input = DefaultExtractorInput(FileReader(raf), position, length)
                        var result: Int
                        do {
                            result = extractor.read(input, holder)
                        } while (result == Extractor.RESULT_CONTINUE)
                        if (result != Extractor.RESULT_SEEK) break
                        position = holder.position
                    }
                }
            } finally {
                extractor.release()
            }
            val format = output.track.format ?: throw IOException("no subtitle track in the .mks")
            return PictureTrack(format, output.track.samples.sortedBy { it.timeMs }, store, work)
        }
    }
}

/** A [RandomAccessFile] read from where it is. */
@OptIn(UnstableApi::class)
private class FileReader(private val raf: RandomAccessFile) : DataReader {
    override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
        if (length == 0) return 0
        val read = raf.read(buffer, offset, length)
        return if (read < 0) C.RESULT_END_OF_INPUT else read
    }
}

/** The extractor's output: the first text track kept, the rest dropped. */
@OptIn(UnstableApi::class)
private class Collector(store: SampleStore) : ExtractorOutput {
    val track = SampleCollector(store)
    private var taken = false

    override fun track(id: Int, type: Int): TrackOutput {
        if (type != C.TRACK_TYPE_TEXT || taken) return DiscardingTrackOutput()
        taken = true
        return track
    }

    override fun endTracks() = Unit

    override fun seekMap(seekMap: SeekMap) = Unit
}

/** A track's format and samples: each sample's bytes are the last ones written, [TrackOutput]'s way. */
@OptIn(UnstableApi::class)
private class SampleCollector(private val store: SampleStore) : TrackOutput {
    var format: Format? = null
        private set
    val samples = mutableListOf<PictureSample>()
    private var pending = ByteArray(INITIAL)
    private var size = 0

    override fun format(format: Format) {
        this.format = format
    }

    override fun sampleData(input: DataReader, length: Int, allowEndOfInput: Boolean, sampleDataPart: Int): Int {
        room(length)
        val read = input.read(pending, size, length)
        if (read == C.RESULT_END_OF_INPUT) {
            if (allowEndOfInput) return C.RESULT_END_OF_INPUT
            throw EOFException()
        }
        size += read
        return read
    }

    override fun sampleData(data: ParsableByteArray, length: Int, sampleDataPart: Int) {
        room(length)
        data.readBytes(pending, size, length)
        size += length
    }

    override fun sampleMetadata(timeUs: Long, flags: Int, size: Int, offset: Int, cryptoData: TrackOutput.CryptoData?) {
        val start = this.size - offset - size
        if (start < 0 || size <= 0) return
        val at = store.append(pending, start, size)
        samples += PictureSample(timeUs / 1000, at, size)
        // What follows the sample belongs to the next one.
        System.arraycopy(pending, this.size - offset, pending, 0, offset)
        this.size = offset
    }

    private fun room(more: Int) {
        if (size + more <= pending.size) return
        pending = pending.copyOf(maxOf(size + more, pending.size * 2))
    }

    private companion object {
        const val INITIAL = 64 * 1024
    }
}

/** A picture track as the server answered it: whole, or still being read out of its file (`X-Pley-Complete: 0`). */
class PictureLoad(val track: PictureTrack, val complete: Boolean)

/** Fetches and reads a picture subtitle track: [url] is the track's .mks, made absolute. */
fun interface PictureSource {
    suspend fun load(fileId: Long, index: Int, url: String): PictureLoad
}

/**
 * Through the subtitles client (long reads: the server may still be
 * copying the track out of its file), into [dir] in the app's cache; read
 * on [io]. The .mks goes once read; the samples stay until the track does.
 */
class OkHttpPictureSource(
    private val client: () -> OkHttpClient,
    private val dir: File,
    private val io: CoroutineDispatcher = Dispatchers.IO,
    private val work: CoroutineDispatcher = Dispatchers.Default,
) : PictureSource {
    override suspend fun load(fileId: Long, index: Int, url: String): PictureLoad {
        val http = withContext(io) {
            // What an earlier process left (it was killed with a track showing) goes first.
            if (swept.compareAndSet(false, true)) dir.listFiles()?.forEach { it.delete() }
            client()
        }
        val n = counter.incrementAndGet()
        val mks = File(dir, "$fileId-$index-$n.mks")
        val samples = File(dir, "$fileId-$index-$n.samples")
        val request = Request.Builder().url(url).header("Accept", "video/x-matroska").build()
        val complete = suspendCancellableCoroutine { continuation ->
            val call = http.newCall(request)
            continuation.invokeOnCancellation { call.cancel() }
            call.enqueue(
                object : Callback {
                    override fun onFailure(call: Call, e: IOException) {
                        continuation.resumeWith(Result.failure(OfflineException(e)))
                    }

                    override fun onResponse(call: Call, response: Response) {
                        val result = try {
                            response.use {
                                if (!it.isSuccessful) throw ApiException.from(it)
                                dir.mkdirs()
                                mks.outputStream().use { out -> it.body.byteStream().copyTo(out) }
                                Result.success(it.header(HttpPleyApi.SUBTITLES_COMPLETE_HEADER)?.trim() != "0")
                            }
                        } catch (e: IOException) {
                            mks.delete()
                            Result.failure(e)
                        }
                        continuation.resumeWith(result)
                    }
                },
            )
        }
        // Read to the end even when cancelled meanwhile (the read can't be
        // interrupted): a result thrown away would keep its samples open.
        val loaded = withContext(io + NonCancellable) {
            val store = SampleStore(samples)
            try {
                PictureLoad(PictureTrack.read(mks, store, work), complete)
            } catch (e: Exception) {
                store.close()
                throw if (e is IOException) e else IOException(e)
            } finally {
                mks.delete()
            }
        }
        try {
            currentCoroutineContext().ensureActive()
        } catch (e: CancellationException) {
            loaded.track.close()
            throw e
        }
        return loaded
    }

    private companion object {
        val counter = AtomicInteger()
        val swept = java.util.concurrent.atomic.AtomicBoolean(false)
    }
}

/**
 * The picture subtitles shown, side-loaded beside a stream (capabilities
 * v2 `picture_subtitles_sideload`): the server's .mks of the track, read
 * into [track]. A track still being copied out of its file is asked for
 * again every 3 s until it is whole, as [ServerSubtitles] does.
 */
class PictureSubtitles(
    private val scope: CoroutineScope,
    private val source: PictureSource,
    /** Called when the track can't be loaded (on [scope]). */
    private val onError: (Throwable) -> Unit = {},
) {
    private val state = MutableStateFlow<PictureTrack?>(null)

    /** The track shown; `null` when none is (or it hasn't come yet). */
    val track: StateFlow<PictureTrack?> = state.asStateFlow()

    private var job: Job? = null

    /** Which track shows: `fileId:index`, or `null`. */
    var showing: String? = null
        private set

    /** Shows track [index] of file [fileId], at [url]; the same track again changes nothing (unless it failed). */
    fun show(fileId: Long, index: Int, url: String) {
        val key = "$fileId:$index"
        if (key == showing) return
        hide()
        showing = key
        job = scope.launch {
            while (true) {
                val answer = try {
                    source.load(fileId, index, url)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: IOException) {
                    if (showing == key) showing = null
                    onError(e)
                    return@launch
                }
                if (showing != key) {
                    answer.track.close()
                    return@launch
                }
                // A later answer holds everything found so far: it replaces the one before.
                val before = state.value
                state.value = answer.track
                before?.close()
                if (answer.complete) return@launch
                delay(ServerSubtitles.AGAIN_MS)
            }
        }
    }

    /** Shows none, and lets go of the track's files. */
    fun hide() {
        job?.cancel()
        job = null
        showing = null
        state.value?.close()
        state.value = null
    }
}
