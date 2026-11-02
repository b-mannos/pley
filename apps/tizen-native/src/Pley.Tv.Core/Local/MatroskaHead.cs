using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using Pley.Tv.Core.Api;

namespace Pley.Tv.Core.Local;

/// <summary>What a Matroska file says of itself before its first cluster: its length, tracks and chapters.</summary>
/// <param name="Streams">As ffprobe would list them, in the file's order, numbered from 0.</param>
public sealed record LocalFacts(long? DurationMs, IReadOnlyList<MediaStream> Streams, IReadOnlyList<Chapter> Chapters)
{
    /// <summary>Each stream's track in the file, by the stream's index: what reading its blocks needs.</summary>
    public IReadOnlyList<MatroskaTrack> Tracks { get; init; } = new MatroskaTrack[0];
    /// <summary>Where the Segment's data starts: positions in it count from here.</summary>
    public long SegmentStart { get; init; }
    /// <summary>Where the Cues are, from the file's start; null when the file has none.</summary>
    public long? CuesAt { get; init; }
    /// <summary>A timestamp's unit, in nanoseconds.</summary>
    public long ScaleNs { get; init; } = 1_000_000;
}

/// <summary>A track as the file numbers and codes it: its blocks name its number; their bytes may be compressed (0: zlib, 3: a stripped header).</summary>
public sealed record MatroskaTrack(int Index, long Number, string CodecId, byte[]? Private, long? CompAlgo, byte[]? CompSettings);

/// <summary>
/// Reads a Matroska file's head (EBML, the Segment's Info, Tracks and Chapters) for a file played
/// from a drive, which no server has probed: the player's audio and subtitle choices and its
/// chapters. Elements the head doesn't hold are found through the SeekHead and read where it says.
/// </summary>
public static class MatroskaHead
{
    const long Ebml = 0x1a45dfa3;
    const long Segment = 0x18538067;
    const long SeekHead = 0x114d9b74;
    const long Seek = 0x4dbb;
    const long SeekId = 0x53ab;
    const long SeekPosition = 0x53ac;
    const long Info = 0x1549a966;
    const long TimecodeScale = 0x2ad7b1;
    const long Duration = 0x4489;
    const long Tracks = 0x1654ae6b;
    const long TrackEntry = 0xae;
    const long TrackType = 0x83;
    const long FlagDefault = 0x88;
    const long FlagForced = 0x55aa;
    const long CodecId = 0x86;
    const long Language = 0x22b59c;
    const long LanguageIetf = 0x22b59d;
    const long Name = 0x536e;
    const long Video = 0xe0;
    const long PixelWidth = 0xb0;
    const long PixelHeight = 0xba;
    const long Colour = 0x55b0;
    const long MatrixCoefficients = 0x55b1;
    const long BitsPerChannel = 0x55b2;
    const long TransferCharacteristics = 0x55ba;
    const long Primaries = 0x55bb;
    const long BlockAdditionMapping = 0x41e4;
    const long BlockAddIdType = 0x41e7;
    const long BlockAddIdExtraData = 0x41ed;
    const long TrackNumber = 0xd7;
    const long CodecPrivate = 0x63a2;
    const long ContentEncodings = 0x6d80;
    const long ContentEncoding = 0x6240;
    const long ContentCompression = 0x5034;
    const long ContentCompAlgo = 0x4254;
    const long ContentCompSettings = 0x4255;
    const long Cues = 0x1c53bb6b;
    const long Audio = 0xe1;
    const long Channels = 0x9f;
    const long SamplingFrequency = 0xb5;
    const long Chapters = 0x1043a770;
    const long EditionEntry = 0x45b9;
    const long ChapterAtom = 0xb6;
    const long ChapterTimeStart = 0x91;
    const long ChapterTimeEnd = 0x92;
    const long ChapterDisplay = 0x80;
    const long ChapString = 0x85;
    const long Cluster = 0x1f43b675;

    /// <summary>The first frames read for HDR10+'s mark: a few, whatever their size.</summary>
    const int FramesMost = 4 * 1024 * 1024;

    /// <summary>The most of an element read from where the SeekHead points: Tracks and Chapters are a few KB.</summary>
    const int ElementMost = 1024 * 1024;

