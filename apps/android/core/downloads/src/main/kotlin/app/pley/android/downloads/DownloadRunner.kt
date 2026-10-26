package app.pley.android.downloads

import app.pley.android.data.DownloadState
import app.pley.android.model.ConversionState
import app.pley.android.model.ItemDetail
import app.pley.android.model.PlaybackRequest
import app.pley.android.model.PleyJson
import app.pley.android.model.StreamKind
import app.pley.android.model.caps.ClientCaps
import app.pley.android.network.ApiException
import app.pley.android.network.HttpPleyApi
import app.pley.android.network.PleyApi
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import kotlin.coroutines.resumeWithException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response

/**
 * What a download needs of a server: its API, and clients for the file,
 * subtitles and pictures; and what this device plays, for a converted one.
 */
internal interface DownloadSources {
    fun api(server: String, userId: Long): PleyApi

    fun media(server: String): OkHttpClient

    fun subtitles(server: String): OkHttpClient

    val images: OkHttpClient

    /** This device's capabilities, as a playback sends them; `null` while unknown. */
    suspend fun caps(): ClientCaps? = null
}

/** The server's converted file went (it keeps them a week): it is asked for again. */
private class ConversionGone : IOException("the converted file is gone")

/** A download that can't go on by itself: [message] says why, for the list. */
internal class DownloadFailure(message: String) : Exception(message)

/**
 * Fetches downloads one at a time, oldest first: first the item's
 * details, a picture and its text subtitles as WebVTT, then the file
 * itself, picked up where it stopped (Range) after a lost network or a
 * restart. A download the server converts for this device is asked for,
 * then left while the server makes it (the others go on meanwhile), and
 * looked at again every few seconds until it is whole. A network
 * downloads may not use, or none, makes it wait; errors the server gives
 * fail the download until it is retried.
 */
