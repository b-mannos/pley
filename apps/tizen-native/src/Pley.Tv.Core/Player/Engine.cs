using System;

namespace Pley.Tv.Core.Player;

/// <summary>Where the TV's player is with what it was given.</summary>
public enum EngineStatus
{
    Idle,
    /// <summary>Waiting for data.</summary>
    Buffering,
    /// <summary>It has data and plays, or would if it weren't paused.</summary>
    Ready,
    /// <summary>It played to the end.</summary>
    Ended,
}

/// <summary>What the TV's player is given: an absolute address (keyed: it carries its own key), and whether it is a stream.</summary>
/// <param name="Live">A live channel's HLS: it starts at the live edge, whatever <paramref name="StartMs"/> says.</param>
/// <param name="DurationHintMs">How long the server says it is, for a player that knows only once it opened.</param>
public sealed record EngineSource(string Url, bool Hls, bool Live = false, long StartMs = 0, long? DurationHintMs = null);

/// <summary>What went wrong in the TV's player, in the terms the controllers decide on.</summary>
public abstract record EngineError(string Detail)
{
    /// <summary>It can't decode or open this.</summary>
    public sealed record Unsupported(string Detail) : EngineError(Detail);

    /// <summary>The server answered a playlist, segment or file with <paramref name="Status"/>.</summary>
    public sealed record Http(int Status, string Detail) : EngineError(Detail);

    /// <summary>410: stopped from elsewhere, with what the server said.</summary>
    public sealed record Stopped(string? Message) : EngineError(Message ?? "");

    /// <summary>The network failed under it.</summary>
    public sealed record Network(string Detail) : EngineError(Detail);

    public sealed record Other(string Detail) : EngineError(Detail);
}

/// <summary>
/// The TV's own player as Live TV and music drive it (packages/tv's PlayerEngine): the Tizen app
/// wraps Tizen.Multimedia.Player in it, tests a fake. Its events come on the UI thread, after the
/// call that caused them has returned.
/// </summary>
public interface IMediaEngine
{
    event Action<EngineStatus>? StatusChanged;

    /// <summary>The first picture (or, with sound only, the first sound) of a source is out.</summary>
    event Action? Started;

    /// <summary>It plays, or stopped playing (paused, waiting, ended).</summary>
    event Action<bool>? PlayingChanged;

    event Action<EngineError>? Failed;

    /// <summary>Opens <paramref name="source"/> and starts at its start; plays at once when <paramref name="play"/>.</summary>
    void Prepare(EngineSource source, bool play);

    void Play();

    void Pause();

    void SeekTo(long ms);

    /// <summary>Stops and closes the source; another can be prepared.</summary>
    void Stop();

    /// <summary>Gone for good.</summary>
    void Release();

    long PositionMs { get; }

    /// <summary>0 while unknown.</summary>
    long DurationMs { get; }

    /// <summary>How far it has data; the position when the player doesn't say.</summary>
    long BufferedMs { get; }

    bool Playing { get; }

    /// <summary>Whether what plays has a picture; null until known.</summary>
    bool? HasVideo { get; }
}
