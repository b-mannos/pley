using System.Collections.Generic;
using System.Text;
using System.Text.Json;

namespace Pley.Tv.Core.Api;

// The parts of Pley's API the browsing screens use (docs/API.md), as the web TV app's
// api/types.ts has them. Kinds and sorts stay strings, as the server writes them, so a kind a
// newer server adds doesn't break an older app. Fields a server leaves out read as null.

public sealed record User
{
    public long Id { get; init; }
    public string Username { get; init; } = "";
    public string Name { get; init; } = "";
    public string? DisplayName { get; init; }
    public bool? IsAdmin { get; init; }
    /// <summary>A profile without a password, reached by switching.</summary>
    public bool? Managed { get; init; }
    public string? AvatarUrl { get; init; }
    public string? AvatarColor { get; init; }
    public string? AudioLanguage { get; init; }
    public string? SubtitleLanguage { get; init; }
    /// <summary>"off", "forced", "foreign" or "always".</summary>
    public string SubtitleMode { get; init; } = "off";
    /// <summary>An app's token: what it was allowed ("library play").</summary>
    public string? Scope { get; init; }
}

public sealed record Library
{
    public long Id { get; init; }
    public string Name { get; init; } = "";
    /// <summary>"movies", "shows" or "music".</summary>
    public string Kind { get; init; } = "";
    public long ItemCount { get; init; }
    public bool Scanning { get; init; }
}

public sealed record UserState
{
    public long PositionMs { get; init; }
    public bool Watched { get; init; }
    public long PlayCount { get; init; }
    public long? LastPlayedAt { get; init; }
    public double? Rating { get; init; }
}

public sealed record MediaSummary
{
    public long? Bitrate { get; init; }
    public long? Size { get; init; }
    public int? Width { get; init; }
    public int? Height { get; init; }
    public string? VideoCodec { get; init; }
    /// <summary>"sdr", "hdr10", "hdr10plus", "hlg" or "dolby_vision".</summary>
    public string? DynamicRange { get; init; }
}

/// <summary>A title in a list. Kinds: "movie", "show", "season", "episode", "artist", "album", "track", "extra".</summary>
public record ItemSummary
{
    public long Id { get; init; }
    public long LibraryId { get; init; }
    public string Kind { get; init; } = "";
    public string Title { get; init; } = "";
    public string SortTitle { get; init; } = "";
    public int? Year { get; init; }
    public int? Index { get; init; }
    public long? ParentId { get; init; }
    public string? ParentTitle { get; init; }
    public int? ParentIndex { get; init; }
    public long? GrandparentId { get; init; }
    public string? GrandparentTitle { get; init; }
    public long? DurationMs { get; init; }
    public double? Rating { get; init; }
    /// <summary>Unix seconds.</summary>
    public long AddedAt { get; init; }
    public string? PosterUrl { get; init; }
    public string? BackdropUrl { get; init; }
    public string? ThumbUrl { get; init; }
    public UserState UserState { get; init; } = new UserState();
    public int? LeafCount { get; init; }
    public int? ViewedLeafCount { get; init; }
    public int? ChildCount { get; init; }
    public string? Summary { get; init; }
    public string? ReleaseDate { get; init; }
    public string? ExtraType { get; init; }
    public MediaSummary? Media { get; init; }
}

public sealed record ItemDetail : ItemSummary
{
    public string? OriginalTitle { get; init; }
    public string? Tagline { get; init; }
    public IReadOnlyList<string> Genres { get; init; } = new string[0];
    public string? Studio { get; init; }
    public string? ContentRating { get; init; }
    public IReadOnlyList<MediaFile> Files { get; init; } = new MediaFile[0];
    public IReadOnlyList<Marker>? Markers { get; init; }
    public IReadOnlyList<ItemSummary>? Extras { get; init; }
    public IReadOnlyList<Credit>? Cast { get; init; }
    public IReadOnlyList<Credit>? Crew { get; init; }
    public bool? InWatchlist { get; init; }
}

/// <summary>Roles: "actor", "creator", "director", "writer", "producer".</summary>
public sealed record Credit
{
    public long Id { get; init; }
    public string Name { get; init; } = "";
    public string Role { get; init; } = "";
    public string? Character { get; init; }
    public string? PhotoUrl { get; init; }
}

public record PersonSummary
{
    public long Id { get; init; }
    public string Name { get; init; } = "";
    public string? PhotoUrl { get; init; }
}

public sealed record PersonRole
{
    public string Role { get; init; } = "";
    public string? Character { get; init; }
    public int? Episodes { get; init; }
}

public sealed record PersonCredit
{
    public ItemSummary Item { get; init; } = new ItemSummary();
    public IReadOnlyList<PersonRole> Roles { get; init; } = new PersonRole[0];
}

public sealed record PersonDetail : PersonSummary
{
    public IReadOnlyList<PersonCredit> Credits { get; init; } = new PersonCredit[0];
}

public sealed record Marker
{
    public long FileId { get; init; }
    /// <summary>"intro" or "credits".</summary>
    public string Kind { get; init; } = "";
    public long StartMs { get; init; }
    public long EndMs { get; init; }
    /// <summary>"chapter", "fingerprint" or "frames".</summary>
    public string Source { get; init; } = "";
}

public sealed record Chapter
{
    public long StartMs { get; init; }
    public long EndMs { get; init; }
    public string? Title { get; init; }
    public string? ThumbUrl { get; init; }
}