internal class DownloadRunner(
    private val store: DownloadStore,
    private val sources: DownloadSources,
    private val gate: NetworkGate,
    /** Whether mobile data may be used now (Settings.downloadOnMobileData). */
    private val mobileAllowed: suspend () -> Boolean,
    private val io: CoroutineDispatcher = Dispatchers.IO,
    private val clock: () -> Long = System::currentTimeMillis,
    /** How often a conversion the server is making is looked at. */
    private val conversionPollMs: Long = CONVERSION_POLL_MS,
) {
    @Volatile
    private var current: Pair<Key, Job>? = null

    /** Stops the download of [key] if it is the one running (it was removed). */
    fun cancel(key: Key) {
        current?.takeIf { it.first == key }?.second?.cancel()
    }

    /**
     * Runs downloads until none is left to do, or none can go on: no
     * network they may use for [waitMs]. Returns whether some still wait.
     */
    suspend fun run(waitMs: Long = WAIT_MS): Boolean {
        var failures = 0
        while (true) {
            currentCoroutineContext().ensureActive()
            val pending = store.all.value.filter { it.state == DownloadState.Queued || it.state == DownloadState.Waiting }
            if (pending.isEmpty()) return false
            val now = clock()
            val next = pending.firstOrNull { it.notBefore <= now }
            if (next == null) {
                // Every one left is being converted by the server: look again soon.
                delay((pending.minOf { it.notBefore } - now).coerceIn(0, conversionPollMs))
                continue
            }
            val mobile = mobileAllowed()
            if (!gate.open(mobile)) {
                markWaiting()
                if (!gate.await(mobile, waitMs)) return true
                continue
            }
            val outcome = runOne(next)
            when (outcome) {
                Outcome.Done, Outcome.Removed, Outcome.Converting -> failures = 0
                Outcome.Failed -> failures = 0
                Outcome.Lost -> {
                    // The network went, or the server: wait a little longer each time.
                    failures++
                    if (failures > MAX_LOST_IN_A_ROW) return true
                    delay(BACKOFF_MS * failures)
                }
            }
        }
    }

    private enum class Outcome { Done, Removed, Failed, Lost, Converting }

    private suspend fun runOne(saved: Saved): Outcome {
        val key = saved.key
        store.update(key) { it.copy(state = DownloadState.Downloading, error = null) } ?: return Outcome.Removed
        val job = Job(currentCoroutineContext()[Job])
        current = key to job
        return try {
            withContext(job) {
                var s = store.get(key) ?: return@withContext Outcome.Removed
                if (!s.extrasSaved) s = saveExtras(s)
                if (s.convert && s.mediaUrl == null) s = converted(s) ?: return@withContext Outcome.Converting
                fetchMedia(s)
                store.update(key) { it.copy(state = DownloadState.Done, downloadedBytes = it.sizeBytes, error = null) }
                Outcome.Done
            }
        } catch (e: CancellationException) {
            currentCoroutineContext().ensureActive()
            // Only this download was stopped: it was removed.
            Outcome.Removed
        } catch (e: DownloadFailure) {
            store.update(key) { it.copy(state = DownloadState.Failed, error = e.message) }
            Outcome.Failed
        } catch (e: ApiException) {
            store.update(key) { it.copy(state = DownloadState.Failed, error = messageFor(e)) }
            Outcome.Failed
        } catch (_: ConversionGone) {
            store.update(key) { it.copy(state = DownloadState.Queued, conversion = null, mediaUrl = null, converting = 0f) }
            Outcome.Converting
        } catch (_: IOException) {
            store.update(key) { it.copy(state = DownloadState.Waiting) }
            Outcome.Lost
        } finally {
            job.complete()
            current = null
        }
    }

    private fun markWaiting() {
        store.all.value.filter { it.state == DownloadState.Queued }.forEach { s ->
            store.update(s.key) { it.copy(state = DownloadState.Waiting) }
        }
    }

    /** The details (for playing and listing offline), a picture and the text subtitles, saved beside the file. */
    private suspend fun saveExtras(saved: Saved): Saved {
        val api = sources.api(saved.server, saved.userId)
        val detail = api.item(saved.itemId)
        val dir = store.dirOf(saved).apply { mkdirs() }
        withContext(io) { writeWhole(store.detailOf(saved), PleyJson.encodeToString(ItemDetail.serializer(), detail)) }
        val picture = (detail.thumbUrl ?: detail.posterUrl ?: detail.backdropUrl)?.let { path ->
            try {
                fetch(sources.images, api.url(path, IMAGE_WIDTH), store.imageOf(saved))
                true
            } catch (e: CancellationException) {
                throw e
            } catch (_: IOException) {
                false
            }
        } ?: false
        val file = detail.files.firstOrNull { it.id == saved.fileId }
        val tracks = file?.streams.orEmpty().filter { it.kind == StreamKind.Subtitle && it.textBased == true }
        val subtitles = tracks.mapNotNull { track ->
            try {
                saveSubtitles(saved, api, track.index)
                track.index
            } catch (e: CancellationException) {
                throw e
            } catch (_: IOException) {
                null
            }
        }
        dir.mkdirs()
        return store.update(saved.key) {
            it.copy(
                extrasSaved = true,
                image = picture,
                subtitles = subtitles,
                durationMs = file?.durationMs ?: detail.durationMs ?: it.durationMs,
            )
        } ?: throw CancellationException("removed")
    }

    /** A text track as the server's WebVTT, asked for again while it is still being read out of the file. */
    private suspend fun saveSubtitles(saved: Saved, api: PleyApi, index: Int) {
        val client = sources.subtitles(saved.server)
        val url = api.url("/api/files/${saved.fileId}/subtitles/$index.vtt")
        repeat(SUBTITLE_TRIES) { attempt ->
            val (text, complete) = call(client, Request.Builder().url(url).header("Accept", "text/vtt").build()) { response ->
                if (!response.isSuccessful) throw ApiException.from(response)
                response.body.string() to (response.header(HttpPleyApi.SUBTITLES_COMPLETE_HEADER)?.trim() != "0")
            }
            if (complete || attempt == SUBTITLE_TRIES - 1) {
                withContext(io) { writeWhole(store.subtitleOf(saved, index), text) }
                return
            }
            delay(SUBTITLE_WAIT_MS)
        }
    }

    /**
     * [saved] with its converted file's address, once the server has made
     * it whole (or without converting, when the original plays here after
     * all); `null` while it is being made, looked at again in a while.
     */
    private suspend fun converted(saved: Saved): Saved? {
        val api = sources.api(saved.server, saved.userId)
        val request = PlaybackRequest(
            itemId = saved.itemId,
            fileId = saved.fileId,
            maxHeight = saved.maxHeight,
            maxBitrate = saved.maxBitrateKbps,
            caps = sources.caps() ?: ClientCaps(),
        )
        val info = try {
            saved.conversion?.let { api.conversion(it) } ?: api.convert(request)
        } catch (e: ApiException) {
            when {
                // The server restarted, or forgot it: asked for again.
                e.status == 404 && saved.conversion != null -> api.convert(request)
                // A server that can't convert: the original, unless a smaller file was asked for.
                e.status == 503 && saved.maxHeight == null -> return keep(saved.key) { it.copy(convert = false, converting = null) }
                e.status == 503 -> throw DownloadFailure(NO_CONVERSIONS)
                else -> throw e
            }
        }
        return when (info.state) {
            ConversionState.Direct -> keep(saved.key) { it.copy(convert = false, converting = null) }
            ConversionState.Done -> keep(saved.key) {
                it.copy(conversion = info.id, mediaUrl = info.url, converting = null, sizeBytes = info.size ?: 0)
            }
            ConversionState.Failed -> throw DownloadFailure(info.error?.let { "$CONVERSION_FAILED: $it" } ?: CONVERSION_FAILED)
            else -> {
                keep(saved.key) {
                    it.copy(
                        state = DownloadState.Queued,
                        conversion = info.id,
                        converting = info.progress.toFloat().coerceIn(0f, 1f),
                        notBefore = clock() + conversionPollMs,
                    )
                }
                null
            }
        }
    }

    /** [change] made to [key]'s download, which must still be there. */
    private fun keep(key: Key, change: (Saved) -> Saved): Saved = store.update(key, change = change) ?: throw CancellationException("removed")

    /** The file, from where its partial copy ends, then moved to its name. */
    private suspend fun fetchMedia(saved: Saved) {
        val part = store.partOf(saved)
        val media = store.mediaOf(saved)
        if (media.isFile && saved.sizeBytes > 0 && media.length() == saved.sizeBytes) return
        part.parentFile?.mkdirs()
        var have = if (part.isFile) part.length() else 0L
        val left = (saved.sizeBytes - have).coerceAtLeast(0)
        if (left > 0 && withContext(io) { store.freeBytes() } < left + SPACE_MARGIN) throw DownloadFailure(NO_SPACE)
        val api = sources.api(saved.server, saved.userId)
        val request = Request.Builder()
            .url(api.url(saved.mediaUrl ?: "/api/files/${saved.fileId}/stream"))
            .apply { if (have > 0) header("Range", "bytes=$have-") }
            .build()
        call(sources.media(saved.server), request) { response ->
            when {
                response.code == 416 && saved.sizeBytes > 0 && have >= saved.sizeBytes -> return@call
                response.code == 404 && saved.mediaUrl != null -> {
                    part.delete()
                    throw ConversionGone()
                }
                response.code == 206 -> Unit
                response.isSuccessful -> {
                    // The whole file again: the server didn't take the range.
                    have = 0
                    part.delete()
                }
                else -> throw ApiException.from(response)
            }
            val total = totalOf(response, have)
            if (total > 0 && total != saved.sizeBytes) store.update(saved.key, persist = false) { it.copy(sizeBytes = total) }
            copy(response, part, have, saved.key)
        }
        val size = store.get(saved.key)?.sizeBytes ?: saved.sizeBytes
        if (size > 0 && part.length() != size) throw IOException("the file stopped short")
        withContext(io) {
            media.delete()
            if (!part.renameTo(media)) throw IOException("the file couldn't be put in place")
        }
    }

    /** Appends the body to [part], saying how far it is about twice a second (on [io], as [call] reads). */
    private suspend fun copy(response: Response, part: File, from: Long, key: Key) {
        var have = from
        var shownAt = 0L
        FileOutputStream(part, from > 0).use { out ->
            response.body.byteStream().use { input ->
                val buffer = ByteArray(BUFFER_BYTES)
                while (true) {
                    currentCoroutineContext().ensureActive()
                    val read = input.read(buffer)
                    if (read < 0) break
                    out.write(buffer, 0, read)
                    have += read
                    val now = clock()
                    if (now - shownAt >= PROGRESS_MS) {
                        shownAt = now
                        val done = have
                        store.update(key, persist = false) { it.copy(downloadedBytes = done) }
                    }
                }
            }
        }
        store.update(key, persist = false) { it.copy(downloadedBytes = have) }
    }

    private suspend fun fetch(client: OkHttpClient, url: String, into: File) {
        val bytes = call(client, Request.Builder().url(url).build()) { response ->
            if (!response.isSuccessful) throw ApiException.from(response)
            response.body.bytes()
        }
        withContext(io) {
            into.parentFile?.mkdirs()
            into.writeBytes(bytes)
        }
    }

    /**
     * Sends [request], then reads its answer on [io]. Cancelling cancels
     * the call, which also ends a read waiting on the network.
     */
    private suspend fun <T> call(client: OkHttpClient, request: Request, read: suspend (Response) -> T): T {
        val running = client.newCall(request)
        val response = running.await()
        return response.use {
            coroutineScope {
                val canceller = launch {
                    try {
                        awaitCancellation()
                    } finally {
                        running.cancel()
                    }
                }
                try {
                    withContext(io) { read(response) }
                } finally {
                    canceller.cancel()
                }
            }
        }
    }

    private suspend fun Call.await(): Response = suspendCancellableCoroutine { continuation ->
        continuation.invokeOnCancellation { cancel() }
        enqueue(
            object : Callback {
                override fun onFailure(call: Call, e: IOException) {
                    continuation.resumeWithException(e)
                }

                override fun onResponse(call: Call, response: Response) {
                    continuation.resume(response) { _, _, _ -> response.close() }
                }
            },
        )
    }

    private fun writeWhole(file: File, text: String) {
        file.parentFile?.mkdirs()
        val temp = File(file.path + ".tmp")
        temp.writeText(text)
        if (!temp.renameTo(file)) {
            file.delete()
            temp.renameTo(file)
        }
    }

    companion object {
        /** How long a run waits for a network it may use before it stops (the app starts it again). */
        const val WAIT_MS = 15 * 60 * 1000L
        const val MAX_LOST_IN_A_ROW = 5
        const val BACKOFF_MS = 5_000L
        const val PROGRESS_MS = 500L
        const val BUFFER_BYTES = 256 * 1024
        const val IMAGE_WIDTH = 640
        const val SUBTITLE_TRIES = 20
        const val SUBTITLE_WAIT_MS = 3_000L

        /** How often a conversion the server is making is looked at. */
        const val CONVERSION_POLL_MS = 5_000L

        const val CONVERSION_FAILED = "The server couldn't make it for this device"
        const val NO_CONVERSIONS = "The server can't make smaller downloads: choose Original in Settings → Downloads"

        /** Room kept free beside a download, so the device doesn't fill up. */
        const val SPACE_MARGIN = 500L * 1024 * 1024

        const val NO_SPACE = "Not enough space on this device"

        /** The whole file's size: from Content-Range for a range, else the body's length. */
        fun totalOf(response: Response, from: Long): Long {
            val range = response.header("Content-Range")
            if (response.code == 206 && range != null) return range.substringAfterLast('/').trim().toLongOrNull() ?: -1
            val length = response.body.contentLength()
            return if (length >= 0) from + length else -1
        }

        /** What a failed download says, from the server's answer. */
        fun messageFor(e: ApiException): String = when (e.status) {
            401, 403 -> "Sign in to the server again to download this"
            404, 410 -> "This file is no longer on the server"
            else -> "The server couldn't send this file (${e.status})"
        }
    }
}
