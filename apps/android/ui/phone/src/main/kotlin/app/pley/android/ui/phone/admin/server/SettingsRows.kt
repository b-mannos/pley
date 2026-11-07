package app.pley.android.ui.phone.admin.server

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcons

/*
 * The rows of the pages that edit the server's settings (Server,
 * Requests): as the web's Settings rows, a title, what is chosen, the line
 * that says what it does, and a note under it (why it is greyed out, or
 * what happens instead), in the account kit's groups.
 */

/** A line under a row: why it is greyed out, or ([notice]) what doesn't work as it is. [onClick] makes it a link. */
internal data class RowNote(val text: String, val notice: Boolean = false, val onClick: (() -> Unit)? = null)

/**
 * A row that opens a choice or a dialog: [title], [value] (what is chosen),
 * [hint] and [note] under them, and a chevron. Disabled rows are shown at
 * 38 % and do nothing.
 */
@Composable
internal fun ValueRow(
    title: String,
    value: String?,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    hint: String? = null,
    note: RowNote? = null,
    enabled: Boolean = true,
    valueColor: Color? = null,
    trailing: (@Composable () -> Unit)? = null,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = if (enabled) PleyColors.Fg else PleyColors.Disabled)
            if (!value.isNullOrEmpty()) {
                Text(
                    value,
                    style = MaterialTheme.typography.bodyMedium,
                    color = if (enabled) valueColor ?: PleyColors.Muted else PleyColors.Disabled,
                    maxLines = 3,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            RowLines(hint, note)
        }
        Spacer(Modifier.width(12.dp))
        if (trailing != null) {
            trailing()
        } else if (enabled) {
            Icon(PleyIcons.ChevronRight, contentDescription = null, size = 20.dp, tint = PleyColors.Subtle)
        }
    }
}

/** A row with a switch, which the whole row toggles; disabled, it shows and does nothing. */
@Composable
internal fun ToggleRow(
    title: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    hint: String? = null,
    note: RowNote? = null,
    enabled: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = 56.dp)
            .toggleable(value = checked, enabled = enabled, role = Role.Switch, onValueChange = onChange)
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge, color = if (enabled) PleyColors.Fg else PleyColors.Disabled)
            RowLines(hint, note)
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

/** A row's hint and note. */
@Composable
private fun ColumnScope.RowLines(hint: String?, note: RowNote?) {
    if (hint != null) {
        Text(hint, style = MaterialTheme.typography.bodySmall, color = PleyColors.Subtle, modifier = Modifier.padding(top = 2.dp))
    }
    if (note != null) NoteLine(note, Modifier.padding(top = 6.dp))
}

/** A line at the top of a group that says what it is for. */
@Composable
internal fun GroupLine(text: String) {
    Text(
        text,
        style = MaterialTheme.typography.bodySmall,
        color = PleyColors.Muted,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp),
    )
}

/** A [RowNote] on its own: an icon (a warning for a notice) and its words. */
@Composable
internal fun NoteLine(note: RowNote, modifier: Modifier = Modifier) {
    val link = note.onClick
    Row(
        modifier.then(if (link != null) Modifier.clickable(role = Role.Button, onClick = link) else Modifier),
        verticalAlignment = Alignment.Top,
    ) {
        Icon(
            if (note.notice) PleyIcons.CircleAlert else PleyIcons.Info,
            contentDescription = null,
            size = 14.dp,
            tint = if (note.notice) PleyColors.DangerText else PleyColors.Subtle,
            modifier = Modifier.padding(top = 2.dp),
        )
        Spacer(Modifier.width(6.dp))
        Text(
            note.text,
            style = MaterialTheme.typography.bodySmall,
            color = when {
                link != null -> PleyColors.AccentBright
                note.notice -> PleyColors.Fg
                else -> PleyColors.Subtle
            },
        )
    }
}
