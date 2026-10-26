import { describe, expect, it } from "vitest";
import { cueHtml, CueIndex, mergeCues, parseVtt, timeMs } from "../../src/subs/vtt";
import { clampOffset, formatOffset, OffsetStore } from "../../src/subs/sync";
import type { KeyValue } from "../../src/auth/tokens";

const VTT = `WEBVTT

1
00:00:01.000 --> 00:00:03.500
Hello <i>there</i>

2
00:00:02.000 --> 00:00:04.000 line:0 position:20% align:start size:50%
Sign & <b>top</b>

00:01:00,250 --> 00:01:02,000
Comma times
second line
`;

describe("WebVTT", () => {
  it("reads times, text and settings", () => {
    const cues = parseVtt(VTT);
    expect(cues.map((c) => [c.startMs, c.endMs])).toEqual([
      [1000, 3500],
      [2000, 4000],
      [60250, 62000],
    ]);
    expect(cues[0]!.html).toBe("Hello <i>there</i>");
    expect(cues[1]).toMatchObject({ line: 0, snapToLines: true, position: 20, align: "start", size: 50, html: "Sign &amp; <b>top</b>" });
    expect(cues[2]!.html).toBe("Comma times<br>second line");
  });

  it("keeps only safe markup", () => {
    expect(cueHtml('<script>alert(1)</script><v Joe>Hi</v> <c.yellow>x</c> &lt;b&gt;')).toBe("alert(1)Hi x &lt;b&gt;");
    expect(cueHtml("<i>open <b>both")).toBe("<i>open <b>both</b></i>");
    expect(cueHtml("<i>a <b>b</i> c</b>")).toBe("<i>a <b>b</b></i><b> c</b>");
    expect(cueHtml("&#9834; &amp;")).toBe("♪ &amp;");
  });

  it("parses times with and without hours", () => {
    expect(timeMs("01:02:03.456")).toBe(3723456);
    expect(timeMs("02:03,5")).toBe(123500);
  });

  it("finds the cues showing and when that changes", () => {
    const index = new CueIndex(parseVtt(VTT));
    expect(index.at(500)).toEqual([]);
    expect(index.at(2500).map((c) => c.startMs)).toEqual([1000, 2000]);
    expect(index.at(3500).map((c) => c.startMs)).toEqual([2000]);
    expect(index.nextChange(0)).toBe(1000);
    expect(index.nextChange(1000)).toBe(2000);
    expect(index.nextChange(2500)).toBe(3500);
    expect(index.nextChange(5000)).toBe(60250);
    expect(index.nextChange(70000)).toBeNull();
  });

  it("keeps the partial answer that reaches further", () => {
    const short = parseVtt("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\na\n");
    const long = parseVtt("WEBVTT\n\n00:00:01.000 --> 00:00:02.000\na\n\n00:00:05.000 --> 00:00:06.000\nb\n");
    expect(mergeCues(null, short)).toBe(short);
    expect(mergeCues(short, long)).toBe(long);
    expect(mergeCues(long, short)).toBe(long);
  });
});

describe("subtitle sync", () => {
  it("steps by 100 ms within 10 s", () => {
    expect(clampOffset(149)).toBe(100);
    expect(clampOffset(150)).toBe(200);
    expect(clampOffset(-12_345)).toBe(-10_000);
    expect(clampOffset(NaN)).toBe(0);
    expect(Object.is(clampOffset(-20), 0)).toBe(true);
  });

  it("writes offsets as people read them", () => {
    expect(formatOffset(500)).toBe("+0.5 s");
    expect(formatOffset(-1200)).toBe("−1.2 s");
    expect(formatOffset(0)).toBe("0.0 s");
  });

  it("remembers offsets per file, 0 forgetting", () => {
    const data = new Map<string, string>();
    const kv: KeyValue = { get: (k) => data.get(k) ?? null, set: (k, v) => void data.set(k, v), remove: (k) => void data.delete(k) };
    const store = new OffsetStore(kv);
    store.set("http://nas:7539", 7, 1500);
    store.set("http://nas:7539", 8, -300);
    expect(store.get("http://nas:7539", 7)).toBe(1500);
    expect(store.get("http://other:7539", 7)).toBe(0);
    store.set("http://nas:7539", 7, 0);
    expect(store.get("http://nas:7539", 7)).toBe(0);
    expect(store.get("http://nas:7539", 8)).toBe(-300);
  });
});
