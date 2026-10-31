using System.Collections.Generic;
using System.Linq;

namespace Pley.Tv.Core.Player;

/// <summary>
/// What a Samsung TV says of itself that decides what it plays (apps/tizen/src/facts.ts): Samsung's
/// own calls answer only per codec, so the report comes from the model year's specification (the
/// Tizen version tells the year), refined by these.
/// </summary>
public sealed record TizenFacts
{
    public double Tizen { get; init; } = 9;
    public int Year { get; init; } = 2025;
    public string Model { get; init; } = "Samsung TV";
    public bool Uhd { get; init; } = true;
    public bool Uhd8k { get; init; }
    public bool Hdr { get; init; } = true;
    /// <summary>Whether it decodes AV1, when the TV says; null: by the year.</summary>
    public bool? Av1 { get; init; }
    public bool? Ac4 { get; init; }
    /// <summary>The sound output: "PCM", "DOLBY", "DOLBY_DIGITAL_PLUS", "DTS"; null when unknown (nothing passed through).</summary>
    public string? OutputMode { get; init; }

    static readonly (double Version, int Year)[] Years =
    {
        (10, 2026), (9, 2025), (8, 2024), (7, 2023), (6.5, 2022), (6, 2021), (5.5, 2020), (5, 2019), (4, 2018), (3, 2017), (2.4, 2016), (2.3, 2015),
    };

    /// <summary>The model year of a Tizen version (each year ships one).</summary>
    public static int YearOf(double tizen)
    {
        foreach (var (version, year) in Years)
            if (tizen >= version) return year;
        return 2015;
    }
}

/// <summary>
/// What a Samsung TV plays, reported to the server (capabilities version 2) so it sends files as
/// they are whenever the TV's player takes them: the web app's tizenCaps (apps/tizen/src/caps.ts),
/// the same report for the same TV. Samsung TVs show no Dolby Vision. As JSON for the play request.
/// </summary>
public static class TizenCaps
{
    static readonly string[] Containers = { "mkv", "mp4", "m4v", "mov", "ts", "m2ts", "mpegts", "webm", "avi", "mpg", "mpeg", "mp3", "m4a", "aac", "flac", "ogg", "oga", "wav", "mka" };

    /// <summary>Subtitles the app draws itself: text as WebVTT from the server, pictures from their .mks.</summary>
    static readonly string[] Subtitles = { "subrip", "ass", "ssa", "webvtt", "mov_text", "hdmv_pgs_subtitle", "dvd_subtitle", "dvb_subtitle" };

    // ffprobe's levels: H.264 4.2 = 42, 5.1 = 51; HEVC 4.1 = 123, 5.1 = 153, 5.2 = 156, 6.1 = 183.

