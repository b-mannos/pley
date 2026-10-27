import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FocusManager, nudge, setGroup, setHandlers, type GroupOptions } from "../../src/ui/focus";
import { Input } from "../../src/ui/input";

/** LG's Magic Remote's pointer, as the browser's mouse events, and its wheel. */

function el(tag: string, attrs: Record<string, string> = {}, children: Element[] = []): HTMLElement {
  const e = document.createElement(tag);
  for (const k in attrs) e.setAttribute(k, attrs[k]!);
  for (const c of children) e.appendChild(c);
  return e;
}

function item(id: string): HTMLElement {
  return el("div", { "data-f": "", "data-fid": id }, [el("span")]);
}

function group(kind: GroupOptions["kind"], children: Element[]): HTMLElement {
  const g = el("div", { "data-fg": kind }, children);
  setGroup(g, { kind });
  return g;
}

let detach: (() => void) | null = null;

function setup() {
  document.body.innerHTML = "";
  const a = item("a");
  const b = item("b");
  const c = item("c");
  const blank = el("div", { class: "blank" });
  const root = el("div", {}, [group("col", [a, b, c]), blank]);
  document.body.appendChild(root);
  const focus = new FocusManager();
  focus.setPage(root);
  const input = new Input(focus);
  detach = input.attach(document);
  focus.focus(a);
  const selected: string[] = [];
  for (const x of [a, b, c]) setHandlers(x, { onSelect: () => selected.push(x.getAttribute("data-fid")!) });
  return { a, b, c, blank, focus, input, selected };
}

let x = 0;
/** The pointer moves onto `target` (to a new place each time, unless `still`). */
function point(target: Element, still = false): void {
  if (!still) x += 7;
  target.dispatchEvent(new MouseEvent("mousemove", { bubbles: true, clientX: x, clientY: 100 }));
}
function click(target: Element): void {
  target.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: x, clientY: 100 }));
}
function ok(input: Input): void {
  input.dispatch("ok", "down", false);
  input.dispatch("ok", "up", false);
}
const fid = (f: FocusManager) => f.current?.getAttribute("data-fid") ?? null;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(1_000_000);
});
afterEach(() => {
  detach?.();
  detach = null;
  vi.useRealTimers();
});

describe("the pointer", () => {
  it("focuses what it moves onto, by pointer; content moving under a still pointer doesn't", () => {
    const { a, b, focus, input } = setup();
    point(b.firstElementChild!);
    expect(fid(focus)).toBe("b");
    expect(focus.byPointer).toBe(true);
    input.dispatch("up", "down", false);
    expect(fid(focus)).toBe("a");
    expect(focus.byPointer).toBe(false);
    // The page slid b under the pointer, which didn't move: the keys keep focus.
    point(b, true);
    expect(fid(focus)).toBe("a");
    point(b);
    expect(fid(focus)).toBe("b");
    expect(a.hasAttribute("data-focused")).toBe(false);
  });

  it("selects what it clicks, focused", () => {
    const { c, focus, selected } = setup();
    click(c.firstElementChild!);
    expect(selected).toEqual(["c"]);
    expect(fid(focus)).toBe("c");
  });

  it("selects once for an OK press sent both as the key and as a click, in either order", () => {
    const { a, input, selected } = setup();
    // The key, then its click.
    ok(input);
    vi.advanceTimersByTime(60);
    click(a);
    expect(selected).toEqual(["a"]);
    // The click, then its key.
    vi.advanceTimersByTime(1000);
    click(a);
    vi.advanceTimersByTime(60);
    ok(input);
    expect(selected).toEqual(["a", "a"]);
    // Presses that come one way only are each a press.
    vi.advanceTimersByTime(1000);
    click(a);
    vi.advanceTimersByTime(150);
    click(a);
    vi.advanceTimersByTime(1000);
    ok(input);
    vi.advanceTimersByTime(150);
    ok(input);
    expect(selected.length).toBe(6);
  });

  it("drops the click of a press whose key is still down, and the one after a long press", () => {
    const { a, input, selected } = setup();
    let held = 0;
    setHandlers(a, { onSelect: () => selected.push("a"), onLongSelect: () => held++ });
    input.dispatch("ok", "down", false);
    click(a);
    input.dispatch("ok", "up", false);
    expect(selected).toEqual(["a"]);
    vi.advanceTimersByTime(1000);
    input.dispatch("ok", "down", false);
    vi.advanceTimersByTime(600);
    input.dispatch("ok", "up", false);
    click(a);
    expect(held).toBe(1);
    expect(selected).toEqual(["a"]);
  });

  it("makes OK do nothing while it shows over nothing, until the keys take over or it goes", () => {
    const { blank, focus, input, selected } = setup();
    point(blank);
    ok(input);
    expect(selected).toEqual([]);
    // A direction key: webOS hides the pointer, and OK is the key's again.
    input.dispatch("down", "down", false);
    input.dispatch("down", "up", false);
    ok(input);
    expect(selected).toEqual(["b"]);
    expect(fid(focus)).toBe("b");
    // Left alone, the pointer hides after a few seconds.
    point(blank);
    vi.advanceTimersByTime(3500);
    ok(input);
    expect(selected).toEqual(["b", "b"]);
  });

  it("tells the shell when it moves onto something outside where focus may go (the rail)", () => {
    const { focus } = setup();
    const outside = item("rail");
    document.body.appendChild(outside);
    const told: string[] = [];
    const input2 = new Input(focus);
    detach?.();
    detach = input2.attach(document);
    input2.onPointOutside = (e) => told.push(e.getAttribute("data-fid")!);
    point(outside);
    expect(told).toEqual(["rail"]);
    expect(fid(focus)).toBe("a");
  });
});

describe("the wheel", () => {
  function notch(dy: number): void {
    document.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: dy }));
  }

  it("moves focus a step a notch; notches in a quick run are a held key, once per 100 ms", () => {
    const { focus, input } = setup();
    const said: boolean[] = [];
    input.onHolding = (h) => said.push(h);
    notch(100);
    expect(fid(focus)).toBe("b");
    expect(said).toEqual([]);
    notch(100);
    expect(fid(focus)).toBe("b");
    vi.advanceTimersByTime(150);
    notch(100);
    expect(fid(focus)).toBe("c");
    expect(said).toEqual([true]);
    vi.advanceTimersByTime(1000);
    expect(said).toEqual([true, false]);
    notch(-100);
    expect(fid(focus)).toBe("b");
  });
});

describe("nudge", () => {
  it("leaves a scroller where it is while the element shows whole", () => {
    expect(nudge(100, 200, 300, 0, 540)).toBe(100);
  });
  it("moves only as far as brings in an element cut off at an edge", () => {
    // Its bottom at 700, shown down to 100 + 540: 60 more.
    expect(nudge(100, 600, 700, 0, 540)).toBe(160);
    // Its top at 350, shown from 300 + 160 (a clip line): back 110.
    expect(nudge(300, 350, 450, 160, 540)).toBe(190);
    // Its top at 120: back 140, but not past the content's top.
    expect(nudge(100, 120, 300, 160, 540)).toBe(0);
  });
  it("keeps within its bounds, and shows an element taller than the view from its top", () => {
    expect(nudge(0, 900, 1000, 0, 540, 0, 200)).toBe(200);
    expect(nudge(0, 100, 900, 0, 540)).toBe(100);
  });
});