    /// <summary>
    /// The facts of the file whose bytes <paramref name="read"/> gives (offset, count: as many as
    /// there are, fewer at its end); null when it isn't Matroska.
    /// </summary>
    /// <param name="head">The file's first bytes, a few hundred KB.</param>
    public static LocalFacts? Read(byte[] head, Func<long, int, byte[]> read)
    {
        var ebml = Element(head, 0, head.Length);
        if (ebml == null || ebml.Value.Id != Ebml || ebml.Value.Size == null) return null;
        var segment = Element(head, ebml.Value.End, head.Length);
        if (segment == null || segment.Value.Id != Segment) return null;
        var segmentData = segment.Value.DataStart;
        var found = new Dictionary<long, (byte[] Bytes, long Start, long End)>();
        var seeks = new Dictionary<long, long>();
        var at = segmentData;
        long? firstCluster = null;
        while (at < head.Length)
        {
            var e = Element(head, at, head.Length);
            if (e != null && e.Value.Id == Cluster) firstCluster = at;
            if (e == null || e.Value.Id == Cluster || e.Value.Size == null) break;
            var end = Math.Min(e.Value.End, head.Length);
            if (e.Value.Id == SeekHead) ReadSeeks(head, e.Value.DataStart, end, seeks);
            else if (e.Value.Id is Info or Tracks or Chapters && e.Value.End <= head.Length) found[e.Value.Id] = (head, e.Value.DataStart, e.Value.End);
            at = e.Value.End;
        }
        // Past the head (FFmpeg writes Chapters after the clusters): where the SeekHead says.
        foreach (var id in new[] { Info, Tracks, Chapters })
        {
            if (found.ContainsKey(id) || !seeks.TryGetValue(id, out var position)) continue;
            var bytes = read(segmentData + position, ElementMost);
            var e = Element(bytes, 0, bytes.Length);
            if (e == null || e.Value.Id != id || e.Value.Size == null || e.Value.End > bytes.Length) continue;
            found[id] = (bytes, e.Value.DataStart, e.Value.End);
        }
        long scaleNs = 1_000_000;
        long? durationMs = null;
        if (found.TryGetValue(Info, out var info))
        {
            double? duration = null;
            foreach (var c in Children(info.Bytes, info.Start, info.End))
            {
                if (c.Id == TimecodeScale) scaleNs = Math.Max(1, Uint(info.Bytes, c));
                else if (c.Id == Duration) duration = Float(info.Bytes, c);
            }
            if (duration is double d && d > 0) durationMs = (long)(d * scaleNs / 1_000_000);
        }
        var numbered = new List<MatroskaTrack>();
        var streams = found.TryGetValue(Tracks, out var tracks) ? ReadTracks(tracks.Bytes, tracks.Start, tracks.End, numbered) : new List<MediaStream>();
        // What the frames say of the picture (HDR10's metadata, Dolby Vision's RPU, HDR10+), which many
        // files' heads leave out: read from the first cluster, wherever it starts (after a few MB of
        // fonts, in some files).
        for (var i = 0; i < streams.Count; i++)
        {
            if (streams[i].Kind != "video" || streams[i].Codec != "hevc") continue;
            var at2 = firstCluster ?? (seeks.TryGetValue(Cluster, out var c) ? segmentData + c : (long?)null);
            if (at2 is not long first) break;
            var frames = first + FramesMost <= head.Length ? head.Skip((int)first).Take(FramesMost).ToArray() : read(first, FramesMost);
            streams[i] = HevcFrames.Apply(streams[i], HevcFrames.Scan(frames));
            break;
        }
        var chapters = found.TryGetValue(Chapters, out var ch) ? ReadChapters(ch.Bytes, ch.Start, ch.End, durationMs) : new List<Chapter>();
        return new LocalFacts(durationMs, streams, chapters)
        {
            Tracks = numbered,
            SegmentStart = segmentData,
            CuesAt = seeks.TryGetValue(Cues, out var cues) ? segmentData + cues : null,
            ScaleNs = scaleNs,
        };
    }

    static void ReadSeeks(byte[] b, long from, long to, Dictionary<long, long> seeks)
    {
        foreach (var seek in Children(b, from, to))
        {
            if (seek.Id != Seek) continue;
            long? id = null, position = null;
            foreach (var c in Children(b, seek.DataStart, seek.End))
            {
                if (c.Id == SeekId) id = Uint(b, c);
                else if (c.Id == SeekPosition) position = Uint(b, c);
            }
            if (id is long i && position is long p && !seeks.ContainsKey(i)) seeks[i] = p;
        }
    }