    public static Dictionary<string, object?> Of(TizenFacts f)
    {
        var y = f.Year;
        var video = new List<Dictionary<string, object?>>();
        var h264 = new[] { "Constrained Baseline", "Baseline", "Main", "High" };
        if (f.Uhd) video.Add(Decoder("h264", h264, 3840, 2160, 60, 8, 51));
        video.Add(Decoder("h264", h264, 1920, 1080, y >= 2021 ? 120 : 60, 8, 42));
        var hevc = new[] { "Main", "Main 10" };
        if (f.Uhd8k && y >= 2019) video.Add(Decoder("hevc", hevc, 7680, 4320, 60, 10, 183));
        if (f.Uhd) video.Add(Decoder("hevc", hevc, 4096, 2160, 60, 10, y >= 2025 ? 156 : 153));
        else video.Add(Decoder("hevc", hevc, 1920, 1080, 60, 10, 123));
        var (bigW, bigH) = f.Uhd ? (3840, 2160) : (1920, 1080);
        video.Add(Decoder("vp9", y >= 2018 ? new[] { "Profile 0", "Profile 2" } : new[] { "Profile 0" }, bigW, bigH, 60, y >= 2018 ? 10 : 8));
        // AV1 from 2020's chips.
        if ((f.Av1 ?? y >= 2020) && y >= 2020)
            video.Add(f.Uhd8k ? Decoder("av1", new[] { "Main" }, 7680, 4320, 60, 10) : Decoder("av1", new[] { "Main" }, bigW, bigH, 60, 10));
        video.Add(Decoder("mpeg2video", null, 1920, 1080, 60, 8));
        video.Add(Decoder("mpeg4", null, 1920, 1080, 60, 8));

        var hdrTypes = new List<string>();
        if (f.Hdr)
        {
            hdrTypes.Add("hdr10");
            if (y >= 2017) hdrTypes.Add("hdr10plus");
            if (y >= 2018) hdrTypes.Add("hlg");
        }

        // Sound: what the TV decodes, what goes to a receiver as it is.
        var decode = new List<string> { "aac", "ac3", "eac3", "mp3", "mp2", "flac", "vorbis", "pcm_s16le", "pcm_s24le" };
        if (y >= 2018) decode.Add("opus");
        if (y >= 2018 && f.Ac4 != false) decode.Add("ac4");
        // DTS until 2017: Samsung dropped it from 2018 sets on.
        var dtsDecoded = y <= 2017;
        if (dtsDecoded) decode.Add("dts");
        var mode = (f.OutputMode ?? "").ToUpperInvariant();
        var passthrough = new List<string>();
        if (mode == "DOLBY_DIGITAL_PLUS") passthrough.AddRange(new[] { "ac3", "eac3" });
        else if (mode == "DOLBY") passthrough.Add("ac3");
        else if (mode == "DTS" && dtsDecoded) passthrough.Add("dts");

        var display = f.Uhd8k ? (7680, 4320) : f.Uhd ? (3840, 2160) : (1920, 1080);
        return new Dictionary<string, object?>
        {
            ["containers"] = Containers,
            ["video_codecs"] = video.Select(v => (string)v["codec"]!).Distinct().ToArray(),
            ["audio_codecs"] = decode.Concat(passthrough).Distinct().ToArray(),
            ["hdr"] = hdrTypes.Count > 0,
            ["dolby_vision"] = false,
            ["caps_version"] = 2,
            ["video"] = video,
            ["hdr_types"] = hdrTypes,
            ["dolby_vision_profiles"] = new object[0],
            ["audio_decode"] = decode,
            ["audio_passthrough"] = passthrough.Distinct().ToArray(),
            ["audio_channels"] = passthrough.Count > 0 ? 8 : 6,
            ["subtitle_codecs"] = Subtitles,
            ["audio_track_switching"] = true,
            ["picture_subtitles_sideload"] = true,
            // One size per stream (no variants): the TV's player may go black moving between sizes (docs/QUALITY.md).
            ["hls"] = new Dictionary<string, object?> { ["master_playlist"] = true, ["segments"] = y >= 2020 ? new[] { "ts", "fmp4" } : new[] { "ts" } },
            ["display"] = new Dictionary<string, object?> { ["width"] = display.Item1, ["height"] = display.Item2, ["refresh_rates"] = new[] { 50, 60 } },
            ["device"] = new Dictionary<string, object?> { ["form"] = "tv", ["tier"] = Tier(f), ["fire_tv"] = false, ["model"] = f.Model },
        };
    }

    /// <summary>Low for 2018 sets, mid until 2021, high after.</summary>
    public static string Tier(TizenFacts f) => f.Year <= 2018 ? "low" : f.Year <= 2021 ? "mid" : "high";

    static Dictionary<string, object?> Decoder(string codec, string[]? profiles, int w, int h, int fps, int depth, int? level = null)
    {
        var d = new Dictionary<string, object?> { ["codec"] = codec, ["hardware"] = true };
        if (profiles != null) d["profiles"] = profiles;
        d["max_width"] = w;
        d["max_height"] = h;
        d["max_fps"] = fps;
        d["max_bit_depth"] = depth;
        if (level != null) d["max_level"] = level;
        return d;
    }
}
