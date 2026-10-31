using System;
using System.Collections.Generic;
using System.Globalization;

namespace Pley.Tv.Core.Player;

public enum SleepKind
{
    Off,
    Minutes,
    /// <summary>The end of the episode or film playing.</summary>
    EndOfItem,
}

/// <summary>What the sleep timer is set to. Not remembered between playbacks.</summary>
public sealed record SleepChoice(SleepKind Kind, int Minutes = 0)
{
    public static readonly SleepChoice Off = new SleepChoice(SleepKind.Off);
    public static readonly SleepChoice EndOfItem = new SleepChoice(SleepKind.EndOfItem);

    public static SleepChoice After(int minutes) => new SleepChoice(SleepKind.Minutes, minutes);

    /// <summary>The menu's choices, in order.</summary>
    public static readonly IReadOnlyList<SleepChoice> All = new[] { Off, After(15), After(30), After(45), After(60), After(90), EndOfItem };
}

/// <summary>Where the sleep timer stands.</summary>
/// <param name="SecondsLeft">Seconds before it pauses, during the notice (its last 30 s); null otherwise.</param>
/// <param name="MinutesLeft">Minutes before it pauses, rounded up, for the menu; null when off or unknown.</param>
/// <param name="Fired">It paused the playback, which hasn't played since.</param>
public sealed record SleepState(SleepChoice Choice, int? SecondsLeft = null, int? MinutesLeft = null, bool Fired = false)
{
    public static readonly SleepState Idle = new SleepState(SleepChoice.Off);
}

/// <summary>
/// The sleep timer (the Android app's SleepTimer): after the minutes chosen, or at the end of what
/// plays, it pauses (<c>onFire</c>). 30 s before, <see cref="State"/> says so once a second for a
/// notice; <see cref="Extend"/> ("Keep watching") adds the same time again. Time is the
/// scheduler's monotonic clock; the UI thread only.
/// </summary>
public sealed class SleepTimer
{
    /// <summary>The notice shows this long before the timer pauses.</summary>
    public const long NoticeMs = 30_000;

    /// <summary>On TV, paused by the timer this long, the player leaves.</summary>
    public const long LeaveAfterMs = 10 * 60_000;

    const long SecondMs = 1_000;
    const long MinuteMs = 60_000;

    readonly IScheduler scheduler;
    readonly Func<long?> itemLeftMs;
    readonly Func<object?> itemKey;
    readonly Action onFire;
    IDisposable? timer;
    /// <summary>When a timer of minutes ends, on the scheduler's monotonic clock.</summary>
    long deadline;
    /// <summary>"Keep watching" at the end of this item: the timer waits for the next one's end.</summary>
    object? spared;

    /// <param name="itemLeftMs">How long the item playing has left, in ms of playback; null while unknown.</param>
    /// <param name="itemKey">Tells items apart: the one playing now.</param>
    public SleepTimer(IScheduler scheduler, Func<long?> itemLeftMs, Func<object?> itemKey, Action onFire)
    {
        this.scheduler = scheduler;
        this.itemLeftMs = itemLeftMs;
        this.itemKey = itemKey;
        this.onFire = onFire;
    }

    public Signal<SleepState> State { get; } = new Signal<SleepState>(SleepState.Idle);

    public void Set(SleepChoice choice)
    {
        Stop();
        spared = null;
        State.Value = new SleepState(choice);
        switch (choice.Kind)
        {
            case SleepKind.Off:
                return;
            case SleepKind.Minutes:
                deadline = scheduler.Elapsed + choice.Minutes * MinuteMs;
                break;
        }
        Run();
    }

    /// <summary>Adds the same time again: the minutes chosen, or until the end of the next item.</summary>
    public void Extend()
    {
        var choice = State.Value.Choice;
        switch (choice.Kind)
        {
            case SleepKind.Off:
                return;
            case SleepKind.Minutes:
                deadline += choice.Minutes * MinuteMs;
                break;
            case SleepKind.EndOfItem:
                spared = itemKey();
                break;
        }
        State.Value = State.Value with { SecondsLeft = null };
        Run();
    }

    /// <summary>Whether the timer ends with the item <paramref name="key"/>: what comes next waits.</summary>
    public bool EndsWith(object? key) => State.Value.Choice.Kind == SleepKind.EndOfItem && !Equals(key, spared);

    /// <summary>The item <paramref name="key"/> played to its end: true when that is the timer's end, which then fires (the caller holds what comes next).</summary>
    public bool ItemEnded(object? key)
    {
        if (!EndsWith(key)) return false;
        Fire();
        return true;
    }

