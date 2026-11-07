package app.pley.android.ui.phone.admin.server

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.pley.android.designsystem.PleyColors
import app.pley.android.designsystem.text.shownLocale
import app.pley.android.model.admin.LibraryWorkWeek
import app.pley.android.ui.phone.R
import java.time.DayOfWeek
import java.time.LocalTime
import java.time.format.DateTimeFormatter
import java.time.format.FormatStyle
import java.time.format.TextStyle
import java.util.Locale
import kotlin.math.roundToInt

/** Monday first; the server counts the week from Sunday (0). */
private val DAYS = listOf(1, 2, 3, 4, 5, 6, 0)

/** Hours of a day, a week of days. */
private const val HOURS = 24

/** The width of the days' names, and the gap between cells. */
private val DAY_LABEL = 32.dp
private val GAP = 2.dp

/** Day [day] of the server's week (0: Sunday), short, in [locale]. */
private fun dayName(day: Int, locale: Locale): String =
    DayOfWeek.of(if (day == 0) 7 else day).getDisplayName(TextStyle.SHORT, locale)

/** "9:00 PM", "21:00": an hour of the day as [locale] writes the time. */
internal fun hourOfDay(hour: Int, locale: Locale): String =
    DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT).withLocale(locale).format(LocalTime.of(hour, 0))

/** "9 PM", "21": an hour alone, as the axis shows it. */
private fun hourName(hour: Int, locale: Locale): String {
    val pattern = android.text.format.DateFormat.getBestDateTimePattern(locale, "j")
    return DateTimeFormatter.ofPattern(pattern, locale).format(LocalTime.of(hour, 0))
}

/** How much a cell shows of the accent: from a trace of it (rarely watched) to all of it, as the web mixes it. */
private fun shade(playing: Double): Color = lerp(PleyColors.Surface2, PleyColors.Accent, (0.08 + 0.92 * playing.coerceIn(0.0, 1.0)).toFloat())

