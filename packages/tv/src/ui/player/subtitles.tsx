import { useLayoutEffect, useRef } from "preact/hooks";
import type { Settings } from "../../state";
import type { PictureFrame } from "../../subs/pictures";
import type { Cue } from "../../subs/vtt";
import { useUi } from "../context";

/** Subtitle text height as a share of the screen's (the Android app's sizes). */
const SIZES: Record<Settings["subtitleSize"], number> = { small: 0.038, normal: 0.048, large: 0.06, largest: 0.076 };

/** Whether a cue asks to sit near the top (WebVTT line 0–3, or under 50 %). */
function onTop(c: Cue): boolean {
  if (c.line === null) return false;
  return c.snapToLines ? c.line >= 0 && c.line < 4 : c.line < 50;
}

/**
 * Text subtitles over the whole screen: white bold text with a black
 * outline (or a dark box), at the bottom, lifted above the controls while
 * they show; cues placed at the top stay there.
 */
export function TextSubtitles(p: { cues: Cue[]; size: Settings["subtitleSize"]; box: boolean; lifted: boolean }) {
  if (!p.cues.length) return null;
  const px = Math.round(540 * SIZES[p.size] * 10) / 10;
  const bottom = p.cues.filter((c) => !onTop(c));
  const top = p.cues.filter(onTop);
  const cue = (c: Cue, i: number) => {
    const align = c.align === "start" || c.align === "left" ? "left" : c.align === "end" || c.align === "right" ? "right" : "center";
    return (
      <div key={`${c.startMs}-${i}`} class="cue-line" style={`text-align:${align}`}>
        <span class={`cue${p.box ? " boxed" : ""}`} dangerouslySetInnerHTML={{ __html: c.html }} />
      </div>
    );
  };
  return (
    <div class="subtitles" style={`font-size:${px}px;padding:27px 48px ${p.lifted ? 181 : 40}px 48px`} aria-live="off">
      <div class="cues-top">{top.map(cue)}</div>
      <div class="cues-bottom">{bottom.map(cue)}</div>
    </div>
  );
}

/**
 * Picture subtitles (PGS, VobSub, DVB): each bitmap where its plane puts
 * it, scaled to the whole screen; drawn at the screen's own pixels.
 */
export function PictureSubtitles(p: { frame: PictureFrame | null }) {
  const ui = useUi();
  const canvas = useRef<HTMLCanvasElement>(null);
  const w = Math.round(960 * ui.scale);
  const h = Math.round(540 * ui.scale);
  useLayoutEffect(() => {
    const el = canvas.current;
    if (!el) return;
    if (el.width !== w) el.width = w;
    if (el.height !== h) el.height = h;
    const g = el.getContext("2d");
    if (!g) return;
    g.clearRect(0, 0, w, h);
    const f = p.frame;
    if (!f || !f.bitmaps.length) return;
    const sx = w / f.planeWidth;
    const sy = h / f.planeHeight;
    g.imageSmoothingEnabled = true;
    for (const b of f.bitmaps) {
      if (!b.width || !b.height) continue;
      const off = document.createElement("canvas");
      off.width = b.width;
      off.height = b.height;
      const og = off.getContext("2d");
      if (!og) continue;
      const img = og.createImageData(b.width, b.height);
      img.data.set(b.rgba);
      og.putImageData(img, 0, 0);
      g.drawImage(off, Math.round(b.x * sx), Math.round(b.y * sy), Math.round(b.width * sx), Math.round(b.height * sy));
    }
  }, [p.frame, w, h]);
  return <canvas ref={canvas} class="picture-subtitles" aria-hidden="true" />;
}
