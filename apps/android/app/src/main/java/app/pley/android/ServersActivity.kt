package app.pley.android

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.net.toUri
import app.pley.android.common.Discovery
import app.pley.android.common.FoundServer
import app.pley.android.common.LocalNetwork
import app.pley.android.common.SavedServer
import app.pley.android.common.ServerEntry
import app.pley.android.common.Servers
import app.pley.android.common.describe
import java.net.URI
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch

/**
 * The list of servers the app opens with: the ones opened before and the
 * ones answering on the local network. The server opened last opens by
 * itself, unless the web app asked for this list ("Switch server").
 *
 * On Android 17 (and Fire OS 16) the app first asks to reach devices on the
 * network, once per process: without it nothing at home answers.
 */
class ServersActivity : ComponentActivity() {
    /** Whether the app may reach devices on this network; `null` while it asks. */
    private val access = mutableStateOf<Boolean?>(null)
    private var asking = false
    private val askAccess = registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
        asking = false
        access.value = !LocalNetwork.needsRequest(this)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val store = ServerStore(this)
        val pick = intent.getBooleanExtra(EXTRA_PICK, false)
        val missing = LocalNetwork.missing(this)
        asking = savedInstanceState?.getBoolean(KEY_ASKING) ?: false
        access.value = when {
            missing.isEmpty() -> true
            // The question is on screen already (the activity was recreated under it).
            asking -> null
            askedInThisProcess -> false
            else -> {
                askedInThisProcess = true
                asking = true
                askAccess.launch(missing.toTypedArray())
                null
            }
        }
        setContent {
            PleyTheme {
                ServersScreen(
                    store,
                    autoOpen = !pick && savedInstanceState == null,
                    access = access.value,
                    onOpenSettings = { openAppSettings() },
                ) { url ->
                    startActivity(WebActivity.intent(this, url))
                    finish()
                }
            }
        }
    }

    override fun onResume() {
        super.onResume()
        // Allowed in Settings meanwhile: the screen searches again.
        if (access.value == false && !LocalNetwork.needsRequest(this)) access.value = true
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putBoolean(KEY_ASKING, asking)
    }

    private fun openAppSettings() {
        try {
            startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, "package:$packageName".toUri()))
        } catch (_: ActivityNotFoundException) {
            // No settings screen for apps (some TVs): nothing to open.
        }
    }

    companion object {
        private const val EXTRA_PICK = "pick"
        private const val KEY_ASKING = "asking_local_network"

        /** The question is asked once per process; after a no, the screen says how to allow it. */
        @Volatile private var askedInThisProcess = false

        /** The list, without opening the last server by itself. */
        fun pick(context: Context): Intent = Intent(context, ServersActivity::class.java).putExtra(EXTRA_PICK, true)
    }
}

/** What the screen says when the system keeps the app off the home network. */
private const val NO_ACCESS = "pley needs access to devices on your network to find and reach servers at home."

/** Whether [url] is a server at home, which the local network permission covers. */
private fun atHome(url: String): Boolean = try {
    Servers.isLocalHost(URI(url).host ?: "")
} catch (_: java.net.URISyntaxException) {
    false
}

