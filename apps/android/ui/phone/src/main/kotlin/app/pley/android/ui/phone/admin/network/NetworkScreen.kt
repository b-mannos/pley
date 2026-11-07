package app.pley.android.ui.phone.admin.network

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.PleyIcons
import app.pley.android.model.admin.ListenerKind
import app.pley.android.model.admin.NetworkAdapter
import app.pley.android.screens.admin.network.CompatApi
import app.pley.android.screens.admin.network.ListenMode
import app.pley.android.screens.admin.network.NetworkPage
import app.pley.android.screens.admin.network.NetworkProblem
import app.pley.android.screens.admin.network.NetworkViewModel
import app.pley.android.screens.admin.network.PORTS
import app.pley.android.screens.admin.network.addressesElsewhere
import app.pley.android.screens.admin.network.bonjourChoices
import app.pley.android.screens.admin.network.hostLabel
import app.pley.android.screens.admin.network.hostNameOk
import app.pley.android.screens.admin.valueOrNull
import app.pley.android.ui.phone.R
import app.pley.android.ui.phone.account.Choice
import app.pley.android.ui.phone.account.ChoiceSheet
import app.pley.android.ui.phone.account.RowDivider
import app.pley.android.ui.phone.account.SettingsGroup
import app.pley.android.ui.phone.account.SettingsRow
import app.pley.android.ui.phone.admin.AdminEvents
import app.pley.android.ui.phone.admin.AdminPage
import app.pley.android.ui.phone.admin.Entry
import app.pley.android.ui.phone.admin.TextEntryDialog
import app.pley.android.ui.phone.admin.adminWidth
import app.pley.android.ui.phone.admin.numberProblem
import app.pley.android.ui.phone.kit.ConfirmDialog
import app.pley.android.ui.phone.kit.PleyIconButton
import app.pley.android.ui.phone.kit.PleyTextButton
import app.pley.android.ui.phone.kit.PrimaryButton
import app.pley.android.ui.phone.shell.LocalPhoneShell

/** What a row of the page opened: a choice or a value to type. */
private sealed interface NetworkDialog {
    data object Port : NetworkDialog

    data class ApiWhere(val api: CompatApi) : NetworkDialog

    data class ApiPort(val api: CompatApi) : NetworkDialog

    data object HostName : NetworkDialog

    data object AddProxy : NetworkDialog
}

/**
 * Network (the web's Settings → Network): where the server listens, the
 * Plex- and Jellyfin-compatible APIs, discovery and Bonjour, edited
 * together and saved with the bar that shows once something changed, as
 * the web's form is; what the command line or the environment sets is
 * shown and locked. Before a change that would stop the server listening
 * where this phone reaches it, the page says so and asks. The proxy
 * addresses are sent as each is added or removed; what listens now is read
 * again every few seconds while an address waits. Network has one page:
 * [id] names nothing.
 */
