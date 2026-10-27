import { render } from "preact";
import { act } from "preact/test-utils";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { FocusManager } from "../../src/ui/focus";
import { Input } from "../../src/ui/input";
import { UiContext, type Ui } from "../../src/ui/context";
import { F } from "../../src/ui/kit/focusable";
import { SectionPage, type Section } from "../../src/ui/kit/row";

/**
 * A page of sections, laid out by hand: jsdom lays nothing out, so an
 * element's offsetTop and offsetHeight are its inline top and height, and
 * its offsetParent its parent.
 */
const saved = ["offsetTop", "offsetHeight", "offsetParent"].map((k) => [k, Object.getOwnPropertyDescriptor(HTMLElement.prototype, k)] as const);
beforeEach(() => {
  Object.defineProperty(HTMLElement.prototype, "offsetTop", { configurable: true, get(this: HTMLElement) { return parseFloat(this.style.top) || 0; } });
  Object.defineProperty(HTMLElement.prototype, "offsetHeight", { configurable: true, get(this: HTMLElement) { return parseFloat(this.style.height) || 0; } });
  Object.defineProperty(HTMLElement.prototype, "offsetParent", { configurable: true, get(this: HTMLElement) { return this.parentElement; } });
});
afterEach(() => {
  for (const [k, d] of saved) if (d) Object.defineProperty(HTMLElement.prototype, k, d);
});

function setup() {
  document.body.innerHTML = "";
  const root = document.createElement("div");
  document.body.appendChild(root);
  const focus = new FocusManager();
  focus.setPage(root);
  const ui = { focus, input: new Input(focus), claim: () => undefined } as unknown as Ui;
  // A hero, then rows 300 high, each two tiles 200 high, 40 below its header.
  const sections: Section[] = [{ key: "hero", height: 436, anchor: null, render: (onFocus) => <F fid="hero/play" style="top:300px;height:44px" onFocus={onFocus} /> }];
  for (let i = 0; i < 8; i++) {
    sections.push({
      key: `row${i}`,
      height: 300,
      anchor: 172,
      render: (onFocus) => (
        <>
          <F fid={`tile${i}`} style="top:40px;height:200px" onFocus={onFocus} />
          <F fid={`tile${i}b`} style="top:40px;height:200px" onFocus={onFocus} />
        </>
      ),
    });
  }
  const page = (focusedKey?: string, extraShift?: number) => (
    <UiContext.Provider value={ui}>
      <SectionPage sections={sections} clipAbove={160} focusedKey={focusedKey} extraShift={extraShift} />
    </UiContext.Provider>
  );
  act(() => render(page(), root));
  const scroll = () => (root.querySelector(".page-scroll") as HTMLElement).getAttribute("style") ?? "";
  const el = (fid: string) => root.querySelector<HTMLElement>(`[data-fid="${fid}"]`);
  return { root, focus, page, scroll, el, rerender: (k?: string, extra?: number) => act(() => render(page(k, extra), root)) };
}

// Rows start at 436 + 28 and come every 328: row i's header at 464 + 328 i.
const rowY = (i: number) => 464 + 328 * i;

describe("a page of sections", () => {
  it("comes up so the focused row's header sits at its anchor, drawing only what is near", () => {
    const { focus, scroll, el } = setup();
    act(() => void focus.focus(el("tile0")));
    expect(scroll()).toContain(`translateY(${-(rowY(0) - 172)}px)`);
    act(() => void focus.focus(el("tile1")));
    expect(scroll()).toContain(`translateY(${-(rowY(1) - 172)}px)`);
    // Far rows aren't drawn.
    expect(el("tile7")).toBeNull();
  });

  it("goes to a section the page asks for, drawn, at once when the jump is longer than a screen", () => {
    const { focus, scroll, el, rerender } = setup();
    for (let i = 0; i < 6; i++) act(() => void focus.focus(el(`tile${i}`)));
    expect(scroll()).toContain(`translateY(${-(rowY(5) - 172)}px)`);
    expect(el("hero/play")).toBeNull();
    rerender("row5");
    rerender("hero");
    expect(scroll()).toContain("translateY(0px)");
    expect(scroll()).toContain("transition: none");
    expect(el("hero/play")).not.toBeNull();
    // The next move slides again.
    act(() => void focus.focus(el("tile0")));
    expect(scroll()).not.toContain("transition: none");
  });

  it("stays where it is for the pointer, but for a tile cut off, which comes in only that far", () => {
    const { focus, scroll, el } = setup();
    // The first row's tile peeks under the hero: 464 + 40 = 504 to 704, the screen ends at 540.
    act(() => void focus.focus(el("tile0"), true));
    expect(scroll()).toContain(`translateY(${-(704 - 540)}px)`);
    // The keys put it at its anchor.
    act(() => void focus.focus(el("hero/play")));
    act(() => void focus.focus(el("tile0")));
    expect(scroll()).toContain(`translateY(${-(rowY(0) - 172)}px)`);
    // A tile shown whole: nothing moves.
    const before = scroll();
    act(() => void focus.focus(el("tile0b"), true));
    expect(scroll()).toBe(before);
    // The next row's, below the screen's bottom: it comes up only as far as shows it.
    act(() => void focus.focus(el("tile1"), true));
    expect(scroll()).toContain(`translateY(${-(rowY(1) + 240 - 540)}px)`);
    // And the first's again, now under the clip line: down only as far as shows it below the line.
    act(() => void focus.focus(el("tile0"), true));
    expect(scroll()).toContain(`translateY(${-(rowY(0) + 40 - 160)}px)`);
  });

  it("reads on past its last row from where the keys put it, after the pointer", () => {
    const { focus, scroll, el, rerender } = setup();
    act(() => void focus.focus(el("tile0"), true));
    expect(scroll()).toContain(`translateY(${-(704 - 540)}px)`);
    rerender(undefined, 160);
    expect(scroll()).toContain(`translateY(${-(rowY(0) - 172 + 160)}px)`);
  });
});
