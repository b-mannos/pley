using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using Pley.Tv.Core.Api;

namespace Pley.Tv.Core.Local;

/// <summary>A stretch of a disc image: <paramref name="Offset"/> bytes in, <paramref name="Length"/> long.</summary>
public sealed record DiscSpan(long Offset, long Length);

/// <summary>
/// A DVD title: its number on the disc, its length and chapters, its audio and subtitle streams
/// as the player lists them, and the stretches of the image its video cells are, in play order.
/// </summary>
public sealed record DvdTitle(int Number, long DurationMs, IReadOnlyList<Chapter> Chapters, IReadOnlyList<MediaStream> Streams, IReadOnlyList<DiscSpan> Spans)
{
    public long Bytes => Spans.Sum(s => s.Length);
}

/// <summary>
/// The titles of a DVD-Video disc image, from its IFO files (no menus: the disc's own navigation
/// isn't run). VIDEO_TS.IFO lists the titles and their title sets; each VTS_nn_0.IFO gives a
/// title's program chain (cells, programs, times) and its streams' languages and codings.
/// </summary>
public static class DvdVideo
{
    const int Sector = DiscImage.Sector;
    /// <summary>The most of an IFO read: a few hundred KB at most.</summary>
    const int IfoMost = 4 * 1024 * 1024;

    /// <summary>The disc's titles, longest first; empty when it isn't DVD-Video.</summary>
    public static IReadOnlyList<DvdTitle> Titles(Func<long, int, byte[]> read)
    {
        var files = DiscImage.VideoTs(read);
        if (files == null || !files.TryGetValue("VIDEO_TS.IFO", out var vmgFile)) return new DvdTitle[0];
        var vmg = read(vmgFile.Offset, (int)Math.Min(vmgFile.Length, IfoMost));
        if (vmg.Length < 0xd0 || Encoding.ASCII.GetString(vmg, 0, 12) != "DVDVIDEO-VMG") return new DvdTitle[0];
        var tt = (int)U32(vmg, 0xc4) * Sector;
        if (tt + 8 > vmg.Length) return new DvdTitle[0];
        var count = U16(vmg, tt);
        var titles = new List<DvdTitle>();
        var sets = new Dictionary<int, byte[]?>();
        for (var i = 0; i < count; i++)
        {
            var at = tt + 8 + i * 12;
            if (at + 12 > vmg.Length) break;
            var vtsn = vmg[at + 6];
            var ttn = vmg[at + 7];
            if (!sets.TryGetValue(vtsn, out var vts))
            {
                vts = files.TryGetValue($"VTS_{vtsn:00}_0.IFO", out var ifo) ? read(ifo.Offset, (int)Math.Min(ifo.Length, IfoMost)) : null;
                if (vts != null && (vts.Length < 0x300 || Encoding.ASCII.GetString(vts, 0, 12) != "DVDVIDEO-VTS")) vts = null;
                sets[vtsn] = vts;
            }
            if (vts == null) continue;
            if (Title(vts, i + 1, ttn, Vobs(files, vtsn)) is { } title) titles.Add(title);
        }
        // Several titles can play the same cells (a "play all" and its episodes): each is kept.
        return titles.OrderByDescending(t => t.DurationMs).ToList();
    }

    /// <summary>The title set's VOBs holding its titles (VTS_nn_1.VOB on), in order: its cells' sectors count across them.</summary>
    static List<DiscFile> Vobs(IReadOnlyDictionary<string, DiscFile> files, int vtsn)
    {
        var vobs = new List<DiscFile>();
        for (var n = 1; n <= 9 && files.TryGetValue($"VTS_{vtsn:00}_{n}.VOB", out var vob); n++) vobs.Add(vob);
        return vobs;
    }

    static DvdTitle? Title(byte[] v, int number, int ttn, List<DiscFile> vobs)
    {
        var ptt = (int)U32(v, 0xc8) * Sector;
        var pgcit = (int)U32(v, 0xcc) * Sector;
        if (ptt + 8 > v.Length || pgcit + 8 > v.Length || ttn < 1 || ttn > U16(v, ptt)) return null;
        // The title's chapters (parts of title): each a program chain and a program in it.
        var listAt = ptt + (int)U32(v, ptt + 8 + (ttn - 1) * 4);
        var listEnd = ttn < U16(v, ptt) ? ptt + (int)U32(v, ptt + 8 + ttn * 4) : ptt + (int)U32(v, ptt + 4) + 1;
        var parts = new List<(int Pgcn, int Pgn)>();
        for (var at = listAt; at + 4 <= Math.Min(listEnd, v.Length); at += 4) parts.Add((U16(v, at), U16(v, at + 2)));
        if (parts.Count == 0) return null;
        var pgcn = parts[0].Pgcn;
        if (pgcn < 1 || pgcn > U16(v, pgcit)) return null;
        var pgc = pgcit + (int)U32(v, pgcit + 8 + (pgcn - 1) * 8 + 4);
        if (pgc + 0xec > v.Length) return null;
        var programs = v[pgc + 2];
        var cellCount = v[pgc + 3];
        var durationMs = Bcd(v, pgc + 4);
        var mapAt = pgc + U16(v, pgc + 0xe6);
        var cellsAt = pgc + U16(v, pgc + 0xe8);
        if (cellCount == 0 || cellsAt + cellCount * 24 > v.Length) return null;

        // The cells in play order, the first angle of an angle block only.
        var cells = new List<(int Number, long StartMs, long First, long Last)>();
        long clock = 0;
        for (var c = 0; c < cellCount; c++)
        {
            var at = cellsAt + c * 24;
            var blockMode = v[at] >> 6;
            var blockType = (v[at] >> 4) & 3;
            var time = Bcd(v, at + 4);
            if (blockType == 1 && blockMode > 1) continue;
            cells.Add((c + 1, clock, U32(v, at + 8), U32(v, at + 20)));
            clock += time;
        }
        if (durationMs <= 0) durationMs = clock;

        // Chapters: the parts of this title in this chain, each starting at its program's first cell.
        var chapters = new List<Chapter>();
        foreach (var (chain, program) in parts)
        {
            if (chain != pgcn || program < 1 || program > programs || mapAt + program > v.Length) continue;
            var entry = v[mapAt + program - 1];
            var cell = cells.FirstOrDefault(x => x.Number >= entry);
            if (cell.Number == 0) continue;
            chapters.Add(new Chapter { StartMs = cell.StartMs });
        }
        for (var i = 0; i < chapters.Count; i++)
            chapters[i] = chapters[i] with { EndMs = i + 1 < chapters.Count ? chapters[i + 1].StartMs : durationMs };

        // The cells' sectors, counted from the first VOB, as stretches of the image.
        var spans = new List<DiscSpan>();
        foreach (var cell in cells)
            foreach (var span in Spans(vobs, cell.First, cell.Last))
                if (spans.Count > 0 && spans[spans.Count - 1].Offset + spans[spans.Count - 1].Length == span.Offset)
                    spans[spans.Count - 1] = spans[spans.Count - 1] with { Length = spans[spans.Count - 1].Length + span.Length };
                else spans.Add(span);
        if (spans.Count == 0) return null;

        return new DvdTitle(number, durationMs, chapters, Streams(v, pgc), spans);
    }

