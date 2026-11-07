package app.pley.android.screens.admin.livetv

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.pley.android.data.ServerAdmin
import app.pley.android.model.PleyJson
import app.pley.android.model.admin.LiveSource
import app.pley.android.model.admin.LiveSourceInput
import app.pley.android.model.admin.LiveSourceKind
import app.pley.android.model.admin.LiveSourceTest
import app.pley.android.screens.admin.AdminDone
import app.pley.android.screens.admin.AdminLoad
import app.pley.android.screens.admin.AdminViewModel
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.screens.screensGraph
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject

/** A source's details as typed, before they are saved (the web's SourceModal form). */
data class SourceDraft(
    val kind: LiveSourceKind = LiveSourceKind.M3u,
    val name: String = "",
    val url: String = "",
    val username: String = "",
    /** Empty, for a saved source: the saved one is kept. */
    val password: String = "",
    val guideUrl: String = "",
    val userAgent: String = "",
    /** 0: no limit. */
    val maxConnections: Int = 1,
) {
    /** What it needs before it is tested or saved: an address, and an Xtream account's user name. */
    val complete: Boolean get() = url.isNotBlank() && (kind != LiveSourceKind.Xtream || username.isNotBlank())

    /** The name it is saved with: as typed, else its address's host. */
    val savedName: String get() = name.trim().ifEmpty { defaultSourceName(url) }

    /** What is sent; [keepPassword]: a saved source's password stays when none is typed. */
    fun input(enabled: Boolean, keepPassword: Boolean): LiveSourceInput = LiveSourceInput(
        name = savedName,
        kind = kind,
        url = url.trim(),
        username = username.trim(),
        password = password.takeUnless { keepPassword && it.isEmpty() },
        guideUrl = guideUrl.trim(),
        userAgent = userAgent.trim(),
        maxConnections = maxConnections,
        enabled = enabled,
    )

    companion object {
        /** [source]'s details, its password not typed. */
        fun of(source: LiveSource) = SourceDraft(
            kind = source.kind,
            name = source.name,
            url = source.url,
            username = source.username,
            guideUrl = source.guideUrl,
            userAgent = source.userAgent,
            maxConnections = source.maxConnections,
        )
    }
}

/** Testing a source's details: what it found, or why it couldn't, for the details as they were typed then. */
sealed interface SourceTesting {
    data object None : SourceTesting

    data object Testing : SourceTesting

    data class Found(val draft: SourceDraft, val result: LiveSourceTest) : SourceTesting

    data class Failed(val draft: SourceDraft, val error: Throwable) : SourceTesting
}

/**
 * A source's page ([sourceId]), or adding one (`null`), as the web's
 * source card and SourceModal: its state and counts, turning it on and
 * off, reading it again, removing it; and its details, typed in, tested
 * with the server first if wanted, and saved together.
 */
class LiveSourceViewModel(admin: ServerAdmin, private val sourceId: Long?) : AdminViewModel(admin) {
    /** The source as saved; `null` when adding one, or when it is gone. */
    val source = resource<LiveSource?> { sourceId?.let { id -> liveSources().firstOrNull { it.id == id } } }

    private val draftNow = MutableStateFlow(if (sourceId == null) SourceDraft() else null)

    /** The details as typed; `null` until the source is read. */
    val draft: StateFlow<SourceDraft?> = draftNow.asStateFlow()

    private val testingNow = MutableStateFlow<SourceTesting>(SourceTesting.None)
    val testing: StateFlow<SourceTesting> = testingNow.asStateFlow()

    private val done = Channel<Unit>(Channel.CONFLATED)

    /** Sends once the page is done with: the source added, or removed. */
    val finished: Flow<Unit> = done.receiveAsFlow()

    val adding: Boolean get() = sourceId == null

    init {
        if (sourceId != null) {
            viewModelScope.launch {
                source.state.collect { load ->
                    val saved = (load as? AdminLoad.Ready)?.value
                    if (saved != null && draftNow.value == null) draftNow.value = SourceDraft.of(saved)
                }
            }
        }
    }

