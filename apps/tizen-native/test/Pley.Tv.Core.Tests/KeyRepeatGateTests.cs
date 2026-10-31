using Pley.Tv.Core.Ui;
using Xunit;

namespace Pley.Tv.Core.Tests;

/// <summary>D-pad rule 5: held keys move focus at most every 100 ms, and focus rests after 400 ms.</summary>
public class KeyRepeatGateTests
{
    [Fact]
    public void Lets_one_move_through_every_100_ms_while_a_key_repeats()
    {
        long now = 1000;
        var gate = new KeyRepeatGate(() => now);
        Assert.True(gate.Admit("Right", true));
        Assert.False(gate.Holding);
        var admitted = 0;
        for (var i = 0; i < 10; i++)
        {
            now += 50;
            if (gate.Admit("Right", true)) admitted++;
        }
        Assert.True(gate.Holding);
        Assert.Equal(5, admitted);
        Assert.True(gate.Admit("Right", false));
        Assert.False(gate.Holding);
        Assert.True(gate.Admit("Return", true));
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
        Assert.Equal(0, gate.UntilRest());
        gate.Admit("Down", true);
        Assert.False(gate.Holding);
    }
}
