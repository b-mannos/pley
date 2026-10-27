import { render } from "preact";
import { act } from "preact/test-utils";
import { describe, expect, it } from "vitest";
import { differs, memo } from "../../src/ui/kit/memo";

describe("memo", () => {
  it("tells what shows from what doesn't", () => {
    const f = () => undefined;
    expect(differs({ a: 1, list: [1, 2] }, { a: 1, list: [1, 2] })).toBe(false);
    expect(differs({ a: 1 }, { a: 2 })).toBe(true);
    expect(differs({ list: [1, 2] }, { list: [2, 1] })).toBe(true);
    // A new callback is the same callback; one appearing or going is a change.
    expect(differs({ onSelect: f }, { onSelect: () => undefined })).toBe(false);
    expect(differs({ onSelect: f }, { onSelect: undefined })).toBe(true);
    expect(differs({}, { onSelect: f })).toBe(true);
    // Ignored props never count.
    expect(differs({ priority: 1 }, { priority: 7 }, ["priority"])).toBe(false);
  });

  it("skips a render that changes nothing, and still calls the newest callback", () => {
    let renders = 0;
    const Button = memo((p: { label: string; onSelect: () => void; priority?: number }) => {
      renders++;
      return (
        <button type="button" onClick={p.onSelect}>
          {p.label}
        </button>
      );
    }, ["priority"]);
    const root = document.createElement("div");
    const said: string[] = [];
    act(() => render(<Button label="Play" priority={1} onSelect={() => said.push("first")} />, root));
    act(() => render(<Button label="Play" priority={4} onSelect={() => said.push("second")} />, root));
    expect(renders).toBe(1);
    root.querySelector("button")!.click();
    expect(said).toEqual(["second"]);
    act(() => render(<Button label="Resume" onSelect={() => said.push("third")} />, root));
    expect(renders).toBe(2);
    expect(root.textContent).toBe("Resume");
  });
});
