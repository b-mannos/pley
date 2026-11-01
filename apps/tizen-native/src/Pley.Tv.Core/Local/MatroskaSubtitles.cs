using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;

namespace Pley.Tv.Core.Local;

/// <summary>A subtitle's block: when it shows, for how long when the file says, and its bytes, uncompressed.</summary>
public sealed record SubtitleBlock(long StartMs, long? DurationMs, byte[] Data);

/// <summary>
/// A subtitle track read out of a Matroska file on a drive, as the server's transcoder reads one
/// (docs/TRANSCODER.md, "Subtitle extraction"): the Cues list each subtitle block with its cluster
/// and its place there, so the track is read block by block, a seek each, not the whole film.
/// Text becomes WebVTT; pictures a small Matroska file of the track alone, as the server's .mks.
/// </summary>
public static class MatroskaSubtitles
{
    const long Cluster = 0x1f43b675;
    const long Timecode = 0xe7;
    const long SimpleBlock = 0xa3;
    const long BlockGroup = 0xa0;
    const long Block = 0xa1;
    const long BlockDuration = 0x9b;
    const long CuePoint = 0xbb;
    const long CueTime = 0xb3;
    const long CueTrackPositions = 0xb7;
    const long CueTrack = 0xf7;
    const long CueClusterPosition = 0xf1;
    const long CueRelativePosition = 0xf0;
    const long CueDuration = 0xb2;

    /// <summary>The most of the Cues read: a film's are a few MB at most.</summary>
    const int CuesMost = 32 * 1024 * 1024;
    /// <summary>A block read where the Cues put it: a subtitle's is small, a picture's under this.</summary>
    const int BlockMost = 1024 * 1024;
    /// <summary>A cluster read whole when the Cues don't say where in it the block is.</summary>
    const int ClusterMost = 16 * 1024 * 1024;

    /// <summary>
    /// <paramref name="track"/>'s blocks in time order, read through <paramref name="read"/>; each
    /// read is passed on to <paramref name="found"/> as it comes, so what is read shows while the
    /// rest is. Empty when the file has no Cues for it (FFmpeg writes them for video alone).
    /// </summary>
    public static IReadOnlyList<SubtitleBlock> Read(Func<long, int, byte[]> read, LocalFacts facts, MatroskaTrack track,
        Action<SubtitleBlock>? found = null, CancellationToken cancel = default)
    {
        if (facts.CuesAt is not long cuesAt) return new SubtitleBlock[0];
        var cues = CuesOf(read, cuesAt, track.Number, facts.ScaleNs);
        var blocks = new List<SubtitleBlock>();
        var clusterTimes = new Dictionary<long, long?>();
        foreach (var cue in cues.OrderBy(c => c.Cluster).ThenBy(c => c.Relative ?? 0))
        {
            cancel.ThrowIfCancellationRequested();
            var clusterAt = facts.SegmentStart + cue.Cluster;
            if (!clusterTimes.TryGetValue(clusterAt, out var clusterTime)) clusterTimes[clusterAt] = clusterTime = ClusterTime(read, clusterAt);
            if (clusterTime is not long ct) continue;
            foreach (var raw in cue.Relative is long rel ? BlockAt(read, clusterAt, rel, track.Number) : ClusterBlocks(read, clusterAt, track.Number))
            {
                var startMs = (ct + raw.Relative) * facts.ScaleNs / 1_000_000;
                var durationMs = raw.Duration is long d ? d * facts.ScaleNs / 1_000_000 : cue.DurationMs;
                var block = new SubtitleBlock(startMs, durationMs, Uncompressed(raw.Data, track));
                // Cues may name a block twice (one per track position): keep it once.
                if (blocks.Any(b => b.StartMs == startMs)) continue;
                blocks.Add(block);
                found?.Invoke(block);
            }
        }
        blocks.Sort((a, b) => a.StartMs.CompareTo(b.StartMs));
        return blocks;
    }

    sealed record Cue(long Cluster, long? Relative, long? DurationMs);

