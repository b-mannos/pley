using System;
using System.Collections.Generic;

namespace Pley.Tv.Core.Ui;

/// <summary>
/// D-pad rule 5, held keys (the Android app's KeyRepeatGate). A press moves focus once. Held for
/// 250 ms, focus moves on the app's own clock (<see cref="Tick"/>): sooner than the TV's repeats,
/// which begin after 400 ms and come unevenly, and only say the key is still down. It speeds up
/// the longer the key is held, as the PSP's XMB did: from 12.5 moves a second to 40 within 0.7 s,
/// smoothly. Rows glide and tiles change their focus look evenly over a little more than the time
/// between moves (<see cref="GlideMs"/>), so each step runs into the next without a pause; and
/// backdrops and details wait until focus has rested 400 ms (<see cref="Rested"/>).
/// NUI's key events carry no repeat count: a key that goes down again before coming up repeats.
/// </summary>
public sealed class KeyRepeatGate
{
    /// <summary>How long a key is down before focus starts moving by itself.</summary>
    public const long HoldDelayMs = 250;
    /// <summary>The time between moves when a hold begins: 12.5 a second.</summary>
    public const long MoveIntervalMs = 80;
    /// <summary>The time between moves at full speed: 40 a second.</summary>
    public const long FastestIntervalMs = 25;
    /// <summary>How long into a hold focus reaches full speed.</summary>
    public const long RampMs = 700;
    /// <summary>How long focus rests before backdrops and details follow it.</summary>
    public const long RestMs = 400;
    /// <summary>Repeats come every 50 ms; this long without one means the key is up.</summary>
    const long LostKeyUpMs = 500;

    static readonly HashSet<string> Directions = new HashSet<string> { "Up", "Down", "Left", "Right" };

    readonly Func<long> clock;
    readonly HashSet<string> down = new HashSet<string>();
    long lastMove = long.MinValue / 2;
    long nextMove;
    long holdStart;
    long lastKey = long.MinValue / 2;
    long pressedAt;
    string? held;
    /// <summary>The direction pressed and not let go yet: held once <see cref="HoldDelayMs"/> has passed.</summary>
    string? pressed;

    public KeyRepeatGate(Func<long>? clock = null) => this.clock = clock ?? (() => System.Diagnostics.Stopwatch.GetTimestamp() * 1000 / System.Diagnostics.Stopwatch.Frequency);

    /// <summary>Whether a direction key is held down and moving focus by itself now.</summary>
    public bool Holding { get; private set; }

    /// <summary>Whether a direction key is down: <see cref="Tick"/> is worth calling.</summary>
    public bool Down => pressed != null || Holding;

    /// <summary>The time between moves now: 80 ms, less the longer the key is held.</summary>
    public long Interval { get; private set; } = MoveIntervalMs;

    /// <summary>How long a held move's glide lasts: a little over <see cref="Interval"/>, so the next starts before it ends.</summary>
    public int GlideMs => (int)(Interval * 5 / 4);

    /// <summary>
    /// Whether a key event goes through: true lets it act. A press of a direction does; its repeats
    /// don't, as <see cref="Tick"/> moves focus while it is held. Only direction keys are gated.
    /// </summary>
    public bool Admit(string key, bool isDown)
    {
        if (!Directions.Contains(key)) return true;
        var now = clock();
        if (isDown)
        {
            lastKey = now;
            if (down.Add(key))
            {
                Holding = false;
                held = null;
                pressed = key;
                pressedAt = now;
                lastMove = now;
                Interval = MoveIntervalMs;
                return true;
            }
            if (!Holding || held != key) Hold(key, now);
            return false;
        }
        down.Remove(key);
        if (pressed == key) pressed = null;
        Holding = false;
        held = null;
        Interval = MoveIntervalMs;
        lastMove = now;
        return true;
    }

    /// <summary>
    /// Called every frame or so while a key is down: the direction to move focus now, or null.
    /// Moves come <see cref="Interval"/> apart on average, whenever the frames fall.
    /// </summary>
    public string? Tick()
    {
        ExpireHold();
        var now = clock();
        // Down long enough: the hold begins, before the TV's own repeats would.
        if (!Holding && pressed != null && down.Contains(pressed) && now - pressedAt >= HoldDelayMs) Hold(pressed, now);
        if (!Holding || held == null || now < nextMove) return null;
        Interval = IntervalAt(now - holdStart);
        // Kept to the pace: a late frame doesn't push the next move later, nor does a stall bring a burst.
        nextMove = Math.Max(nextMove + Interval, now + Interval / 2);
        lastMove = now;
        return held;
    }

    void Hold(string key, long now)
    {
        Holding = true;
        held = key;
        holdStart = now;
        Interval = MoveIntervalMs;
        // The first held move is due at once.
        nextMove = now;
    }

    /// <summary>The time between moves <paramref name="heldMs"/> into a hold: 80 ms easing to 25 over 0.7 s.</summary>
    public static long IntervalAt(long heldMs)
    {
        var t = Math.Min(1.0, Math.Max(0.0, heldMs / (double)RampMs));
        var eased = t * t * (3 - 2 * t);
        return (long)Math.Round(MoveIntervalMs - (MoveIntervalMs - FastestIntervalMs) * eased);
    }

    /// <summary>Whether focus has rested: no key held, and nothing moved for <see cref="RestMs"/>.</summary>
    public bool Rested()
    {
        ExpireHold();
        return !Holding && clock() - lastMove >= RestMs;
    }

    /// <summary>Milliseconds until focus has rested: 0 once it has; while a key is held, a short poll.</summary>
    public long UntilRest()
    {
        ExpireHold();
        if (Holding) return 50;
        return Math.Max(0, lastMove + RestMs - clock());
    }

    /// <summary>The app lost the key-up (focus left it mid-hold): the hold is over.</summary>
    void ExpireHold()
    {
        if (Down && clock() - lastKey > LostKeyUpMs)
        {
            Holding = false;
            held = null;
            pressed = null;
            down.Clear();
            lastMove = lastKey;
        }
    }
}
