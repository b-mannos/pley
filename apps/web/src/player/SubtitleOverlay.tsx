import { useLayoutEffect, useRef } from "react";
import {
  cueAtTop,
  SUBTITLE_BOTTOM,
  subtitleFontFamily,
  subtitleFontSize,
  subtitleLift,
  type SubtitleCue,
  type SubtitleStyle,
} from "./subtitles";

function Cue({ cue, background }: { cue: SubtitleCue; background: SubtitleStyle["background"] }) {
  const ref = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    // The browser's WebVTT parser turns the cue's text into nodes (italics,
    // bold, voices): no markup from the file itself reaches the page.
    ref.current?.replaceChildren(cue.getCueAsHTML());
  }, [cue]);
  return <span ref={ref} className="subtitle-cue" data-background={background} />;
}

/**
 * Text subtitles over the video, drawn by pley rather than the browser:
 * white with a dark outline, sized to the player, in the font chosen, at
 * the bottom or as far above it as chosen (above the controls while they
 * show), or at the top for cues placed there.
 */
export function SubtitleOverlay({ cues, lifted, style }: { cues: SubtitleCue[]; lifted: boolean; style: SubtitleStyle }) {
  if (!cues.length) return null;
  const top = cues.filter(cueAtTop);
  const bottom = cues.filter((c) => !cueAtTop(c));
  const lines = (list: SubtitleCue[]) =>
    list.map((cue, i) => <Cue key={`${cue.startTime}-${i}-${cue.text}`} cue={cue} background={style.background} />);
  const under = SUBTITLE_BOTTOM + subtitleLift(style.position);
  return (
    <div className="subtitle-overlay pointer-events-none absolute inset-0" data-testid="subtitles">
      <div
        className="absolute inset-0 flex flex-col justify-between"
        style={{ fontSize: subtitleFontSize(style.size), ["--subtitle-font" as string]: subtitleFontFamily(style.font) }}
      >
        <div className="flex flex-col items-center gap-[0.15em] px-[6%] pt-[5cqh]">{lines(top)}</div>
        <div
          className="flex flex-col items-center gap-[0.15em] px-[6%] transition-[padding] duration-300"
          style={{ paddingBottom: lifted ? `max(${under}cqh, 7rem)` : `${under}cqh` }}
          data-testid="subtitles-bottom"
        >
          {lines(bottom)}
        </div>
      </div>
    </div>
  );
}
