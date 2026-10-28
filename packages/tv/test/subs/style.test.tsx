import { render } from "preact";
import { describe, expect, it } from "vitest";
import { fontFamily, fontsHere, liftPx, SUBTITLE_FONTS, SUBTITLE_POSITIONS } from "../../src/subs/style";
import type { Cue } from "../../src/subs/vtt";
import { TextSubtitles } from "../../src/ui/player/subtitles";

describe("subtitle style", () => {
  it("offers the web's fonts and positions", () => {
    expect(SUBTITLE_FONTS.map((f) => f.label)).toEqual(["Default", "Serif", "Monospace", "Casual"]);
    expect(SUBTITLE_POSITIONS.map((p) => p.label)).toEqual(["Bottom", "A little higher", "Higher", "Highest"]);
    expect(SUBTITLE_POSITIONS.map((p) => liftPx(p.id))).toEqual([0, 27, 54, 86]);
  });

  it("offers a font only when the TV draws it differently from the app's own", () => {
    // A TV with a monospace face, whose serif and casual stacks fall back to its sans.
    const widths = (family: string) => (family.indexOf("monospace") >= 0 ? 1_200 : family.indexOf("cursive") >= 0 ? 1_000.3 : 1_000);
    expect(fontsHere(widths)).toEqual(["default", "mono"]);
    // An engine that can't measure offers the default alone.
    expect(fontsHere(() => null)).toEqual(["default"]);
  });

  it("draws the lines in the font chosen, raised as chosen, and above the controls while they show", () => {
    const root = document.createElement("div");
    const cue = { startMs: 0, endMs: 1_000, html: "Hello", line: null, snapToLines: true, align: "center" } as unknown as Cue;
    const box = () => root.querySelector<HTMLElement>(".subtitles")!;
    render(<TextSubtitles cues={[cue]} size="normal" box={false} lifted={false} />, root);
    expect(box().style.paddingBottom).toBe("40px");
    render(<TextSubtitles cues={[cue]} size="normal" box={false} lifted={false} font="serif" position="higher" />, root);
    expect(box().style.paddingBottom).toBe("94px");
    expect(box().style.fontFamily).toBe(fontFamily("serif"));
    render(<TextSubtitles cues={[cue]} size="normal" box={false} lifted font="serif" position="higher" />, root);
    expect(box().style.paddingBottom).toBe("153px");
    render(<TextSubtitles cues={[cue]} size="normal" box={false} lifted={false} position="highest" />, root);
    expect(box().style.paddingBottom).toBe("126px");
  });
});