    /// <summary>It plays again: the pause the timer made is over.</summary>
    public void Resumed()
    {
        if (State.Value.Fired) State.Value = State.Value with { Fired = false };
    }

    public void Cancel()
    {
        Stop();
        State.Value = SleepState.Idle;
    }

    void Stop()
    {
        timer?.Dispose();
        timer = null;
    }

    void Run()
    {
        Stop();
        Tick();
    }

    void Tick()
    {
        timer = null;
        var choice = State.Value.Choice;
        if (choice.Kind == SleepKind.Off) return;
        var key = itemKey();
        long? left = choice.Kind == SleepKind.Minutes ? deadline - scheduler.Elapsed
            : Equals(key, spared) ? null
            : itemLeftMs();
        if (spared != null && !Equals(key, spared)) spared = null;
        if (left == null) Publish(null, null);
        else if (choice.Kind == SleepKind.Minutes && left <= 0)
        {
            Fire();
            return;
        }
        // The end of the item fires through ItemEnded, when it truly ends.
        else if (left <= 0) Publish(null, null);
        else if (left <= NoticeMs) Publish((int)((left.Value + SecondMs - 1) / SecondMs), 1);
        else Publish(null, (int)((left.Value + MinuteMs - 1) / MinuteMs));
        timer = scheduler.After(Wait(choice, left), Tick);
    }

    /// <summary>Until the next second during the notice; else until the notice or the next minute, looking at an item's end each second.</summary>
    static long Wait(SleepChoice choice, long? left)
    {
        if (left == null || left <= 0 || choice.Kind == SleepKind.EndOfItem) return SecondMs;
        if (left <= NoticeMs) return left.Value % SecondMs is var s && s > 0 ? s : SecondMs;
        var minute = left.Value % MinuteMs is var m && m > 0 ? m : MinuteMs;
        return Math.Min(left.Value - NoticeMs, minute);
    }

    void Publish(int? secondsLeft, int? minutesLeft) => State.Value = State.Value with { SecondsLeft = secondsLeft, MinutesLeft = minutesLeft };

    void Fire()
    {
        Stop();
        spared = null;
        State.Value = new SleepState(SleepChoice.Off, Fired: true);
        onFire();
    }
}

/// <summary>How the sleep timer's menu says things (the Android app's SleepTexts).</summary>
public static class SleepTexts
{
    public const string Title = "Sleep timer";
    public const string Keep = "Keep watching";

    /// <summary>A choice: "Off", "30 min", "End of this episode".</summary>
    public static string Choice(SleepChoice choice) => choice.Kind switch
    {
        SleepKind.Off => "Off",
        SleepKind.Minutes => choice.Minutes.ToString(CultureInfo.InvariantCulture) + " min",
        _ => "End of this episode",
    };

    /// <summary>What the menu row says now: "Off", "42 min left", "End of this episode".</summary>
    public static string Summary(SleepState state) =>
        state.Choice.Kind == SleepKind.Minutes && state.MinutesLeft is int minutes
            ? minutes.ToString(CultureInfo.InvariantCulture) + " min left"
            : Choice(state.Choice);

    /// <summary>"Pausing in 30 s".</summary>
    public static string Notice(int secondsLeft) => $"Pausing in {secondsLeft.ToString(CultureInfo.InvariantCulture)} s";
}

/// <summary>The playback speeds offered (the Android app's PlaybackSpeeds): 0.5× to 2×, 1× normal, for one title at a time.</summary>
public static class PlaybackSpeeds
{
    public static readonly IReadOnlyList<double> All = new[] { 0.5, 0.75, 1, 1.25, 1.5, 2 };

    /// <summary>"1×", "1.25×", "0.5×".</summary>
    public static string Label(double speed) => speed.ToString("0.##", CultureInfo.InvariantCulture) + "×";
}

/// <summary>The audio delay's range and step (the Android app's AudioDelays).</summary>
public static class AudioDelays
{
    public const long MaxMs = 5_000;
    public const long StepMs = 50;

    /// <summary><paramref name="ms"/> within ±5 s, on a 50 ms step.</summary>
    public static long Clamp(long ms) => Math.Max(-MaxMs, Math.Min(MaxMs, SubtitleSync.Round(ms / (double)StepMs) * StepMs));

    /// <summary>"+0.25 s", "−1.5 s", "0 s": the sound later (+) or earlier (−) than the picture.</summary>
    public static string Label(long ms)
    {
        if (ms == 0) return "0 s";
        var seconds = (Math.Abs(ms) / 1000m).ToString("0.###", CultureInfo.InvariantCulture);
        return (ms > 0 ? "+" : "−") + seconds + " s";
    }
}