    /** Whether the details typed differ from the saved ones (always, when adding). */
    fun changed(draft: SourceDraft): Boolean {
        val saved = source.state.value.valueOrNull ?: return true
        return draft != SourceDraft.of(saved)
    }

    /** Changes the details as typed; nothing is sent. */
    fun edit(transform: (SourceDraft) -> SourceDraft) {
        draftNow.update { it?.let(transform) }
    }

    /** Reads what the details typed give, without saving them; a saved source lends its password. */
    fun test() {
        val draft = draftNow.value?.takeIf { it.complete } ?: return
        if (testingNow.value == SourceTesting.Testing) return
        testingNow.value = SourceTesting.Testing
        viewModelScope.launch {
            testingNow.value = try {
                val body = PleyJson.encodeToJsonElement(LiveSourceInput.serializer(), draft.input(enabled = true, keepPassword = !adding)).jsonObject
                val withSource = if (sourceId == null) body else JsonObject(body + ("source_id" to JsonPrimitive(sourceId)))
                val result = admin.api().send("POST", LiveSourceTest.serializer(), listOf("api", "live", "sources", "test"), withSource)
                // An Xtream account says how many connections it has.
                val connections = result.account?.maxConnections?.takeIf { adding && it > 0 }
                val tested = if (connections != null) draft.copy(maxConnections = connections) else draft
                if (connections != null) draftNow.update { if (it == draft) tested else it }
                SourceTesting.Found(tested, result)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                SourceTesting.Failed(draft, e)
            }
        }
    }

    /** Adds the source, or saves its details; the page is done with once one is added. */
    fun save() {
        val draft = draftNow.value?.takeIf { it.complete } ?: return
        if (busy.value > 0) return
        val saved = source.state.value.valueOrNull
        if (sourceId == null) {
            change(said = AdminDone(ADDED)) {
                send("POST", LiveSource.serializer(), listOf("api", "live", "sources"), body(draft.input(enabled = true, keepPassword = false)))
                done.send(Unit)
            }
        } else {
            change(said = AdminDone(SAVED)) {
                val input = draft.input(enabled = saved?.enabled ?: true, keepPassword = true)
                val answer = send("PUT", LiveSource.serializer(), listOf("api", "live", "sources", sourceId), body(input))
                source.set(answer)
                draftNow.value = SourceDraft.of(answer)
            }
        }
    }

    /** Turns the source on or off at once, as saved (what is typed and not saved stays typed). */
    fun setEnabled(enabled: Boolean) {
        val saved = source.state.value.valueOrNull ?: return
        source.set(saved.copy(enabled = enabled))
        change(after = source::reload) {
            call("PUT", listOf("api", "live", "sources", saved.id), body(SourceDraft.of(saved).input(enabled = enabled, keepPassword = true)))
        }
    }

    /** Reads its channels and guide again now. */
    fun refresh() {
        val id = sourceId ?: return
        change(after = source::reload) { call("POST", listOf("api", "live", "sources", id, "refresh")) }
    }

    /** Removes the source, its channels and their guide. */
    fun remove() {
        val saved = source.state.value.valueOrNull ?: return
        change(said = AdminDone(REMOVED, listOf(saved.name))) {
            call("DELETE", listOf("api", "live", "sources", saved.id))
            done.send(Unit)
        }
    }

    /** Reads the source again every [LiveTvAdminViewModel.READING_EVERY] ms while it is being read, until cancelled. */
    suspend fun watch() {
        while (true) {
            delay(LiveTvAdminViewModel.READING_EVERY)
            if (source.state.value.valueOrNull?.refreshing == true) source.reload()
        }
    }

    private fun body(input: LiveSourceInput) = PleyJson.encodeToJsonElement(LiveSourceInput.serializer(), input)

    companion object {
        const val ADDED = "livetv.source.added"
        const val SAVED = "livetv.source.saved"
        const val REMOVED = "livetv.source.removed"

        fun factory(sourceId: Long?): ViewModelProvider.Factory = viewModelFactory {
            initializer { LiveSourceViewModel(screensGraph().data.admin, sourceId) }
        }
    }
}
