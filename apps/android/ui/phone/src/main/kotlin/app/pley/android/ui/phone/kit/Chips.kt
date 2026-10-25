package app.pley.android.ui.phone.kit

import androidx.compose.foundation.layout.height
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.PleyShape
import app.pley.android.designsystem.icon.Icon
import app.pley.android.designsystem.icon.PleyIcon
import app.pley.android.designsystem.icon.PleyIcons

/**
 * A filter or sort chip (design spec §3.4): a 32 dp pill in
 * [surface3][PleyColors.Surface3] with a 48 dp touch target. Selected, it
 * turns [primaryContainer][PleyColors.PrimaryContainer] with
 * [accentBright][PleyColors.AccentBright] text and a check. [icon] leads an
 * unselected chip (the sort's arrows); [dropdown] adds a chevron for
 * chips that open a menu or sheet (Genre, Year).
 */
@Composable
fun PleyChip(
    label: String,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    icon: PleyIcon? = null,
    dropdown: Boolean = false,
    enabled: Boolean = true,
) {
    val content = if (selected) PleyColors.AccentBright else PleyColors.Fg
    FilterChip(
        selected = selected,
        onClick = onClick,
        modifier = modifier.height(32.dp),
        enabled = enabled,
        shape = PleyShape.Pill,
        label = { Text(label, style = MaterialTheme.typography.labelMedium, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        leadingIcon = when {
            selected -> ({ Icon(PleyIcons.Check, contentDescription = null, size = 18.dp, tint = content) })
            icon != null -> ({ Icon(icon, contentDescription = null, size = 18.dp, tint = content) })
            else -> null
        },
        trailingIcon = if (dropdown) ({ Icon(PleyIcons.ChevronDown, contentDescription = null, size = 18.dp, tint = content) }) else null,
        border = null,
        colors = FilterChipDefaults.filterChipColors(
            containerColor = PleyColors.Surface3,
            labelColor = PleyColors.Fg,
            iconColor = PleyColors.Fg,
            selectedContainerColor = PleyColors.PrimaryContainer,
            selectedLabelColor = PleyColors.AccentBright,
            selectedLeadingIconColor = PleyColors.AccentBright,
            selectedTrailingIconColor = PleyColors.AccentBright,
        ),
    )
}

/**
 * A choice of a few (design spec §3.4): Material's segmented buttons, the
 * chosen one in [primaryContainer][PleyColors.PrimaryContainer] with
 * [accentBright][PleyColors.AccentBright], the rest on the page, edged in
 * [control][PleyColors.Control]. Screen readers hear a radio group.
 */
@Composable
fun SegmentedControl(
    options: List<String>,
    selected: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    SingleChoiceSegmentedButtonRow(modifier) {
        options.forEachIndexed { index, option ->
            SegmentedButton(
                selected = index == selected,
                onClick = { onSelect(index) },
                shape = SegmentedButtonDefaults.itemShape(index, options.size),
                colors = SegmentedButtonDefaults.colors(
                    activeContainerColor = PleyColors.PrimaryContainer,
                    activeContentColor = PleyColors.AccentBright,
                    activeBorderColor = PleyColors.Control,
                    inactiveContainerColor = PleyColors.Bg,
                    inactiveContentColor = PleyColors.Fg,
                    inactiveBorderColor = PleyColors.Control,
                ),
                label = { Text(option, style = MaterialTheme.typography.labelLarge, maxLines = 1, overflow = TextOverflow.Ellipsis) },
            )
        }
    }
}