/**
 * The week as the server learned it (the web's WatchingHoursChart): how
 * much of each hour something played, and the hours work on the rest of
 * the libraries may begin in, drawn on a canvas a phone's width shows
 * whole. Touching an hour says what it is, as hovering does on the web.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun WatchingHoursChart(week: LibraryWorkWeek, modifier: Modifier = Modifier) {
    val locale = shownLocale()
    var picked by rememberSaveable { mutableStateOf<Int?>(null) }
    val title = stringResource(R.string.admin_server_when_people_watch)
    Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val density = LocalDensity.current
            val cell = (maxWidth - DAY_LABEL - GAP * (HOURS - 1)) / HOURS
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row {
                    Spacer(Modifier.width(DAY_LABEL))
                    for (h in listOf(0, 6, 12, 18)) {
                        Text(
                            hourName(h, locale),
                            style = MaterialTheme.typography.labelSmall,
                            color = PleyColors.Subtle,
                            maxLines = 1,
                            overflow = TextOverflow.Clip,
                            modifier = Modifier.width((cell + GAP) * 6),
                        )
                    }
                }
                Row {
                    Column(Modifier.width(DAY_LABEL), verticalArrangement = Arrangement.spacedBy(GAP)) {
                        for (d in DAYS) {
                            Box(Modifier.height(cell), contentAlignment = Alignment.CenterStart) {
                                Text(dayName(d, locale), style = MaterialTheme.typography.labelSmall, color = PleyColors.Subtle, maxLines = 1)
                            }
                        }
                    }
                    val cellPx = with(density) { cell.toPx() }
                    val gapPx = with(density) { GAP.toPx() }
                    val ring = with(density) { 2.dp.toPx() }
                    val hairline = with(density) { 1.dp.toPx() }
                    val dot = with(density) { 2.dp.toPx() }
                    val dashes = remember(density) { PathEffect.dashPathEffect(floatArrayOf(dot * 1.5f, dot)) }
                    Canvas(
                        Modifier
                            .width(cell * HOURS + GAP * (HOURS - 1))
                            .height(cell * DAYS.size + GAP * (DAYS.size - 1))
                            .semantics { contentDescription = title }
                            .pointerInput(week) {
                                detectTapGestures { at ->
                                    val h = (at.x / (cellPx + gapPx)).toInt().coerceIn(0, HOURS - 1)
                                    val row = (at.y / (cellPx + gapPx)).toInt().coerceIn(0, DAYS.size - 1)
                                    val hour = DAYS[row] * HOURS + h
                                    picked = if (picked == hour) null else hour
                                }
                            },
                    ) {
                        val corner = CornerRadius(cellPx * 0.2f)
                        for ((row, d) in DAYS.withIndex()) {
                            for (h in 0 until HOURS) {
                                val at = d * HOURS + h
                                val hour = week.hours.getOrNull(at)
                                val topLeft = Offset(h * (cellPx + gapPx), row * (cellPx + gapPx))
                                val size = Size(cellPx, cellPx)
                                val playing = hour?.playing
                                if (playing == null) {
                                    drawRoundRect(
                                        PleyColors.BorderStrong,
                                        topLeft + Offset(hairline / 2, hairline / 2),
                                        Size(cellPx - hairline, cellPx - hairline),
                                        corner,
                                        style = Stroke(hairline, pathEffect = dashes),
                                    )
                                } else {
                                    drawRoundRect(shade(playing), topLeft, size, corner)
                                }
                                if (hour?.open == true) {
                                    drawCircle(PleyColors.Fg.copy(alpha = 0.9f), dot, topLeft + Offset(cellPx / 2, cellPx / 2))
                                }
                                if (at == week.now || at == picked) {
                                    drawRoundRect(
                                        if (at == picked) PleyColors.AccentBright else PleyColors.Fg,
                                        topLeft - Offset(ring / 2, ring / 2),
                                        Size(cellPx + ring, cellPx + ring),
                                        corner,
                                        style = Stroke(ring),
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
        val hour = picked
        Text(
            if (hour == null) stringResource(R.string.admin_server_chart_touch) else hourLabel(week, hour, locale),
            style = MaterialTheme.typography.bodySmall,
            color = if (hour == null) PleyColors.Subtle else PleyColors.Fg,
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Legend(stringResource(R.string.admin_server_rarely))
                for (p in listOf(0.0, 0.33, 0.66, 1.0)) {
                    Box(Modifier.size(12.dp).background(shade(p), RoundedCornerShape(2.dp)))
                }
                Legend(stringResource(R.string.admin_server_often))
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.size(12.dp).border(1.dp, PleyColors.BorderStrong, RoundedCornerShape(2.dp)))
                Legend(stringResource(R.string.admin_server_not_seen_yet))
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Box(Modifier.size(12.dp).background(PleyColors.Surface3, RoundedCornerShape(2.dp)), contentAlignment = Alignment.Center) {
                    Box(Modifier.size(4.dp).background(PleyColors.Fg, CircleShape))
                }
                Legend(stringResource(R.string.admin_server_may_begin))
            }
        }
    }
}

@Composable
private fun Legend(text: String) {
    Text(text, style = MaterialTheme.typography.labelSmall, color = PleyColors.Subtle)
}

/** What [hour] of the week is, as the web's cell says it: "Mon 9:00 PM: something played 40% of the time". */
@Composable
private fun hourLabel(week: LibraryWorkWeek, hour: Int, locale: Locale): String {
    val cell = week.hours.getOrNull(hour)
    val playing = cell?.playing
    val what = if (playing == null) {
        stringResource(R.string.admin_server_not_seen_enough)
    } else {
        stringResource(R.string.admin_server_played, (playing * 100).roundToInt())
    }
    val day = dayName(hour / HOURS, locale)
    val time = hourOfDay(hour % HOURS, locale)
    return stringResource(if (cell?.open == true) R.string.admin_server_hour_open else R.string.admin_server_hour, day, time, what)
}
