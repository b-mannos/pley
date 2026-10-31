using System;
using System.Collections.Generic;

namespace Pley.Tv.Core.Ui;

/// <summary>
/// D-pad rule 5, held keys (the Android app's KeyRepeatGate). A held key repeats after 400 ms and
/// then every 50 ms; moving focus that fast makes TVs drop frames and starts a picture load for
/// every tile passed. So while a key repeats: at most one focus move goes through every 100 ms,
/// rows scroll without animation and tiles snap to their focus look (<see cref="Holding"/>), and
/// backdrops and details wait until focus has rested 400 ms (<see cref="Rested"/>).
/// NUI's key events carry no repeat count: a key that goes down again before coming up repeats.
/// </summary>
public sealed class KeyRepeatGate
{
    /// <summary>At most one move this often while a key repeats (the TV repeats every 50 ms).</summary>
    public const long MoveIntervalMs = 100;
    /// <summary>How long focus rests before backdrops and details follow it.</summary>
    public const long RestMs = 400;
    /// <summary>Repeats come every 50 ms; this long without one means the key is up.</summary>
    const long LostKeyUpMs = 500;

    static readonly HashSet<string> Directions = new HashSet<string> { "Up", "Down", "Left", "Right" };

    readonly Func<long> clock;
    readonly HashSet<string> down = new HashSet<string>();
    long lastMove = long.MinValue / 2;
    long lastKey = long.MinValue / 2;

    public KeyRepeatGate(Func<long>? clock = null) => this.clock = clock ?? (() => System.Diagnostics.Stopwatch.GetTimestamp() * 1000 / System.Diagnostics.Stopwatch.Frequency);

    /// <summary>Whether a direction key is held down and repeating now.</summary>
    public bool Holding { get; private set; }

    /// <summary>Whether a key event goes through: true lets it move focus, false drops it. Only direction keys are gated.</summary>
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
                lastMove = now;
                return true;
            }
            Holding = true;
            if (now - lastMove < MoveIntervalMs) return false;
            lastMove = now;
            return true;
        }
        down.Remove(key);
        Holding = false;
        lastMove = now;
        return true;
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
        if (Holding && clock() - lastKey > LostKeyUpMs)
        {
            Holding = false;
            down.Clear();
            lastMove = lastKey;
        }
    }
}