@Composable
private fun ServersScreen(
    store: ServerStore,
    autoOpen: Boolean,
    access: Boolean?,
    onOpenSettings: () -> Unit,
    onOpen: (String) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var saved by remember { mutableStateOf(store.all()) }
    var found by remember { mutableStateOf(emptyList<FoundServer>()) }
    var blocked by remember { mutableStateOf(false) }
    var started by remember { mutableStateOf(false) }
    var searching by remember { mutableStateOf(false) }
    val reachable = remember { mutableStateMapOf<String, Boolean>() }
    var busy by remember { mutableStateOf<String?>(null) }
    var status by remember { mutableStateOf<String?>(null) }
    var address by remember { mutableStateOf("") }
    var addressError by remember { mutableStateOf<String?>(null) }
    // An unencrypted address someone was warned about: connecting again goes ahead.
    var warnedAbout by remember { mutableStateOf<String?>(null) }
    var opening by remember { mutableStateOf(if (autoOpen) Servers.lastUsed(saved) else null) }
    var openingJob by remember { mutableStateOf<Job?>(null) }

    fun search() {
        searching = true
        scope.launch {
            val checks = saved.map { server ->
                launch {
                    reachable[server.url] = runCatching { PleyApi(server.url, null).serverInfo() }.isSuccess
                }
            }
            val search = Discovery.search(context)
            found = search.servers
            blocked = search.blocked
            checks.forEach { it.join() }
            searching = false
        }
    }

    fun open(url: String, id: String?) {
        busy = url
        status = null
        scope.launch {
            try {
                val info = PleyApi(url, null).serverInfo()
                store.save(SavedServer(url, info.name, id, System.currentTimeMillis()))
                onOpen(url)
            } catch (e: Exception) {
                reachable[url] = false
                status = "Couldn't reach ${Servers.displayAddress(url)}: ${describe(e)}"
                busy = null
            }
        }
    }

    LaunchedEffect(access) {
        // Nothing reaches the network before the system's question is answered.
        if (access == null) return@LaunchedEffect
        if (started) {
            // Allowed since (in Settings): look again.
            if (access) search()
            return@LaunchedEffect
        }
        started = true
        // Without access, a server at home can't answer: the list says why instead.
        if (!access && opening?.let { atHome(it.url) } == true) opening = null
        val last = opening
        if (last == null) {
            search()
        } else {
            // In the screen's scope: an answer to the question later doesn't stop it.
            openingJob = scope.launch {
                try {
                    val info = PleyApi(last.url, null).serverInfo()
                    store.save(last.copy(name = info.name, lastUsed = System.currentTimeMillis()))
                    onOpen(last.url)
                } catch (e: Exception) {
                    reachable[last.url] = false
                    status = "${last.name} didn't answer: ${describe(e)}"
                    opening = null
                    search()
                }
            }
        }
    }

    Box(
        Modifier
            .fillMaxSize()
            .background(PleyColors.Bg)
            .systemBarsPadding()
            .imePadding(),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            Modifier
                .widthIn(max = 520.dp)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp, vertical = 40.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Image(painterResource(R.drawable.pley_mark), contentDescription = null, modifier = Modifier.size(40.dp))
                Text("pley", fontSize = 26.sp, fontWeight = FontWeight.ExtraBold, color = PleyColors.Fg)
            }
            Spacer(Modifier.height(36.dp))

            val auto = opening
            if (auto != null) {
                Text("Opening ${auto.name}…", fontSize = 18.sp, fontWeight = FontWeight.SemiBold, color = PleyColors.Fg)
                TextButton(onClick = {
                    openingJob?.cancel()
                    opening = null
                    search()
                }) { Text("Choose another server") }
                return@Column
            }

            Text("Choose a server", fontSize = 22.sp, fontWeight = FontWeight.Bold, color = PleyColors.Fg)
            Spacer(Modifier.height(14.dp))
            val entries = Servers.merge(saved, found)
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                for (entry in entries) {
                    ServerRow(
                        entry = entry,
                        online = reachable[entry.url],
                        connecting = busy == entry.url,
                        enabled = busy == null,
                        onOpen = { open(entry.url, entry.id) },
                        onForget = if (entry.saved) {
                            {
                                store.forget(entry.url)
                                saved = store.all()
                            }
                        } else {
                            null
                        },
                    )
                }
            }
            val noAccess = access == false || blocked
            val note = status ?: when {
                access == null -> null
                searching -> "Looking for servers on this network…"
                noAccess -> NO_ACCESS
                entries.isEmpty() -> "No server answered. Check that pley is running on it, or enter its address."
                else -> null
            }
            if (note != null) {
                Text(note, fontSize = 14.sp, color = PleyColors.Muted, modifier = Modifier.padding(top = 12.dp))
            }
            if (noAccess && !searching) {
                TextButton(onClick = onOpenSettings, modifier = Modifier.padding(top = 4.dp)) {
                    Text("Open settings")
                }
            }
            if (!searching) {
                TextButton(onClick = { search() }, modifier = Modifier.padding(top = 4.dp)) {
                    Text("Search the network again")
                }
            }

            HorizontalDivider(Modifier.padding(vertical = 20.dp), color = PleyColors.Border)
            Text("Or enter its address", fontSize = 15.sp, fontWeight = FontWeight.SemiBold, color = PleyColors.Fg)
            Spacer(Modifier.height(8.dp))
            val connect: () -> Unit = {
                val urls = Servers.candidates(address)
                if (urls.isEmpty()) {
                    addressError = "Enter an address such as nas, 192.168.1.20 or https://pley.example.com."
                } else {
                    addressError = null
                    busy = urls.last()
                    scope.launch {
                        // The first that answers (https before http for names on the internet).
                        val url = urls.firstOrNull { runCatching { PleyApi(it, null).serverInfo() }.isSuccess }
                        busy = null
                        when {
                            url == null -> addressError = "Couldn't reach ${Servers.displayAddress(urls.last())}."
                            Servers.unencrypted(url) && warnedAbout != url -> {
                                warnedAbout = url
                                addressError = "${Servers.displayAddress(url)} isn't encrypted: your password and what you " +
                                    "watch could be read on the way. Use its https address if it has one, or tap " +
                                    "Connect again to go ahead anyway."
                            }
                            else -> open(url, null)
                        }
                    }
                }
            }
            OutlinedTextField(
                value = address,
                onValueChange = { address = it },
                singleLine = true,
                placeholder = { Text("nas, 192.168.1.20 or https://…") },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
                keyboardActions = KeyboardActions(onGo = { connect() }),
                isError = addressError != null,
                modifier = Modifier.fillMaxWidth(),
            )
            if (addressError != null) {
                Text(addressError ?: "", fontSize = 13.sp, color = PleyColors.Danger, modifier = Modifier.padding(top = 6.dp))
            }
            Button(onClick = { connect() }, enabled = busy == null, modifier = Modifier.padding(top = 12.dp)) {
                Text("Connect", fontWeight = FontWeight.Bold)
            }
        }
    }
}

