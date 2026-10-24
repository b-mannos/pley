import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SubtitleOverlay } from "./SubtitleOverlay";
import { DEFAULT_SUBTITLE_STYLE, type SubtitleCue } from "./subtitles";

/** A cue as the browser's WebVTT parser gives it: `<i>` stays a tag, the rest is text. */
function cue(text: string, extra: Partial<SubtitleCue> = {}): SubtitleCue {
  return {
    startTime: 1,
    endTime: 2,
    text,
    line: "auto",
    snapToLines: true,
    getCueAsHTML: () => {
      const fragment = document.createDocumentFragment();
      const italic = /^<i>(.*)<\/i>$/.exec(text);
      if (italic) {
        const i = document.createElement("i");
        i.textContent = italic[1] ?? "";
        fragment.append(i);
      } else {
        fragment.append(text);
      }
      return fragment;
    },
    ...extra,
  };
}

describe("SubtitleOverlay", () => {
  it("draws nothing between lines", () => {
    render(<SubtitleOverlay cues={[]} lifted={false} style={DEFAULT_SUBTITLE_STYLE} />);
    expect(screen.queryByTestId("subtitles")).not.toBeInTheDocument();
  });

  it("draws the lines showing, with their italics, the file's markup never", () => {
    render(
      <SubtitleOverlay
        cues={[cue("Where are we?\nNo idea."), cue("<i>(thunder)</i>", { startTime: 2 }), cue("<img src=x onerror=alert(1)>")]}
        lifted={false}
        style={DEFAULT_SUBTITLE_STYLE}
      />,
    );
    const overlay = screen.getByTestId("subtitles");
    expect(overlay).toHaveTextContent("Where are we? No idea.");
    expect(overlay.querySelector("i")).toHaveTextContent("(thunder)");
    expect(overlay.querySelector("img")).toBeNull();
    expect(overlay).toHaveTextContent("<img src=x onerror=alert(1)>");
  });

  it("moves above the controls while they show, and has a box when asked", () => {
    const { rerender } = render(<SubtitleOverlay cues={[cue("Hi")]} lifted={false} style={DEFAULT_SUBTITLE_STYLE} />);
    const line = () => screen.getByText("Hi");
    expect(line().parentElement?.className).toContain("pb-[6cqh]");
    rerender(<SubtitleOverlay cues={[cue("Hi")]} lifted style={{ size: "large", background: "box" }} />);
    expect(line().parentElement?.className).toContain("pb-[max(6cqh,7rem)]");
    expect(line()).toHaveAttribute("data-background", "box");
  });

  it("puts cues placed high at the top", () => {
    render(<SubtitleOverlay cues={[cue("SIGN", { line: 0 }), cue("Dialogue")]} lifted={false} style={DEFAULT_SUBTITLE_STYLE} />);
    const [top, bottom] = Array.from(screen.getByTestId("subtitles").firstElementChild?.children ?? []);
    expect(top).toHaveTextContent("SIGN");
    expect(bottom).toHaveTextContent("Dialogue");
  });
});