    static List<MediaStream> ReadTracks(byte[] b, long from, long to, List<MatroskaTrack> numbered)
    {
        var streams = new List<MediaStream>();
        foreach (var entry in Children(b, from, to))
        {
            if (entry.Id != TrackEntry) continue;
            long type = 0;
            var isDefault = true;
            var forced = false;
            string codec = "", language = "eng";
            string? ietf = null, name = null;
            int? width = null, height = null, channels = null, rate = null, depth = null;
            long transfer = 2, primaries = 2, matrix = 2;
            int? dvProfile = null, dvLevel = null, dvCompat = null;
            var hdr10plus = false;
            long number = 0;
            byte[]? codecPrivate = null, compSettings = null;
            long? compAlgo = null;
            foreach (var c in Children(b, entry.DataStart, entry.End))
            {
                switch (c.Id)
                {
                    case TrackNumber: number = Uint(b, c); break;
                    case CodecPrivate: codecPrivate = Bytes(b, c); break;
                    case ContentEncodings:
                        foreach (var enc in Children(b, c.DataStart, c.End))
                        {
                            if (enc.Id != ContentEncoding) continue;
                            foreach (var part in Children(b, enc.DataStart, enc.End))
                            {
                                if (part.Id != ContentCompression) continue;
                                compAlgo = 0;
                                foreach (var x in Children(b, part.DataStart, part.End))
                                {
                                    if (x.Id == ContentCompAlgo) compAlgo = Uint(b, x);
                                    else if (x.Id == ContentCompSettings) compSettings = Bytes(b, x);
                                }
                            }
                        }
                        break;
                    case TrackType: type = Uint(b, c); break;
                    case FlagDefault: isDefault = Uint(b, c) != 0; break;
                    case FlagForced: forced = Uint(b, c) != 0; break;
                    case CodecId: codec = Text(b, c); break;
                    case Language: language = Text(b, c); break;
                    case LanguageIetf: ietf = Text(b, c); break;
                    case Name: name = Text(b, c); break;
                    case Video:
                        foreach (var v in Children(b, c.DataStart, c.End))
                        {
                            if (v.Id == PixelWidth) width = (int)Uint(b, v);
                            else if (v.Id == PixelHeight) height = (int)Uint(b, v);
                            else if (v.Id == Colour)
                                foreach (var k in Children(b, v.DataStart, v.End))
                                {
                                    if (k.Id == TransferCharacteristics) transfer = Uint(b, k);
                                    else if (k.Id == Primaries) primaries = Uint(b, k);
                                    else if (k.Id == MatrixCoefficients) matrix = Uint(b, k);
                                    else if (k.Id == BitsPerChannel) depth = (int)Uint(b, k);
                                }
                        }
                        break;
                    case BlockAdditionMapping:
                    {
                        long addType = 0;
                        El? extra = null;
                        foreach (var m in Children(b, c.DataStart, c.End))
                        {
                            if (m.Id == BlockAddIdType) addType = Uint(b, m);
                            else if (m.Id == BlockAddIdExtraData) extra = m;
                        }
                        // ITU-T T.35: HDR10+'s dynamic metadata, as mkvmerge marks it.
                        if (addType == 4) hdr10plus = true;
                        // Dolby Vision's configuration record ("dvcC", "dvvC"): profile, level, base layer.
                        else if ((addType == 0x64766343 || addType == 0x64767643) && extra is El x && x.End - x.DataStart >= 5 && x.End <= b.Length)
                        {
                            var at = (int)x.DataStart;
                            dvProfile = (b[at + 2] >> 1) & 0x7f;
                            dvLevel = ((b[at + 2] & 1) << 5) | (b[at + 3] >> 3);
                            dvCompat = b[at + 4] >> 4;
                        }
                        break;
                    }
                    case Audio:
                        foreach (var a in Children(b, c.DataStart, c.End))
                        {
                            if (a.Id == Channels) channels = (int)Uint(b, a);
                            else if (a.Id == SamplingFrequency) rate = (int)(Float(b, a) ?? 0);
                        }
                        break;
                }
            }
            var kind = type switch { 1 => "video", 2 => "audio", 0x11 => "subtitle", _ => null };
            if (kind == null) continue;
            var ffmpeg = CodecName(codec);
            numbered.Add(new MatroskaTrack(streams.Count, number, codec, codecPrivate, compAlgo, compSettings));
            var range = kind != "video" ? null
                : dvProfile != null ? "dolby_vision"
                : transfer == 16 ? (hdr10plus ? "hdr10plus" : "hdr10")
                : transfer == 18 ? "hlg"
                : "sdr";
            streams.Add(new MediaStream
            {
                Index = streams.Count,
                Kind = kind,
                Codec = ffmpeg,
                // "und" says nothing; the IETF tag is the newer, finer name.
                Language = Lang(ietf) ?? (language == "und" ? null : language),
                Title = string.IsNullOrWhiteSpace(name) ? null : name,
                Default = isDefault,
                Forced = forced,
                Width = width,
                Height = height,
                Channels = channels,
                SampleRate = rate > 0 ? rate : null,
                BitDepth = kind == "video" ? depth : null,
                DynamicRange = range,
                Hdr = kind == "video" ? range != "sdr" : null,
                Hdr10plus = kind == "video" && hdr10plus ? true : null,
                DvProfile = dvProfile,
                DvLevel = dvLevel,
                DvBlCompat = dvCompat,
                ColorTransfer = kind != "video" ? null : transfer switch
                {
                    16 => "smpte2084",
                    18 => "arib-std-b67",
                    1 => "bt709",
                    // Unsaid: Dolby Vision's base layer says it (8.1 and 7: HDR10, 8.4: HLG).
                    _ => dvProfile != null ? dvCompat switch { 1 or 6 => "smpte2084", 4 => "arib-std-b67", _ => null } : null,
                },
                ColorPrimaries = kind != "video" ? null : primaries switch { 9 => "bt2020", 1 => "bt709", _ => null },
                ColorSpace = kind != "video" ? null : matrix switch { 9 => "bt2020nc", 1 => "bt709", _ => null },
                TextBased = kind == "subtitle" ? ffmpeg is "subrip" or "ass" or "ssa" or "webvtt" or "mov_text" : null,
            });
        }
        return streams;
    }

