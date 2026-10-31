using System;
using System.Globalization;
using System.Text.RegularExpressions;

namespace Pley.Tv.Core.Player;

/// <summary>How the player writes times and titles (the Android app's TvPlayerText).</summary>
public static class PlayerText
{
    /// <summary>"4:05", "32:40", "1:02:03".</summary>
    public static string Time(long ms)
    {
        var total = Math.Max(0, ms) / 1000;
        var h = total / 3600;
        var m = total % 3600 / 60;
        var s = total % 60;
        return h > 0
            ? string.Format(CultureInfo.InvariantCulture, "{0}:{1:00}:{2:00}", h, m, s)
            : string.Format(CultureInfo.InvariantCulture, "{0}:{1:00}", m, s);
    }

    /// <summary>What is left, with a real minus: "−25:20".</summary>
    public static string Remaining(long positionMs, long durationMs) => "−" + Time(Math.Max(0, durationMs - positionMs));

    static readonly Regex EpisodeNumber = new Regex(@"\bS(\d+) E(\d+)\b");

    /// <summary>An episode's line as the player shows it: "S1 E2 — Pilot" becomes "S1 · E2 · Pilot".</summary>
    public static string? EpisodeLine(string? subtitle)
    {
        if (string.IsNullOrWhiteSpace(subtitle)) return null;
        return EpisodeNumber.Replace(subtitle, "S$1 · E$2").Replace(" — ", " · ");
    }

    /// <summary>The player's title: an episode's show, a film with its year ("Heat (1995)").</summary>
    public static string Title(Api.ItemSummary item) =>
        item.Kind == "episode" ? item.GrandparentTitle ?? item.Title
        : item.Year != null ? $"{item.Title} ({item.Year.Value.ToString(CultureInfo.InvariantCulture)})"
        : item.Title;

    /// <summary>The line under an episode's title: "S2 · E5 · The Long Night"; null for anything else.</summary>
    public static string? Line(Api.ItemSummary item)
    {
        if (item.Kind != "episode") return null;
        var parts = new System.Collections.Generic.List<string>();
        if (item.ParentIndex != null) parts.Add("S" + item.ParentIndex.Value.ToString(CultureInfo.InvariantCulture));
        if (item.Index != null) parts.Add("E" + item.Index.Value.ToString(CultureInfo.InvariantCulture));
        if (item.Title.Length > 0) parts.Add(item.Title);
        return string.Join(" · ", parts);
    }

    /// <summary>"−10 s"'s number: whole seconds, never 0 for a real step.</summary>
    public static int Seconds(long ms) => (int)((Math.Abs(ms) + 500) / 1000);

    /// <summary>The seek badge: "+30 s", "−10 s".</summary>
    public static string Badge(long ms) => (ms < 0 ? "−" : "+") + Seconds(ms).ToString(CultureInfo.InvariantCulture) + " s";
}
