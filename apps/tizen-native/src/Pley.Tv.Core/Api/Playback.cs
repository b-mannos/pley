using System.Collections.Generic;

namespace Pley.Tv.Core.Api;

/// <summary>A session the server made for a play request (POST /api/playback; docs/API.md).</summary>
public sealed record PlaybackInfo
{
    public string SessionId { get; init; } = "";
    /// <summary>"direct", "remux" or "transcode".</summary>
    public string Method { get; init; } = "";
    /// <summary>Where the player reads it; with keyed URLs, an address that carries its own key.</summary>
    public string Url { get; init; } = "";
    public bool Hls { get; init; }
    public long FileId { get; init; }
    public long DurationMs { get; init; }
    public int? AudioIndex { get; init; }
    public int? SubtitleIndex { get; init; }
    public bool BurnSubtitles { get; init; }
    public IReadOnlyList<string>? Reasons { get; init; }
    public string? Reason { get; init; }
    /// <summary>What the screen gets of HDR video, by name: "HDR10", "SDR (tone-mapped)"…</summary>
    public string? VideoRange { get; init; }
}