    /// <summary>The first edition's chapters, in order; one that doesn't say where it ends ends where the next starts.</summary>
    static List<Chapter> ReadChapters(byte[] b, long from, long to, long? durationMs)
    {
        var chapters = new List<Chapter>();
        foreach (var edition in Children(b, from, to))
        {
            if (edition.Id != EditionEntry) continue;
            foreach (var atom in Children(b, edition.DataStart, edition.End))
            {
                if (atom.Id != ChapterAtom) continue;
                long start = 0;
                long? end = null;
                string? title = null;
                foreach (var c in Children(b, atom.DataStart, atom.End))
                {
                    if (c.Id == ChapterTimeStart) start = Uint(b, c) / 1_000_000;
                    else if (c.Id == ChapterTimeEnd) end = Uint(b, c) / 1_000_000;
                    else if (c.Id == ChapterDisplay && title == null)
                        foreach (var d in Children(b, c.DataStart, c.End))
                            if (d.Id == ChapString) title = Text(b, d);
                }
                chapters.Add(new Chapter { StartMs = start, EndMs = end ?? 0, Title = string.IsNullOrWhiteSpace(title) ? null : title });
            }
            break;
        }
        chapters.Sort((x, y) => x.StartMs.CompareTo(y.StartMs));
        for (var i = 0; i < chapters.Count; i++)
        {
            if (chapters[i].EndMs > chapters[i].StartMs) continue;
            var next = i + 1 < chapters.Count ? chapters[i + 1].StartMs : durationMs ?? chapters[i].StartMs;
            chapters[i] = chapters[i] with { EndMs = next };
        }
        return chapters;
    }

    /// <summary>Matroska's codec id as ffprobe names the codec, which the player's rules know.</summary>
    public static string CodecName(string id)
    {
        if (id.StartsWith("A_AAC", StringComparison.Ordinal)) return "aac";
        if (id.StartsWith("A_PCM", StringComparison.Ordinal)) return "pcm_s16le";
        if (id.StartsWith("A_DTS", StringComparison.Ordinal)) return "dts";
        return id switch
        {
            "V_MPEGH/ISO/HEVC" => "hevc",
            "V_MPEG4/ISO/AVC" => "h264",
            "V_AV1" => "av1",
            "V_VP9" => "vp9",
            "V_VP8" => "vp8",
            "V_MPEG2" => "mpeg2video",
            "V_MPEG4/ISO/ASP" or "V_MPEG4/ISO/SP" => "mpeg4",
            "V_MS/VFW/FOURCC" => "unknown",
            "A_AC3" => "ac3",
            "A_EAC3" => "eac3",
            "A_TRUEHD" => "truehd",
            "A_FLAC" => "flac",
            "A_OPUS" => "opus",
            "A_VORBIS" => "vorbis",
            "A_MPEG/L3" => "mp3",
            "A_MPEG/L2" => "mp2",
            "S_TEXT/UTF8" or "S_TEXT/ASCII" => "subrip",
            "S_TEXT/ASS" => "ass",
            "S_TEXT/SSA" => "ssa",
            "S_TEXT/WEBVTT" => "webvtt",
            "S_HDMV/PGS" => "hdmv_pgs_subtitle",
            "S_VOBSUB" => "dvd_subtitle",
            "S_DVBSUB" => "dvb_subtitle",
            _ => id.ToLowerInvariant(),
        };
    }