    static List<Cue> CuesOf(Func<long, int, byte[]> read, long at, long trackNumber, long scaleNs)
    {
        var head = read(at, 16);
        var e = MatroskaHead.Element(head, 0, head.Length);
        if (e == null || e.Value.Size is not long size) return new List<Cue>();
        var bytes = read(at, (int)Math.Min(CuesMost, e.Value.DataStart + size));
        var cues = new List<Cue>();
        foreach (var point in MatroskaHead.Children(bytes, e.Value.DataStart, Math.Min(bytes.Length, e.Value.DataStart + size)))
        {
            if (point.Id != CuePoint) continue;
            foreach (var p in MatroskaHead.Children(bytes, point.DataStart, point.End))
            {
                if (p.Id != CueTrackPositions) continue;
                long track = 0, cluster = -1;
                long? relative = null, duration = null;
                foreach (var c in MatroskaHead.Children(bytes, p.DataStart, p.End))
                {
                    if (c.Id == CueTrack) track = MatroskaHead.Uint(bytes, c);
                    else if (c.Id == CueClusterPosition) cluster = MatroskaHead.Uint(bytes, c);
                    else if (c.Id == CueRelativePosition) relative = MatroskaHead.Uint(bytes, c);
                    else if (c.Id == CueDuration) duration = MatroskaHead.Uint(bytes, c) * scaleNs / 1_000_000;
                }
                if (track == trackNumber && cluster >= 0) cues.Add(new Cue(cluster, relative, duration));
            }
        }
        return cues;
    }

    /// <summary>A cluster's timestamp: its first child.</summary>
    static long? ClusterTime(Func<long, int, byte[]> read, long at)
    {
        var bytes = read(at, 64);
        var cluster = MatroskaHead.Element(bytes, 0, bytes.Length);
        if (cluster == null || cluster.Value.Id != Cluster) return null;
        foreach (var c in MatroskaHead.Children(bytes, cluster.Value.DataStart, bytes.Length))
            if (c.Id == Timecode) return MatroskaHead.Uint(bytes, c);
        return null;
    }

    sealed record RawBlock(short Relative, long? Duration, byte[] Data);

    /// <summary>The block at <paramref name="relative"/> in the cluster's data, when it is the track's.</summary>
    static IEnumerable<RawBlock> BlockAt(Func<long, int, byte[]> read, long clusterAt, long relative, long trackNumber)
    {
        var head = read(clusterAt, 16);
        var cluster = MatroskaHead.Element(head, 0, head.Length);
        if (cluster == null) yield break;
        var at = clusterAt + cluster.Value.DataStart + relative;
        var top = read(at, 16);
        var e = MatroskaHead.Element(top, 0, top.Length);
        if (e == null || e.Value.Size is not long size || size > BlockMost) yield break;
        var bytes = read(at, (int)(e.Value.DataStart + size));
        if (Parse(bytes, MatroskaHead.Element(bytes, 0, bytes.Length)!.Value, trackNumber) is { } block) yield return block;
    }

    /// <summary>Every block of the track in the cluster, read whole: Cues that don't say where in it.</summary>
    static IEnumerable<RawBlock> ClusterBlocks(Func<long, int, byte[]> read, long clusterAt, long trackNumber)
    {
        var head = read(clusterAt, 16);
        var cluster = MatroskaHead.Element(head, 0, head.Length);
        if (cluster == null || cluster.Value.Size is not long size) yield break;
        var bytes = read(clusterAt, (int)Math.Min(ClusterMost, cluster.Value.DataStart + size));
        foreach (var c in MatroskaHead.Children(bytes, cluster.Value.DataStart, bytes.Length))
            if (Parse(bytes, c, trackNumber) is { } block) yield return block;
    }

    static RawBlock? Parse(byte[] b, MatroskaHead.El e, long trackNumber)
    {
        long? duration = null;
        MatroskaHead.El? block = null;
        if (e.Id == SimpleBlock) block = e;
        else if (e.Id == BlockGroup)
        {
            foreach (var c in MatroskaHead.Children(b, e.DataStart, e.End))
            {
                if (c.Id == Block) block = c;
                else if (c.Id == BlockDuration) duration = MatroskaHead.Uint(b, c);
            }
        }
        if (block is not MatroskaHead.El x || x.End > b.Length) return null;
        var track = MatroskaHead.Vint(b, x.DataStart, x.End, keepMarker: false);
        if (track == null || track.Value.Value != trackNumber) return null;
        var at = (int)x.DataStart + track.Value.Length;
        if (at + 3 > x.End) return null;
        var relative = (short)((b[at] << 8) | b[at + 1]);
        var start = at + 3;
        var data = new byte[(int)x.End - start];
        Array.Copy(b, start, data, 0, data.Length);
        return new RawBlock(relative, duration, data);
    }