@Composable
private fun ServerRow(
    entry: ServerEntry,
    online: Boolean?,
    connecting: Boolean,
    enabled: Boolean,
    onOpen: () -> Unit,
    onForget: (() -> Unit)?,
) {
    Surface(
        onClick = onOpen,
        enabled = enabled,
        shape = RoundedCornerShape(10.dp),
        color = PleyColors.Surface,
        border = BorderStroke(1.dp, PleyColors.Border),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 16.dp)) {
            val dot = when {
                entry.found || online == true -> PleyColors.Success
                online == false -> PleyColors.Danger
                else -> PleyColors.Subtle
            }
            Box(Modifier.size(9.dp).background(dot, CircleShape))
            Column(
                Modifier
                    .weight(1f)
                    .padding(horizontal = 12.dp, vertical = 12.dp),
            ) {
                Text(entry.name, fontWeight = FontWeight.SemiBold, color = PleyColors.Fg, maxLines = 1, overflow = TextOverflow.Ellipsis)
                Text(
                    if (connecting) "Connecting…" else Servers.displayAddress(entry.url),
                    fontSize = 13.sp,
                    color = PleyColors.Muted,
                )
            }
            val warning = entry.lookalike || Servers.unencrypted(entry.url)
            val tag = when {
                entry.lookalike -> "Not your saved server"
                Servers.unencrypted(entry.url) -> "Not encrypted"
                !entry.saved -> "New"
                entry.found -> "On this network"
                else -> null
            }
            if (tag != null) {
                Text(
                    tag,
                    fontSize = 12.sp,
                    color = if (warning) PleyColors.Danger else PleyColors.Subtle,
                    modifier = Modifier.padding(end = 8.dp),
                )
            }
            if (onForget != null) {
                TextButton(
                    onClick = onForget,
                    modifier = Modifier.semantics { contentDescription = "Forget ${entry.name}" },
                ) { Text("×", fontSize = 20.sp, color = PleyColors.Subtle) }
            }
        }
    }
}