public sealed record MediaFile
{
    public long Id { get; init; }
    public string Path { get; init; } = "";
    public long Size { get; init; }
    public string? Container { get; init; }
    public long? DurationMs { get; init; }
    public long? Bitrate { get; init; }
    public int? Width { get; init; }
    public int? Height { get; init; }
    public string? VideoCodec { get; init; }
    public string? AudioCodec { get; init; }
    public IReadOnlyList<MediaStream> Streams { get; init; } = new MediaStream[0];
    public string? Edition { get; init; }
    public int? SelectedAudio { get; init; }
    public int? SelectedSubtitle { get; init; }
    /// <summary>
    /// <see cref="SelectedAudio"/> is a track the profile picked. False: nothing is picked, and each
    /// player gets its best track of that language at play time, so a title's page names the
    /// language. Absent from older servers.
    /// </summary>
    public bool? AudioPicked { get; init; }
    public IReadOnlyList<Chapter> Chapters { get; init; } = new Chapter[0];
}

/// <summary>A file's stream. Kinds: "video", "audio", "subtitle".</summary>
public sealed record MediaStream
{
    public int Index { get; init; }
    public string Kind { get; init; } = "";
    public string Codec { get; init; } = "";
    public string? Language { get; init; }
    public string? Title { get; init; }
    public bool Default { get; init; }
    public bool Forced { get; init; }
    public long? Bitrate { get; init; }
    public int? Width { get; init; }
    public int? Height { get; init; }
    public double? FrameRate { get; init; }
    public int? BitDepth { get; init; }
    public bool? Hdr { get; init; }
    public string? DynamicRange { get; init; }
    public int? DvProfile { get; init; }
    public int? DvBlCompat { get; init; }
    public int? DvLevel { get; init; }
    /// <summary>Profile 7's enhancement layer: "fel" or "mel" (absent: not known).</summary>
    public string? DvEl { get; init; }
    /// <summary>HDR10+ in the first frames (Dolby Vision and PQ streams; absent: not looked for).</summary>
    public bool? Hdr10plus { get; init; }
    /// <summary>Numbered as ffprobe numbers it (HEVC 5.1 = 153, H.264 5.1 = 51).</summary>
    public int? Level { get; init; }
    public string? Profile { get; init; }
    public string? PixFmt { get; init; }
    public bool? DecodeOrderTimes { get; init; }
    public int? Channels { get; init; }
    public string? ChannelLayout { get; init; }
    public int? SampleRate { get; init; }
    public bool? External { get; init; }
    public bool? TextBased { get; init; }
    /// <summary>Added on the server (an uploaded file, or one found on OpenSubtitles).</summary>
    public bool? Added { get; init; }
    public bool? Deletable { get; init; }
}

public sealed record Hub
{
    public string Key { get; init; } = "";
    public string Title { get; init; } = "";
    public long? LibraryId { get; init; }
    public IReadOnlyList<ItemSummary> Items { get; init; } = new ItemSummary[0];
}

public record Collection
{
    public long Id { get; init; }
    public long LibraryId { get; init; }
    public string Title { get; init; } = "";
    public string? Summary { get; init; }
    public long ItemCount { get; init; }
    public string PosterUrl { get; init; } = "";
    public string BackdropUrl { get; init; } = "";
}

public record Playlist
{
    public long Id { get; init; }
    public string Title { get; init; } = "";
    /// <summary>"video" or "audio".</summary>
    public string Kind { get; init; } = "";
    public long ItemCount { get; init; }
    public long DurationMs { get; init; }
    public IReadOnlyList<string> Artwork { get; init; } = new string[0];
    public string? SmartFilter { get; init; }
}

public sealed record PlaylistEntry
{
    public long EntryId { get; init; }
    public ItemSummary Item { get; init; } = new ItemSummary();
}

public sealed record HistoryEntry
{
    public long Id { get; init; }
    public long ViewedAt { get; init; }
    public string? Device { get; init; }
    public ItemSummary Item { get; init; } = new ItemSummary();
}

public sealed record HouseholdProfile
{
    public long Id { get; init; }
    public string Name { get; init; } = "";
    public string? AvatarUrl { get; init; }
    public string? AvatarColor { get; init; }
    public bool Current { get; init; }
    /// <summary>"none", "pin", "password" or "locked".</summary>
    public string Needs { get; init; } = "none";
}

/// <summary>A library's sorts, as the server names them.</summary>
public static class LibrarySort
{
    public const string Title = "title";
    public const string Year = "year";
    public const string Added = "added";
    public const string Rating = "rating";
    public const string UserRating = "user_rating";
    public const string Release = "release";
    public const string Duration = "duration";
    public const string LastPlayed = "last_played";
    public const string Bitrate = "bitrate";
}

/// <summary>Reading and writing the API's JSON: snake_case names, as the server writes them.</summary>
public static class ApiJson
{
    public static readonly JsonSerializerOptions Options = new JsonSerializerOptions
    {
        PropertyNamingPolicy = new SnakeCase(),
    };

    /// <summary>"DisplayName" as "display_name", "Hdr10plus" as "hdr10plus". .NET 6 has no policy of its own for this.</summary>
    sealed class SnakeCase : JsonNamingPolicy
    {
        public override string ConvertName(string name)
        {
            var b = new StringBuilder(name.Length + 4);
            for (var i = 0; i < name.Length; i++)
            {
                var c = name[i];
                if (char.IsUpper(c))
                {
                    if (i > 0) b.Append('_');
                    b.Append(char.ToLowerInvariant(c));
                }
                else b.Append(c);
            }
            return b.ToString();
        }
    }
}