    /// <summary>A block's bytes as the codec has them: zlib undone, a stripped header put back.</summary>
    static byte[] Uncompressed(byte[] data, MatroskaTrack track)
    {
        if (track.CompAlgo == 3 && track.CompSettings is { } header) return header.Concat(data).ToArray();
        if (track.CompAlgo != 0 || data.Length < 2) return data;
        try
        {
            // zlib: its two-byte header, then deflate.
            using var input = new MemoryStream(data, 2, data.Length - 2);
            using var inflate = new DeflateStream(input, CompressionMode.Decompress);
            using var output = new MemoryStream();
            inflate.CopyTo(output);
            return output.ToArray();
        }
        catch (Exception)
        {
            return data;
        }
    }

    /// <summary>A text track's blocks as WebVTT. ASS lines lose their fields and styling; a block without a duration ends at the next.</summary>
    public static string WebVtt(IReadOnlyList<SubtitleBlock> blocks, string codecId)
    {
        var vtt = new StringBuilder("WEBVTT\n\n");
        var ass = codecId.StartsWith("S_TEXT/ASS", StringComparison.Ordinal) || codecId.StartsWith("S_TEXT/SSA", StringComparison.Ordinal);
        for (var i = 0; i < blocks.Count; i++)
        {
            var b = blocks[i];
            var end = b.DurationMs is long d && d > 0 ? b.StartMs + d : i + 1 < blocks.Count ? blocks[i + 1].StartMs : b.StartMs + 4_000;
            var text = Encoding.UTF8.GetString(b.Data).TrimEnd('\0');
            if (ass) text = AssText(text);
            text = text.Replace("\r\n", "\n").Trim();
            if (text.Length == 0) continue;
            vtt.Append(Time(b.StartMs)).Append(" --> ").Append(Time(end)).Append('\n').Append(text).Append("\n\n");
        }
        return vtt.ToString();
    }

    /// <summary>An ASS event's text (the ninth field), without its override tags.</summary>
    static string AssText(string line)
    {
        var parts = line.Split(new[] { ',' }, 9);
        var text = parts.Length == 9 ? parts[8] : line;
        text = Regex.Replace(text, @"\{[^}]*\}", "");
        return text.Replace("\\N", "\n").Replace("\\n", "\n").Replace("\\h", " ");
    }

    static string Time(long ms) =>
        $"{ms / 3_600_000:00}:{ms / 60_000 % 60:00}:{ms / 1000 % 60:00}.{ms % 1000:000}";

    /// <summary>
    /// A picture track's blocks as a Matroska file of the track alone, as the server's .mks is:
    /// what the app's picture subtitles read.
    /// </summary>
    public static byte[] Mks(IReadOnlyList<SubtitleBlock> blocks, MatroskaTrack track)
    {
        var ebml = El(0x1a45dfa3, El(0x4282, Encoding.ASCII.GetBytes("matroska")));
        var info = El(0x1549a966, El(0x2ad7b1, U(1_000_000)));
        var entry = new List<byte[]> { El(0xd7, U(1)), El(0x83, U(0x11)), El(0x86, Encoding.ASCII.GetBytes(track.CodecId)) };
        if (track.Private != null) entry.Add(El(0x63a2, track.Private));
        var tracks = El(0x1654ae6b, El(0xae, entry.SelectMany(x => x).ToArray()));
        var body = new List<byte[]> { info, tracks };
        foreach (var b in blocks)
        {
            var block = new byte[] { 0x81, 0, 0, 0 }.Concat(b.Data).ToArray();
            var group = b.DurationMs is long d ? El(0xa0, El(0xa1, block).Concat(El(0x9b, U(d))).ToArray()) : El(0xa0, El(0xa1, block));
            body.Add(El(Cluster, El(Timecode, U(b.StartMs)).Concat(group).ToArray()));
        }
        return ebml.Concat(El(0x18538067, body.SelectMany(x => x).ToArray())).ToArray();
    }

    static byte[] El(long id, byte[] data)
    {
        var idBytes = new List<byte>();
        for (var v = id; v > 0; v >>= 8) idBytes.Insert(0, (byte)(v & 0xff));
        var size = new byte[8];
        size[0] = 0x01;
        var n = (long)data.Length;
        for (var i = 7; i >= 1; i--, n >>= 8) size[i] = (byte)(n & 0xff);
        return idBytes.Concat(size).Concat(data).ToArray();
    }

    static byte[] U(long v)
    {
        var bytes = BitConverter.GetBytes(v);
        if (BitConverter.IsLittleEndian) Array.Reverse(bytes);
        return bytes;
    }
}