@Composable
internal fun NetworkScreen(@Suppress("UNUSED_PARAMETER") id: String?) {
    val model: NetworkViewModel = viewModel(factory = NetworkViewModel.factory)
    val load by model.page.collectAsStateWithLifecycle()
    val proxy by model.proxy.state.collectAsStateWithLifecycle()
    val busy by model.busy.collectAsStateWithLifecycle()
    val resources = LocalResources.current
    val shell = LocalPhoneShell.current
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    AdminEvents(model) { done -> if (done.what == NetworkViewModel.PROXY_SAVED) resources.getString(R.string.admin_network_proxy_saved) else null }
    LaunchedEffect(model) {
        model.saved.collect { waiting ->
            shell.toasts.show(resources.getString(if (waiting) R.string.admin_network_saved_waiting else R.string.admin_network_listens_now))
        }
    }
    LaunchedEffect(model, lifecycle) { lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) { model.watchListeners() } }

    var dialog by remember { mutableStateOf<NetworkDialog?>(null) }
    var confirmCutOff by remember { mutableStateOf(false) }
    val page = load.valueOrNull
    // While a change waits to be saved, and while it is sent (it stays edited until the server takes it).
    val barShown = page != null && page.edited
    val proxies = proxy.valueOrNull?.proxyAddresses

    Box(Modifier.fillMaxSize()) {
        AdminPage(
            title = stringResource(R.string.admin_section_network),
            load = load,
            onRetry = model.network::reload,
            tag = NETWORK_PAGE,
        ) { now ->
            networkItems(now, model, busy, proxies, open = { dialog = it })
            // Room for the bar, so it doesn't cover the last rows.
            if (barShown) item(key = "bar") { Spacer(Modifier.height(96.dp)) }
        }
        if (page != null && barShown) {
            SaveBar(
                page = page,
                busy = busy > 0,
                onReset = model::reset,
                onSave = { if (page.cutsOff) confirmCutOff = true else model.save() },
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom))
                    .padding(16.dp)
                    .adminWidth,
            )
        }
    }

    if (confirmCutOff) {
        ConfirmDialog(
            title = stringResource(R.string.admin_network_would_stop_title),
            body = stringResource(R.string.admin_network_would_stop),
            confirmLabel = stringResource(R.string.admin_network_save_anyway),
            onConfirm = {
                confirmCutOff = false
                model.save()
            },
            onDismiss = { confirmCutOff = false },
            destructive = true,
        )
    }
    val form = page?.form
    when (val open = dialog) {
        null -> Unit
        NetworkDialog.Port -> if (form != null) {
            TextEntryDialog(
                title = stringResource(R.string.admin_network_port),
                initial = form.port.toString(),
                onSave = { typed ->
                    typed.toIntOrNull()?.let(model::setPort)
                    dialog = null
                },
                onDismiss = { dialog = null },
                hint = stringResource(R.string.admin_network_port_hint),
                entry = Entry.Number,
                check = numberProblem(PORTS),
            )
        }
        is NetworkDialog.ApiWhere -> if (form != null) {
            ChoiceSheet(
                title = stringResource(portChoiceOf(open.api)),
                choices = listOf(
                    Choice(true, stringResource(R.string.admin_network_on_pley_port, form.port)),
                    Choice(false, stringResource(R.string.admin_network_own_port)),
                ),
                selected = form.api(open.api).onWeb,
                onPick = { model.setApiOnWeb(open.api, it) },
                onDismiss = { dialog = null },
            )
        }
        is NetworkDialog.ApiPort -> if (form != null) {
            TextEntryDialog(
                title = stringResource(portLabelOf(open.api)),
                initial = form.api(open.api).port.toString(),
                onSave = { typed ->
                    typed.toIntOrNull()?.let { model.setApiPort(open.api, it) }
                    dialog = null
                },
                onDismiss = { dialog = null },
                hint = stringResource(portHintOf(open.api), open.api.ownPort),
                entry = Entry.Number,
                check = numberProblem(PORTS),
            )
        }
        NetworkDialog.HostName -> if (page != null) {
            val problem = stringResource(R.string.admin_network_problem_host_name)
            TextEntryDialog(
                title = stringResource(R.string.admin_network_host_name),
                initial = page.form.bonjourHost,
                onSave = { typed ->
                    model.setBonjourHost(typed)
                    dialog = null
                },
                onDismiss = { dialog = null },
                hint = stringResource(R.string.admin_network_host_name_hint, page.network.bonjourHostDefault),
                entry = Entry.Address,
                check = { typed -> if (hostNameOk(typed)) null else problem },
            )
        }
        NetworkDialog.AddProxy -> TextEntryDialog(
            title = stringResource(R.string.admin_network_proxy_add),
            initial = "",
            onSave = { typed ->
                if (typed.isNotEmpty()) model.addProxy(typed)
                dialog = null
            },
            onDismiss = { dialog = null },
            label = stringResource(R.string.admin_network_proxy_address),
            hint = PROXY_EXAMPLE,
            entry = Entry.Address,
            confirmLabel = stringResource(R.string.admin_network_proxy_add_button),
        )
    }
}