    /// <summary>Sectors <paramref name="first"/> to <paramref name="last"/> of the VOBs, as stretches of the image (a VOB ends at 1 GB, and the next goes on).</summary>
    static IEnumerable<DiscSpan> Spans(List<DiscFile> vobs, long first, long last)
    {
        long start = 0;
        foreach (var vob in vobs)
        {
            var sectors = vob.Length / Sector;
            var from = Math.Max(first, start);
            var to = Math.Min(last, start + sectors - 1);
            if (from <= to) yield return new DiscSpan(vob.Offset + (from - start) * Sector, (to - from + 1) * Sector);
            start += sectors;
        }
    }

    /// <summary>The video, then the audio streams the program chain plays, then its subtitles, numbered as the player numbers them.</summary>
    static List<MediaStream> Streams(byte[] v, int pgc)
    {
        var streams = new List<MediaStream>();
        // The title set's video attributes: coding, TV system, then the resolution.
        var video = v[0x200];
        var pal = ((video >> 4) & 3) == 1;
        var resolution = (v[0x201] >> 3) & 7;
        var width = resolution switch { 0 => 720, 1 => 704, 2 => 352, _ => 352 };
        var height = resolution == 3 ? (pal ? 288 : 240) : pal ? 576 : 480;
        streams.Add(new MediaStream { Index = 0, Kind = "video", Codec = ((video >> 6) & 3) == 0 ? "mpeg1video" : "mpeg2video", Width = width, Height = height, FrameRate = pal ? 25 : 29.97, Default = true, DynamicRange = "sdr" });
        var audioCount = Math.Min(8, U16(v, 0x202));
        for (var a = 0; a < audioCount; a++)
        {
            // Only the streams the chain marks available play in it.
            if ((v[pgc + 0x0c + a * 2] & 0x80) == 0) continue;
            var at = 0x204 + a * 8;
            var coding = v[at] >> 5;
            var language = (v[at] >> 2 & 3) == 1 ? Lang(v, at + 2) : null;
            streams.Add(new MediaStream
            {
                Index = streams.Count,
                Kind = "audio",
                Codec = coding switch { 0 => "ac3", 2 or 3 => "mp2", 4 => "pcm_dvd", 6 => "dts", _ => "unknown" },
                Language = language,
                Channels = (v[at + 1] & 7) + 1,
                Default = streams.All(s => s.Kind != "audio"),
            });
        }
        var spuCount = Math.Min(32, U16(v, 0x254));
        for (var s = 0; s < spuCount; s++)
        {
            if ((v[pgc + 0x1c + s * 4] & 0x80) == 0) continue;
            var at = 0x256 + s * 6;
            streams.Add(new MediaStream
            {
                Index = streams.Count,
                Kind = "subtitle",
                Codec = "dvd_subtitle",
                Language = (v[at] & 3) == 1 ? Lang(v, at + 2) : null,
                TextBased = false,
            });
        }
        return streams;
    }

    static string? Lang(byte[] b, int at)
    {
        var code = Encoding.ASCII.GetString(b, at, 2);
        return code.All(char.IsLetter) ? code.ToLowerInvariant() : null;
    }

    /// <summary>A DVD time: hours, minutes, seconds and frames in BCD, the frame rate in the last byte's top bits.</summary>
    static long Bcd(byte[] b, int at)
    {
        if (at + 4 > b.Length) return 0;
        int D(byte x) => (x >> 4) * 10 + (x & 15);
        var fps = (b[at + 3] >> 6) == 1 ? 25.0 : 29.97;
        var frames = D((byte)(b[at + 3] & 0x3f));
        return (D(b[at]) * 3600L + D(b[at + 1]) * 60L + D(b[at + 2])) * 1000 + (long)(frames * 1000 / fps);
    }

    static int U16(byte[] b, int at) => at + 2 <= b.Length ? (b[at] << 8) | b[at + 1] : 0;

    static long U32(byte[] b, int at) => at + 4 <= b.Length ? ((long)b[at] << 24) | ((long)b[at + 1] << 16) | ((long)b[at + 2] << 8) | b[at + 3] : 0;
}
