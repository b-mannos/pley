package app.pley.android.data

import app.pley.android.auth.AuthException
import app.pley.android.model.Me
import app.pley.android.model.PleyJson
import app.pley.android.model.TrackDefaults
import app.pley.android.network.ApiException
import app.pley.android.network.OfflineException
import app.pley.android.network.UnreadableAnswerException
import java.io.IOException
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.serialization.SerializationException
import okhttp3.Call
import okhttp3.Callback
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response

/**
 * PUT /api/me/track-defaults over [client] (a signed-in client of the
 * server, without a cache), until PleyApi carries the route. Failures come
 * out as PleyApi's do: ApiException, OfflineException,
 * UnreadableAnswerException.
 */
class HttpTrackDefaults(private val client: (issuer: String) -> OkHttpClient) : TrackDefaultsRoute {
    override suspend fun put(issuer: String, defaults: TrackDefaults): Me {
        val origin = ProfileScope.originOf(issuer)
        val body = PleyJson.encodeToString(TrackDefaults.serializer(), defaults).toRequestBody(JSON)
        val request = Request.Builder()
            .url("$origin/api/me/track-defaults")
            .header("Accept", "application/json")
            .put(body)
            .build()
        return suspendCancellableCoroutine { continuation ->
            val call = client(origin).newCall(request)
            continuation.invokeOnCancellation { call.cancel() }
            call.enqueue(
                object : Callback {
                    override fun onFailure(call: Call, e: IOException) {
                        continuation.resumeWith(Result.failure(if (e is AuthException || e is ApiException) e else OfflineException(e)))
                    }

                    override fun onResponse(call: Call, response: Response) {
                        val result = response.use {
                            if (!it.isSuccessful) {
                                Result.failure(ApiException.from(it))
                            } else {
                                try {
                                    Result.success(PleyJson.decodeFromString(Me.serializer(), it.body.string()))
                                } catch (e: SerializationException) {
                                    Result.failure(UnreadableAnswerException(it.code, e))
                                } catch (e: IllegalArgumentException) {
                                    Result.failure(UnreadableAnswerException(it.code, e))
                                } catch (e: IOException) {
                                    Result.failure(OfflineException(e))
                                }
                            }
                        }
                        continuation.resumeWith(result)
                    }
                },
            )
        }
    }

    private companion object {
        val JSON = "application/json".toMediaType()
    }
}