/** The page's groups, in the web's order. */
private fun LazyListScope.networkItems(
    page: NetworkPage,
    model: NetworkViewModel,
    busy: Int,
    proxies: List<String>?,
    open: (NetworkDialog) -> Unit,
) {
    val network = page.network
    val form = page.form
    val fixed = network.fixed
    item(key = "intro") {
        Text(
            stringResource(R.string.admin_network_description, network.configFile),
            style = MaterialTheme.typography.bodyMedium,
            color = PleyColors.Muted,
            modifier = Modifier.adminWidth.padding(horizontal = 4.dp),
        )
    }
    item(key = "listen") {
        val notes = listOfNotNull(
            stringResource(R.string.admin_network_fixed_bind).takeIf { fixed.web },
            stringResource(R.string.admin_network_firewall).takeIf { network.windows && form.onNetworks },
        )
        SettingsGroup(
            stringResource(R.string.admin_network_listen_on),
            Modifier.adminWidth,
            footer = notes.joinToString("\n\n").ifEmpty { null },
        ) {
            for (mode in ListenMode.entries) {
                RadioRow(
                    title = stringResource(modeLabelOf(mode)),
                    hint = stringResource(modeHintOf(mode)),
                    selected = form.mode == mode,
                    enabled = !fixed.web,
                    onClick = { model.setMode(mode) },
                    modifier = Modifier.testTag(MODE_TAG + mode.name),
                )
                RowDivider(inset = false)
            }
            SettingsRow(
                stringResource(R.string.admin_network_port),
                value = form.port.toString(),
                enabled = !fixed.web,
                onClick = { open(NetworkDialog.Port) },
                modifier = Modifier.testTag(PORT_TAG),
            )
        }
    }
    if (form.mode == ListenMode.Networks) {
        item(key = "adapters") {
            SettingsGroup(stringResource(R.string.admin_network_adapters), Modifier.adminWidth) {
                val elsewhere = addressesElsewhere(form, network.adapters)
                for ((i, adapter) in network.adapters.withIndex()) {
                    if (i > 0) RowDivider(inset = false)
                    CheckRow(
                        title = adapter.name,
                        checked = adapter.addresses.any { it in form.ips },
                        onToggle = { model.toggleAdapter(adapter) },
                        tags = adapterTags(adapter),
                        detail = listOfNotNull(adapter.addresses.joinToString(", ").ifEmpty { null }, adapter.network?.name, adapter.description)
                            .joinToString(" · "),
                        enabled = !fixed.web,
                    )
                }
                for ((i, ip) in elsewhere.withIndex()) {
                    if (i > 0 || network.adapters.isNotEmpty()) RowDivider(inset = false)
                    CheckRow(
                        title = ip,
                        checked = true,
                        onToggle = { model.dropAddress(ip) },
                        detail = stringResource(R.string.admin_network_adapter_elsewhere),
                        enabled = !fixed.web,
                        mono = true,
                    )
                }
                if (network.adapters.isEmpty() && elsewhere.isEmpty()) {
                    Text(
                        stringResource(R.string.admin_network_no_adapters),
                        style = MaterialTheme.typography.bodyMedium,
                        color = PleyColors.Subtle,
                        modifier = Modifier.padding(16.dp),
                    )
                }
            }
        }
    }
    item(key = "apps") {
        SettingsGroup(null, Modifier.adminWidth) {
            for (api in CompatApi.entries) {
                val value = form.api(api)
                val locked = if (api == CompatApi.Jellyfin) fixed.jellyfin else fixed.plex
                LockableSwitchRow(
                    title = stringResource(listenerNameOf(api)),
                    checked = value.on,
                    onChange = { model.setApiOn(api, it) },
                    subtitle = stringResource(if (locked) fixedOf(api) else hintOf(api)),
                    enabled = !locked,
                    modifier = Modifier.testTag(API_TAG + api.name),
                )
                if (value.on) {
                    RowDivider(inset = false)
                    SettingsRow(
                        stringResource(portChoiceOf(api)),
                        value = if (value.onWeb) stringResource(R.string.admin_network_on_pley_port, form.port) else stringResource(R.string.admin_network_own_port),
                        enabled = !locked,
                        onClick = { open(NetworkDialog.ApiWhere(api)) },
                    )
                }
                if (value.on && !value.onWeb) {
                    RowDivider(inset = false)
                    SettingsRow(
                        stringResource(portLabelOf(api)),
                        value = value.port.toString(),
                        enabled = !locked,
                        onClick = { open(NetworkDialog.ApiPort(api)) },
                    )
                }
                RowDivider(inset = false)
            }
            LockableSwitchRow(
                title = stringResource(R.string.admin_network_discovery),
                checked = form.discovery && form.onNetworks,
                onChange = model::setDiscovery,
                subtitle = stringResource(
                    when {
                        fixed.discovery -> R.string.admin_network_discovery_fixed
                        !form.onNetworks -> R.string.admin_network_local_only
                        else -> R.string.admin_network_discovery_hint
                    },
                ),
                enabled = !fixed.discovery && form.onNetworks,
            )
        }
    }
    val bonjourShown = form.bonjour && form.onNetworks
    item(key = "bonjour") {
        SettingsGroup(null, Modifier.adminWidth) {
            LockableSwitchRow(
                title = stringResource(R.string.admin_network_bonjour),
                checked = bonjourShown,
                onChange = model::setBonjour,
                subtitle = stringResource(
                    when {
                        fixed.bonjour -> R.string.admin_network_bonjour_fixed
                        !form.onNetworks -> R.string.admin_network_local_only
                        else -> R.string.admin_network_bonjour_hint
                    },
                ),
                enabled = !fixed.bonjour && form.onNetworks,
                modifier = Modifier.testTag(BONJOUR_TAG),
            )
            if (bonjourShown) {
                RowDivider(inset = false)
                val label = if (hostNameOk(form.bonjourHost)) hostLabel(form.bonjourHost) else form.bonjourHost.trim()
                SettingsRow(
                    stringResource(R.string.admin_network_host_name),
                    value = "${label.ifEmpty { network.bonjourHostDefault }}.local",
                    enabled = !fixed.bonjour,
                    onClick = { open(NetworkDialog.HostName) },
                )
            }
        }
    }
    if (bonjourShown) {
        item(key = "announced") {
            val choices = bonjourChoices(form, network.adapters)
            SettingsGroup(
                stringResource(R.string.admin_network_announced_on),
                Modifier.adminWidth,
                footer = stringResource(R.string.admin_network_announced_on_hint),
            ) {
                CheckRow(
                    title = stringResource(R.string.admin_network_every),
                    checked = form.bonjourAdapters.isEmpty(),
                    onToggle = model::announceEverywhere,
                    detail = stringResource(R.string.admin_network_every_hint),
                    enabled = !fixed.bonjour,
                )
                for (adapter in choices.adapters) {
                    RowDivider(inset = false)
                    CheckRow(
                        title = adapter.name,
                        checked = adapter.name in form.bonjourAdapters,
                        onToggle = { model.toggleBonjourAdapter(adapter.name) },
                        detail = listOfNotNull(adapter.addresses.joinToString(", ").ifEmpty { null }, adapter.network?.name).joinToString(" · "),
                        enabled = !fixed.bonjour,
                    )
                }
                for (name in choices.elsewhere) {
                    RowDivider(inset = false)
                    CheckRow(
                        title = name,
                        checked = true,
                        onToggle = { model.toggleBonjourAdapter(name) },
                        detail = stringResource(R.string.admin_network_bonjour_elsewhere),
                        enabled = !fixed.bonjour,
                    )
                }
            }
        }
    }
    // A server from before the setting has none to show.
    if (proxies != null) {
        item(key = "proxy") {
            SettingsGroup(
                stringResource(R.string.admin_network_proxy_title),
                Modifier.adminWidth,
                footer = stringResource(R.string.admin_network_proxy_description),
            ) {
                for (address in proxies) {
                    Row(
                        Modifier.fillMaxWidth().heightIn(min = 56.dp).padding(start = 16.dp, end = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(
                            address,
                            style = MaterialTheme.typography.bodyLarge,
                            color = PleyColors.Fg,
                            fontFamily = FontFamily.Monospace,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        PleyIconButton(
                            PleyIcons.X,
                            label = stringResource(R.string.admin_network_proxy_remove, address),
                            onClick = { model.removeProxy(address) },
                            enabled = busy == 0,
                            tint = PleyColors.Muted,
                        )
                    }
                    RowDivider(inset = false)
                }
                SettingsRow(
                    stringResource(R.string.admin_network_proxy_add),
                    icon = PleyIcons.Plus,
                    chevron = false,
                    enabled = busy == 0,
                    onClick = { open(NetworkDialog.AddProxy) },
                    modifier = Modifier.testTag(PROXY_ADD_TAG),
                )
            }
        }
    }
    item(key = "listening") {
        SettingsGroup(
            stringResource(R.string.admin_network_listening),
            Modifier.adminWidth,
            footer = stringResource(R.string.admin_network_listening_retries),
        ) {
            val onPleyPort = CompatApi.entries.filter { if (it == CompatApi.Jellyfin) network.jellyfinOnWeb else network.plexOnWeb }
            for (api in onPleyPort) {
                ListenerRow(ok = true, what = stringResource(R.string.admin_network_listening_on_pley_port, stringResource(listenerNameOf(api))))
            }
            for (listener in network.listeners) {
                ListenerRow(
                    ok = listener.error == null,
                    what = stringResource(kindNameOf(listener.kind)),
                    address = listener.address,
                    error = listener.error?.let { stringResource(R.string.admin_network_cant_listen, it) },
                )
            }
        }
    }
    if (page.unusual && !page.fixedAll) {
        item(key = "unusual") {
            NoteLine(
                stringResource(R.string.admin_network_unusual, (network.web + network.jellyfin + network.plex).joinToString(", ")),
                Modifier.adminWidth,
            )
        }
    }
}

/** What saving would do, Reset and Save: over the page's foot while something changed. */
@Composable
private fun SaveBar(page: NetworkPage, busy: Boolean, onReset: () -> Unit, onSave: () -> Unit, modifier: Modifier = Modifier) {
    val problem = page.problem
    val (message, danger) = when {
        page.edited && problem != null -> stringResource(problemOf(problem)) to true
        page.cutsOff -> stringResource(R.string.admin_network_would_stop) to true
        page.dirty -> stringResource(R.string.admin_network_unsaved) to false
        else -> null to false
    }
    Column(
        modifier
            .background(PleyColors.Surface2, PleyShape.Card)
            .padding(horizontal = 16.dp, vertical = 12.dp)
            .testTag(SAVE_BAR),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (message != null) {
            Text(message, style = MaterialTheme.typography.bodySmall, color = if (danger) PleyColors.DangerText else PleyColors.Muted)
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End, verticalAlignment = Alignment.CenterVertically) {
            PleyTextButton(stringResource(R.string.admin_network_reset), onClick = onReset, enabled = page.edited && !busy)
            Spacer(Modifier.width(8.dp))
            PrimaryButton(
                stringResource(R.string.admin_save),
                onClick = onSave,
                icon = PleyIcons.Save,
                enabled = page.dirty && problem == null && !busy,
                modifier = Modifier.testTag(SAVE_TAG),
            )
        }
    }
}

@Composable
private fun adapterTags(adapter: NetworkAdapter): List<RowTag> = listOfNotNull(
    adapter.network?.let { network ->
        kindOf(network.kind)?.let { RowTag(stringResource(it), danger = network.kind == "public") }
    },
    if (adapter.up) null else RowTag(stringResource(R.string.admin_network_not_connected)),
)

private fun kindOf(kind: String): Int? = when (kind) {
    "private" -> R.string.admin_network_kind_private
    "domain" -> R.string.admin_network_kind_domain
    "public" -> R.string.admin_network_kind_public
    else -> null
}

private fun modeLabelOf(mode: ListenMode): Int = when (mode) {
    ListenMode.Local -> R.string.admin_network_mode_local
    ListenMode.Networks -> R.string.admin_network_mode_networks
    ListenMode.All -> R.string.admin_network_mode_all
}

private fun modeHintOf(mode: ListenMode): Int = when (mode) {
    ListenMode.Local -> R.string.admin_network_mode_local_hint
    ListenMode.Networks -> R.string.admin_network_mode_networks_hint
    ListenMode.All -> R.string.admin_network_mode_all_hint
}

private fun listenerNameOf(api: CompatApi): Int = when (api) {
    CompatApi.Jellyfin -> R.string.admin_network_listener_jellyfin
    CompatApi.Plex -> R.string.admin_network_listener_plex
}

private fun kindNameOf(kind: ListenerKind): Int = when (kind) {
    ListenerKind.Web -> R.string.admin_network_listener_web
    ListenerKind.Jellyfin -> R.string.admin_network_listener_jellyfin
    ListenerKind.Plex -> R.string.admin_network_listener_plex
    ListenerKind.Discovery -> R.string.admin_network_listener_discovery
}

private fun hintOf(api: CompatApi): Int = when (api) {
    CompatApi.Jellyfin -> R.string.admin_network_jellyfin_hint
    CompatApi.Plex -> R.string.admin_network_plex_hint
}

private fun fixedOf(api: CompatApi): Int = when (api) {
    CompatApi.Jellyfin -> R.string.admin_network_jellyfin_fixed
    CompatApi.Plex -> R.string.admin_network_plex_fixed
}

private fun portChoiceOf(api: CompatApi): Int = when (api) {
    CompatApi.Jellyfin -> R.string.admin_network_jellyfin_port_choice
    CompatApi.Plex -> R.string.admin_network_plex_port_choice
}

private fun portLabelOf(api: CompatApi): Int = when (api) {
    CompatApi.Jellyfin -> R.string.admin_network_jellyfin_port_label
    CompatApi.Plex -> R.string.admin_network_plex_port_label
}

private fun portHintOf(api: CompatApi): Int = when (api) {
    CompatApi.Jellyfin -> R.string.admin_network_jellyfin_port_hint
    CompatApi.Plex -> R.string.admin_network_plex_port_hint
}

private fun problemOf(problem: NetworkProblem): Int = when (problem) {
    NetworkProblem.Port -> R.string.admin_network_problem_port
    NetworkProblem.JellyfinPortRange -> R.string.admin_network_jellyfin_port_range
    NetworkProblem.JellyfinSamePort -> R.string.admin_network_jellyfin_same_port
    NetworkProblem.PlexPortRange -> R.string.admin_network_plex_port_range
    NetworkProblem.PlexSamePort -> R.string.admin_network_plex_same_port
    NetworkProblem.SharedPort -> R.string.admin_network_problem_shared_port
    NetworkProblem.NoNetwork -> R.string.admin_network_problem_no_network
    NetworkProblem.HostName -> R.string.admin_network_problem_host_name
}

/** What a proxy address looks like, as the web's field shows it. */
private const val PROXY_EXAMPLE = "https://media.example.com:9999"

internal const val NETWORK_PAGE = "pley.admin.network"
internal const val MODE_TAG = "pley.admin.network.mode."
internal const val PORT_TAG = "pley.admin.network.port"
internal const val API_TAG = "pley.admin.network.api."
internal const val BONJOUR_TAG = "pley.admin.network.bonjour"
internal const val PROXY_ADD_TAG = "pley.admin.network.proxy.add"
internal const val SAVE_BAR = "pley.admin.network.bar"
internal const val SAVE_TAG = "pley.admin.network.save"
