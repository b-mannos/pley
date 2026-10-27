package app.pley.android.ui.tv.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyType
import app.pley.android.screens.onboarding.ServersUiState
import app.pley.android.ui.tv.R
import app.pley.android.ui.tv.kit.TvPillButton
import app.pley.android.ui.tv.kit.TvTextField
import app.pley.android.ui.tv.kit.digitKeys
import kotlinx.coroutines.flow.drop

/**
 * The address pad (design spec §2.11; tv-addendum §4.4): the PIN pad's
 * keys with "." and ":", ⌫ and Connect under them, above a field that
 * starts with the TV's network ("192.168.1."). The remote's digit keys
 * type too; Select on the field opens the system keyboard, for names.
 * Port 7539 is added when none is typed.
 */
@Composable
internal fun TvAddressPad(
    state: ServersUiState,
    onConnect: (String) -> Unit,
    onEdited: () -> Unit,
) {
    val field = rememberTextFieldState(state.subnet.orEmpty())
    LaunchedEffect(state.subnet) {
        val subnet = state.subnet
        if (subnet != null && field.text.isEmpty()) field.setTextAndPlaceCursorAtEnd(subnet)
    }
    LaunchedEffect(field) {
        snapshotFlow { field.text.toString() }.drop(1).collect { onEdited() }
    }
    val flash = remember { PadFlash() }
    val labels = padLabels()
    val connect = { if (field.text.isNotBlank()) onConnect(field.text.toString().trim()) }
    Box(
        Modifier
            .fillMaxSize()
            .brandGlow()
            .digitKeys(
                onDigit = { d ->
                    flash.flash(PadKey.Digit(d))
                    field.type(d.toString())
                },
                onDelete = {
                    flash.flash(PadKey.Delete)
                    field.deleteLast()
                },
                onDot = {
                    flash.flash(PadKey.Symbol("."))
                    field.type(".")
                },
                onText = { c ->
                    flash.flash(PadKey.Symbol(c.toString()))
                    field.type(c.toString())
                },
            )
            .testTag(ADDRESS_PAD_TAG),
    ) {
        // Beside the pad: how addresses work, and what went wrong.
        Column(
            Modifier.padding(start = 96.dp, top = 121.dp).width(240.dp).semantics { liveRegion = LiveRegionMode.Polite },
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            BasicText(stringResource(R.string.tv_onboarding_address_hint), style = PleyType.Tv.Caption.copy(color = PleyColors.Muted))
            state.addressProblem?.let {
                BasicText(problemText(it), style = PleyType.Tv.Body.copy(color = PleyColors.DangerText), modifier = Modifier.testTag(ADDRESS_PROBLEM_TAG))
            }
        }
        Column(
            Modifier.fillMaxWidth().padding(top = 27.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            BasicText(stringResource(R.string.tv_onboarding_address_title), style = PleyType.Tv.Title.copy(color = PleyColors.Fg))
            Spacer(Modifier.height(8.dp))
            TvTextField(
                state = field,
                placeholder = stringResource(R.string.tv_onboarding_address_placeholder),
                modifier = Modifier.width(360.dp).testTag(ADDRESS_FIELD_TAG),
                keyboardType = KeyboardType.Uri,
                imeAction = ImeAction.Go,
                onAction = connect,
            )
            Spacer(Modifier.height(14.dp))
            TvKeypad(
                rows = AddressKeys,
                labels = labels,
                flash = flash,
                onKey = { key ->
                    when (key) {
                        is PadKey.Digit -> field.type(key.value.toString())
                        is PadKey.Symbol -> field.type(key.text)
                        PadKey.Delete -> field.deleteLast()
                        PadKey.Ok -> connect()
                    }
                },
            )
            Spacer(Modifier.height(KeyGap))
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(KeyGap)) {
                TvPadKey(
                    key = PadKey.Delete,
                    labels = labels,
                    enabled = true,
                    flashing = flash.key == PadKey.Delete,
                    initial = false,
                    onClick = {
                        flash.flash(PadKey.Delete)
                        field.deleteLast()
                    },
                )
                TvPillButton(
                    label = stringResource(if (state.checkingAddress) R.string.tv_onboarding_checking else R.string.tv_onboarding_connect),
                    onClick = connect,
                    modifier = Modifier.testTag(CONNECT_TAG),
                )
            }
        }
    }
}

/** What screen readers hear for the pads' keys. */
@Composable
internal fun padLabels(): PadLabels {
    val resources = LocalResources.current
    val delete = stringResource(R.string.tv_onboarding_delete)
    val ok = stringResource(R.string.tv_onboarding_ok)
    return remember(resources, delete, ok) { PadLabels({ n -> resources.getString(R.string.tv_onboarding_digit, n) }, delete, ok) }
}

private fun TextFieldState.type(text: String) = edit { append(text) }

private fun TextFieldState.deleteLast() = edit { if (length > 0) replace(length - 1, length, "") }

internal const val ADDRESS_PAD_TAG = "pley.servers.pad"
internal const val ADDRESS_FIELD_TAG = "pley.servers.pad.field"
internal const val ADDRESS_PROBLEM_TAG = "pley.servers.pad.problem"
internal const val CONNECT_TAG = "pley.servers.pad.connect"