    /// <summary>An IETF tag's language ("en-US": "en"); null without one.</summary>
    static string? Lang(string? ietf)
    {
        if (string.IsNullOrWhiteSpace(ietf) || ietf == "und") return null;
        var dash = ietf!.IndexOf('-');
        return dash > 0 ? ietf.Substring(0, dash) : ietf;
    }

    internal readonly struct El
    {
        public El(long id, long dataStart, long? size)
        {
            Id = id;
            DataStart = dataStart;
            Size = size;
        }

        public long Id { get; }
        public long DataStart { get; }
        /// <summary>Null: unknown (a live file's Segment or Cluster).</summary>
        public long? Size { get; }
        public long End => Size is long s ? DataStart + s : long.MaxValue;
    }

    internal static IEnumerable<El> Children(byte[] b, long from, long to)
    {
        var at = from;
        while (at < to)
        {
            var e = Element(b, at, to);
            if (e == null || e.Value.Size == null || e.Value.End > to) yield break;
            yield return e.Value;
            at = e.Value.End;
        }
    }

    internal static El? Element(byte[] b, long at, long limit)
    {
        var id = Vint(b, at, limit, keepMarker: true);
        if (id == null) return null;
        var size = Vint(b, at + id.Value.Length, limit, keepMarker: false);
        if (size == null) return null;
        var dataStart = at + id.Value.Length + size.Value.Length;
        return new El(id.Value.Value, dataStart, size.Value.Unknown ? null : size.Value.Value);
    }

    internal static (long Value, int Length, bool Unknown)? Vint(byte[] b, long at, long limit, bool keepMarker)
    {
        var end = Math.Min(limit, b.Length);
        if (at < 0 || at >= end) return null;
        int first = b[at];
        if (first == 0) return null;
        var length = 1;
        var mask = 0x80;
        while ((first & mask) == 0)
        {
            mask >>= 1;
            length++;
        }
        if (at + length > end) return null;
        long value = keepMarker ? first : first & (mask - 1);
        var allOnes = (first & (mask - 1)) == mask - 1;
        for (var i = 1; i < length; i++)
        {
            var x = b[at + i];
            if (x != 0xff) allOnes = false;
            value = unchecked(value * 256 + x);
        }
        return (value, length, !keepMarker && allOnes);
    }

    internal static long Uint(byte[] b, El e)
    {
        long value = 0;
        for (var i = e.DataStart; i < e.End && i < b.Length; i++) value = unchecked(value * 256 + b[i]);
        return value;
    }

    static byte[] Bytes(byte[] b, El e)
    {
        var n = (int)(Math.Min(e.End, b.Length) - e.DataStart);
        var bytes = new byte[Math.Max(0, n)];
        if (n > 0) Array.Copy(b, e.DataStart, bytes, 0, n);
        return bytes;
    }

    static double? Float(byte[] b, El e)
    {
        var n = (int)(e.End - e.DataStart);
        if (n != 4 && n != 8 || e.End > b.Length) return null;
        var bytes = new byte[n];
        Array.Copy(b, e.DataStart, bytes, 0, n);
        if (BitConverter.IsLittleEndian) Array.Reverse(bytes);
        return n == 4 ? BitConverter.ToSingle(bytes, 0) : BitConverter.ToDouble(bytes, 0);
    }

    static string Text(byte[] b, El e)
    {
        var n = (int)Math.Min(e.End, b.Length) - (int)e.DataStart;
        if (n <= 0) return "";
        var s = Encoding.UTF8.GetString(b, (int)e.DataStart, n);
        var nul = s.IndexOf('\0');
        return nul >= 0 ? s.Substring(0, nul) : s;
    }
}
