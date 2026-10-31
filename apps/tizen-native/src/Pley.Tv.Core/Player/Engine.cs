using System.Collections.Generic;

namespace Pley.Tv.Core.Player;

/// <summary>Where the engine stands with its source.</summary>
public enum EngineStatus
{
    Idle,
    Buffering,
    Ready,
    Ended,
}

/// <summary>The video as it will arrive, so a TV can match its display mode before the first frame; null fields aren't known.</summary>
/// <param name="Range">"sdr", "hdr10", "hdr10plus", "hlg" or "dolby_vision": a conversion's is the server's to choose, so "sdr".</param>
public sealed record EngineVideo(string Codec, int? Width, int? Height, double? FrameRate, string Range, int? DvProfile, int BitDepth);

/// <summary>What the engine plays, and what is known of it before it opens (the web TV app's EngineSource).</summary>
/// <param name="Url">Absolute; carries the session's key, as the TV's player can't send headers.</param>
/// <param name="Mime">The container's MIME type for a file played as it is; null for HLS.</param>
/// <param name="AudioCodec">The audio stream's codec when played as it is (passed through or decoded); null when the server converts it.</param>
public sealed record EngineSource(string Url, bool Hls, string? Mime, long StartMs, long? DurationHintMs, EngineVideo? Video, string? AudioCodec);

/// <summary>An audio track as the engine knows it, in the file's order.</summary>
public sealed record EngineTrack(string? Language, string? Codec, int? Channels);

/// <summary>
/// What the engine can say about what it plays, for the Info panel's "This TV" (the web TV app's
/// EngineStats). Every field is optional: Tizen's player says little.
/// </summary>
public sealed record EngineStats
{
    /// <summary>The video decoder's name.</summary>
    public string? Decoder { get; init; }
    public bool? Hardware { get; init; }
    public int? Width { get; init; }
    public int? Height { get; init; }
    public double? FrameRate { get; init; }
    public int? BitDepth { get; init; }
    /// <summary>The codec arriving, as a MIME type or a codec string: "video/hevc", "hvc1.2.4.L153.B0".</summary>
    public string? Codec { get; init; }
    /// <summary>"HDR10", "Dolby Vision", "HLG", "SDR"… as the engine reports it.</summary>
    public string? Hdr { get; init; }
    /// <summary>Bits per second of the stream as it arrives.</summary>
    public long? Bitrate { get; init; }
    /// <summary>Bits per second the network brings.</summary>
    public long? Bandwidth { get; init; }
    public string? AudioCodec { get; init; }
    public int? AudioChannels { get; init; }
    /// <summary>"Passed through as E-AC-3", "Decoded to PCM"…</summary>
    public string? AudioOutput { get; init; }
    /// <summary>Frames dropped so far, of <see cref="Frames"/>.</summary>
    public long? Dropped { get; init; }
    /// <summary>Every frame the decoder had so far, dropped ones included.</summary>
    public long? Frames { get; init; }
    /// <summary>Lines the engine adds as they are ("Label: value"), when it has more to say.</summary>
    public IReadOnlyList<string> Extra { get; init; } = new string[0];
}

/// <summary>
/// What the engine tells the player. The engine calls these on the UI thread: Tizen's player
/// raises its events on its own threads, so the engine posts them over first.
/// </summary>
public interface IEngineListener
{
    void Status(EngineStatus status);

    /// <summary>The first picture of a source shows.</summary>
    void FirstFrame();

    void Error(EngineError error);
}

/// <summary>
/// The TV's own player, as the session drives it (the web TV app's PlayerEngine): on Samsung TVs,
/// Tizen.Multimedia.Player on the video plane. The Tizen app implements it; everything above it
/// (the server session, recovery, subtitles, Up Next) is Core's, tested with a fake. Every call
/// comes on the UI thread.
/// </summary>
public interface IPlayerEngine
{
    /// <summary>"Tizen player".</summary>
    string Name { get; }

    void SetListener(IEngineListener? listener);

    /// <summary>Opens <paramref name="source"/> at its <see cref="EngineSource.StartMs"/> and plays at once when <paramref name="play"/>; a source playing before is let go.</summary>
    void Prepare(EngineSource source, bool play);

    void Play();

    void Pause();

    void SeekTo(long ms);

    /// <summary>Stops and closes the source; the engine can prepare another.</summary>
    void Stop();

    /// <summary>Gone for good: the player screen closes.</summary>
    void Release();

    long PositionMs { get; }

    /// <summary>0 while unknown.</summary>
    long DurationMs { get; }

    /// <summary>How far it has loaded, in ms of the media; the position when it can't say (<see cref="KnowsBuffer"/>).</summary>
    long BufferedMs { get; }

    /// <summary>False when <see cref="BufferedMs"/> only repeats the position.</summary>
    bool KnowsBuffer { get; }

    EngineStatus Status { get; }

    /// <summary>Meant to play: started and not paused.</summary>
    bool Playing { get; }

    IReadOnlyList<EngineTrack> AudioTracks();

    /// <summary>Switches to the audio track at <paramref name="ordinal"/> among the file's audio streams, checked against <paramref name="language"/>; false when it can't.</summary>
    bool SelectAudio(int ordinal, string? language);

    /// <summary>Plays at these speeds with their sound; empty when only at normal speed (Tizen's player fast-forwards silently).</summary>
    IReadOnlyList<double> Speeds { get; }

    /// <summary>Plays at <paramref name="rate"/> times normal speed, from now and for the sources it opens after; false when it can't.</summary>
    bool SetSpeed(double rate);

    /// <summary>Dropped and shown frames so far; null when the engine doesn't count them.</summary>
    FrameCounts? FrameCounts();

    /// <summary>How fast the network brings the stream, kbps; null when it can't say.</summary>
    long? BandwidthKbps();

    EngineStats? Stats();
}
