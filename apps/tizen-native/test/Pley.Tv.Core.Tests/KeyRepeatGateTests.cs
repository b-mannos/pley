using Pley.Tv.Core.Ui;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>D-pad rule 5: a press moves once; held 250 ms, focus moves on the app's clock, faster the longer it is held; focus rests after 400 ms.</summary>
public class KeyRepeatGateTests
{
    [Fact]
    public void A_press_moves_once_and_held_250_ms_focus_moves_by_itself()
    {
        long now = 1000;
        var gate = new KeyRepeatGate(() => now);
        Assert.True(gate.Admit("Right", true));
        Assert.True(gate.Down);
        Assert.False(gate.Holding);
        now += 200;
        Assert.Null(gate.Tick());
        // Before the TV's repeats (400 ms), the hold begins, and its first move is due at once.
        now += 50;
        Assert.Equal("Right", gate.Tick());
        Assert.True(gate.Holding);
        now += 40;
        Assert.Null(gate.Tick());
        // The TV's repeats only say the key is still down.
        Assert.False(gate.Admit("Right", true));
        now += 40;
        Assert.Equal("Right", gate.Tick());
        Assert.True(gate.Admit("Right", false));
        Assert.False(gate.Down);
        Assert.Null(gate.Tick());
        Assert.True(gate.Admit("Return", true));
    }

    [Fact]
    public void A_quick_press_never_starts_a_hold()
    {
        long now = 1000;
        var gate = new KeyRepeatGate(() => now);
        gate.Admit("Left", true);
        now += 120;
        gate.Admit("Left", false);
        now += 500;
        Assert.Null(gate.Tick());
        Assert.False(gate.Holding);
    }

    [Fact]
    public void Held_longer_it_speeds_up_smoothly_to_40_moves_a_second()
    {
        Assert.Equal(KeyRepeatGate.MoveIntervalMs, KeyRepeatGate.IntervalAt(0));
        Assert.Equal(KeyRepeatGate.FastestIntervalMs, KeyRepeatGate.IntervalAt(KeyRepeatGate.RampMs));
        Assert.Equal(KeyRepeatGate.FastestIntervalMs, KeyRepeatGate.IntervalAt(10_000));
        // No step anywhere along the way bigger than a few milliseconds.
        var before = KeyRepeatGate.IntervalAt(0);
        for (var ms = 16L; ms <= KeyRepeatGate.RampMs; ms += 16)
        {
            var next = KeyRepeatGate.IntervalAt(ms);
            Assert.True(next <= before && before - next <= 4, $"{before} to {next} at {ms} ms");
            before = next;
        }
    }

    [Fact]
    public void Moves_keep_their_pace_whenever_the_frames_fall()
    {
        long now = 1000;
        var gate = new KeyRepeatGate(() => now);
        gate.Admit("Down", true);
        var moves = 0;
        // Two seconds of frames 17 ms apart, the TV repeating every 50 ms from 400 ms.
        for (var t = 0; t < 2000; t += 17)
        {
            now = 1000 + t;
            if (t >= 400 && t % 50 < 17) gate.Admit("Down", true);
            if (gate.Tick() != null) moves++;
        }
        // Nothing for 250 ms, then 12.5 a second easing to 40 within 0.7 s, then 40: about 56.
        Assert.InRange(moves, 48, 64);
        Assert.Equal(KeyRepeatGate.FastestIntervalMs, gate.Interval);
        Assert.True(gate.GlideMs > gate.Interval);
    }

    [Fact]
    public void Rests_400_ms_after_the_last_move_and_ends_a_hold_whose_key_up_was_lost()
    {
        long now = 1000;
        var gate = new KeyRepeatGate(() => now);
        gate.Admit("Down", true);
        now += 50;
        gate.Admit("Down", true);
        Assert.True(gate.Holding);
        Assert.False(gate.Rested());
        now += 600;
        // No repeat for 500 ms: the key-up was lost, and focus has rested since the last key.
        Assert.True(gate.Rested());
        Assert.False(gate.Holding);
        Assert.False(gate.Down);
        Assert.Null(gate.Tick());
        Assert.Equal(0, gate.UntilRest());
        gate.Admit("Down", true);
        Assert.False(gate.Holding);
    }
}
