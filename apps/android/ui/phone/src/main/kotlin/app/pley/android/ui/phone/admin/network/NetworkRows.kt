package app.pley.android.ui.phone.admin.network

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons

/**
 * Network's own rows, beside the account kit's: a switch that can be
 * locked (what the command line sets), one of several choices shown at
 * once (where the server listens, as the web lists them), a checked row
 * (networks, Bonjour's adapters) and a line of what listens now.
 */

/** A row with a switch, as [app.pley.android.ui.phone.account.SwitchRow], shown at 38 % and fixed when not [enabled]. */
@Composable
internal fun LockableSwitchRow(
    title: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    subtitle: String? = null,
    enabled: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = if (enabled) PleyColors.Fg else PleyColors.Disabled)
            if (subtitle != null) Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted)
        }
        Spacer(Modifier.width(12.dp))
        Switch(
            checked = checked,
            onCheckedChange = null,
            enabled = enabled,
            colors = SwitchDefaults.colors(
                checkedThumbColor = PleyColors.OnAccent,
                checkedTrackColor = PleyColors.Accent,
                uncheckedThumbColor = PleyColors.Control,
                uncheckedTrackColor = PleyColors.Surface3,
                uncheckedBorderColor = PleyColors.Control,
            ),
        )
    }
}

/** One of several choices listed at once: its [title], the [hint] under it, a check on the [selected] one. */
@Composable
internal fun RadioRow(
    title: String,
    hint: String?,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .selectable(selected = selected, enabled = enabled, role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = when {
                    !enabled -> PleyColors.Disabled
                    selected -> PleyColors.AccentBright
                    else -> PleyColors.Fg
                },
            )
            if (hint != null) Text(hint, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted)
        }
        Spacer(Modifier.width(12.dp))
        Box(Modifier.size(24.dp), contentAlignment = Alignment.Center) {
            if (selected) Icon(PleyIcons.Check, contentDescription = null, size = 22.dp, tint = if (enabled) PleyColors.Accent else PleyColors.Disabled)
        }
    }
}

/** A word beside a [CheckRow]'s title ("Private network"); [danger] ones read in red ("Public network"). */
internal data class RowTag(val text: String, val danger: Boolean = false)

/** A row with a checkbox: [title], [tags] beside it, and [detail] under it, in one line. */
@Composable
internal fun CheckRow(
    title: String,
    checked: Boolean,
    onToggle: () -> Unit,
    modifier: Modifier = Modifier,
    detail: String? = null,
    tags: List<RowTag> = emptyList(),
    enabled: Boolean = true,
    mono: Boolean = false,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .toggleable(value = checked, enabled = enabled, role = Role.Checkbox, onValueChange = { onToggle() })
            .padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Checkbox(
            checked = checked,
            onCheckedChange = null,
            enabled = enabled,
            colors = CheckboxDefaults.colors(
                checkedColor = PleyColors.Accent,
                uncheckedColor = PleyColors.Control,
                checkmarkColor = PleyColors.OnAccent,
            ),
        )
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            Text(
                title,
                style = MaterialTheme.typography.bodyLarge,
                color = if (enabled) PleyColors.Fg else PleyColors.Disabled,
                fontFamily = if (mono) FontFamily.Monospace else null,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (tags.isNotEmpty()) {
                Row {
                    for ((i, tag) in tags.withIndex()) {
                        if (i > 0) Text(" · ", style = MaterialTheme.typography.labelMedium, color = PleyColors.Subtle)
                        Text(tag.text, style = MaterialTheme.typography.labelMedium, color = if (tag.danger) PleyColors.DangerText else PleyColors.Info)
                    }
                }
            }
            if (!detail.isNullOrEmpty()) {
                Text(detail, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted, maxLines = 2, overflow = TextOverflow.Ellipsis)
            }
        }
    }
}

/** A line of what listens now: [ok] or not, the [address] (none for an API on the server's port), what listens, and why it can't. */
@Composable
internal fun ListenerRow(ok: Boolean, what: String, modifier: Modifier = Modifier, address: String? = null, error: String? = null) {
    Row(modifier.fillMaxWidth().heightIn(min = 48.dp).padding(horizontal = 16.dp, vertical = 10.dp)) {
        Icon(
            if (ok) PleyIcons.CircleCheck else PleyIcons.CircleAlert,
            contentDescription = null,
            size = 20.dp,
            tint = if (ok) PleyColors.Success else PleyColors.DangerText,
            modifier = Modifier.padding(top = 2.dp),
        )
        Spacer(Modifier.width(16.dp))
        Column(Modifier.weight(1f)) {
            if (address != null) {
                Text(address, style = MaterialTheme.typography.bodyLarge, color = PleyColors.Fg, fontFamily = FontFamily.Monospace)
                Text(what, style = MaterialTheme.typography.bodyMedium, color = PleyColors.Muted)
            } else {
                Text(what, style = MaterialTheme.typography.bodyLarge, color = PleyColors.Muted)
            }
            if (error != null) Text(error, style = MaterialTheme.typography.bodyMedium, color = PleyColors.DangerText)
        }
    }
}

/** A note under or between groups, with an [icon]: what the page can't change, or how it will save. */
@Composable
internal fun NoteLine(text: String, modifier: Modifier = Modifier, icon: PleyIcon = PleyIcons.Info, color: Color = PleyColors.Muted) {
    Row(modifier.padding(horizontal = 4.dp)) {
        Icon(icon, contentDescription = null, size = 16.dp, tint = color, modifier = Modifier.padding(top = 2.dp))
        Spacer(Modifier.width(8.dp))
        Text(text, style = MaterialTheme.typography.bodySmall, color = color)
    }
}
