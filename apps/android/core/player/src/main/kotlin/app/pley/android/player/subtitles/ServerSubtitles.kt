package app.pley.android.player.subtitles

import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.extractor.text.SubtitleParser
import androidx.media3.extractor.text.webvtt.WebvttParser
import app.pley.android.network.ApiException
import app.pley.android.network.HttpPleyApi
import app.pley.android.network.OfflineException
import app.pley.android.network.PleyApi
import app.pley.android.network.SubtitleText
import java.io.File
import java.io.IOException
import java.net.URI
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
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

/** Reads a WebVTT file into timed cues. */
fun interface VttParser {
    fun parse(text: String): List<TimedCues>
}

/** Media3's WebvttParser: styling, positions and line breaks as SubtitleView draws them. */
@OptIn(UnstableApi::class)
object Media3VttParser : VttParser {
    override fun parse(text: String): List<TimedCues> {
        val bytes = text.toByteArray(Charsets.UTF_8)
        val out = mutableListOf<TimedCues>()
        WebvttParser().parse(bytes, 0, bytes.size, SubtitleParser.OutputOptions.allCues()) { cues ->
            val start = cues.startTimeUs / 1000
            val end = cues.endTimeUs / 1000
            if (cues.cues.isNotEmpty() && end > start) out += TimedCues(start, end, cues.cues)
        }
        return out
    }
}

/** Fetches a server subtitle track: [url] is the track's own address (SubtitleTrack.url), made absolute. */
fun interface SubtitleFetcher {
    suspend fun fetch(fileId: Long, index: Int, url: String): SubtitleText
}

/** Through the API (its own client): for tests and servers without a subtitles client. */
class ApiSubtitleFetcher(private val api: PleyApi) : SubtitleFetcher {
    override suspend fun fetch(fileId: Long, index: Int, url: String): SubtitleText =
        SavedSubtitles.read(url) ?: api.subtitles(fileId, index)
}

/** Subtitles saved beside a download (`file:///…/subs/3.vtt`), whole. */
internal object SavedSubtitles {
    /** The track at [url] when it is a saved file, else `null`. */
    suspend fun read(url: String, io: CoroutineDispatcher = Dispatchers.IO): SubtitleText? {
        if (!url.startsWith("file:")) return null
        return withContext(io) { SubtitleText(File(URI(url)).readText(), complete = true) }
    }
}

/**
 * Through the subtitles client (NetworkGraph.subtitles: long reads, as the
 * transcoder may still be writing the track), on OkHttp's threads.
 */
class OkHttpSubtitleFetcher(private val client: () -> OkHttpClient, private val io: CoroutineDispatcher = Dispatchers.IO) : SubtitleFetcher {
    override suspend fun fetch(fileId: Long, index: Int, url: String): SubtitleText {
        SavedSubtitles.read(url, io)?.let { return it }
        val http = withContext(io) { client() }
        val request = Request.Builder().url(url).header("Accept", "text/vtt").build()
        return suspendCancellableCoroutine { continuation ->
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
                                val complete = it.header(HttpPleyApi.SUBTITLES_COMPLETE_HEADER)?.trim() != "0"
                                Result.success(SubtitleText(it.body.string(), complete))
                            }
                        } catch (e: IOException) {
                            Result.failure(e)
                        }
                        continuation.resumeWith(result)
                    }
                },
            )
        }
    }
}

/**
 * The server's WebVTT subtitles for the track shown (HLS sessions, and
 * subtitle files beside the media in direct play). A track still being
 * read out of its file (`X-Pley-Complete: 0`) is asked for again every 3 s
 * until it is whole, each answer merged into [index].
 */
class ServerSubtitles(
    private val scope: CoroutineScope,
    private val fetcher: SubtitleFetcher,
    private val parser: VttParser = Media3VttParser,
    private val parse: CoroutineDispatcher = Dispatchers.Default,
    /** Called when the track can't be loaded (on [scope]). */
    private val onError: (Throwable) -> Unit = {},
) {
    private val state = MutableStateFlow<SubtitleIndex?>(null)

    /** The cues of the track shown; `null` when none is (or it hasn't come yet). */
    val index: StateFlow<SubtitleIndex?> = state.asStateFlow()

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
            var merged: SubtitleIndex? = null
            while (true) {
                val answer = try {
                    fetcher.fetch(fileId, index, url)
                } catch (e: CancellationException) {
                    throw e
                } catch (e: IOException) {
                    // Choosing the track again tries again.
                    if (showing == key) showing = null
                    onError(e)
                    return@launch
                }
                val parsed = withContext(parse) { SubtitleIndex(parser.parse(answer.text)) }
                merged = merged?.merge(parsed) ?: parsed
                state.value = merged
                if (answer.complete) return@launch
                delay(AGAIN_MS)
            }
        }
    }

    /** Shows none. */
    fun hide() {
        job?.cancel()
        job = null
        showing = null
        state.value = null
    }

    companion object {
        /** How often a track still being read out of its file is asked for again. */
        const val AGAIN_MS = 3_000L
    }
}
