import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { FocusManager, setGroup, setHandlers, type GroupOptions } from "../../src/ui/focus";
import { Input } from "../../src/ui/input";
import { KeyRepeatGate } from "../../src/ui/keys";

/**
 * jsdom lays nothing out: elements say where they are with `data-rect`
 * ("x,y,w,h" on the 960×540 canvas), which getBoundingClientRect returns.
 */
const original = Element.prototype.getBoundingClientRect;
beforeEach(() => {
  Element.prototype.getBoundingClientRect = function (this: Element) {
    const r = (this.getAttribute("data-rect") ?? "0,0,0,0").split(",").map(Number);
    const [x, y, w, h] = [r[0] ?? 0, r[1] ?? 0, r[2] ?? 0, r[3] ?? 0];
    return { x, y, width: w, height: h, left: x, top: y, right: x + w, bottom: y + h, toJSON: () => ({}) } as DOMRect;
  };
  document.body.innerHTML = "";
});
afterEach(() => {
  Element.prototype.getBoundingClientRect = original;
});

function el(tag: string, attrs: Record<string, string> = {}, children: Element[] = []): HTMLElement {
  const e = document.createElement(tag);
  for (const k in attrs) e.setAttribute(k, attrs[k]!);
  for (const c of children) e.appendChild(c);
  return e;
}

function item(id: string, rect: string): HTMLElement {
  return el("div", { "data-f": "", "data-fid": id, "data-rect": rect });
}

function group(kind: GroupOptions["kind"], children: Element[], opts: Partial<GroupOptions> = {}, rect = "0,0,0,0"): HTMLElement {
  const g = el("div", { "data-fg": kind, "data-rect": rect }, children);
  setGroup(g, { kind, ...opts });
  return g;
}

function page(...children: Element[]): { root: HTMLElement; focus: FocusManager } {
  const root = el("div", {}, children);
  document.body.appendChild(root);
  const focus = new FocusManager();
  focus.setPage(root);
  return { root, focus };
}

const fid = (f: FocusManager) => f.current?.getAttribute("data-fid") ?? null;

describe("focus moves", () => {
  it("walks a row and stops at its ends", () => {
    const row = group("row", [item("a", "96,100,100,150"), item("b", "210,100,100,150"), item("c", "324,100,100,150")]);
    const { focus } = page(row);
    focus.focus(row.children[0] as HTMLElement);
    expect(focus.move("right")).toBe(true);
    expect(fid(focus)).toBe("b");
    focus.move("right");
    expect(fid(focus)).toBe("c");
    expect(focus.move("right")).toBe(false);
    expect(fid(focus)).toBe("c");
  });

  it("asks the row's edge handler at its ends (Right stops, Left goes to the rail)", () => {
    let railed = 0;
    const row = group("row", [item("a", "96,100,100,150"), item("b", "210,100,100,150")], {
      onEdge: (dir) => {
        if (dir === "left") railed++;
        return true;
      },
    });
    const { focus } = page(row);
    focus.focus(row.children[0] as HTMLElement);
    expect(focus.move("left")).toBe(true);
    expect(railed).toBe(1);
    expect(fid(focus)).toBe("a");
  });

  it("moves through a grid by rows and columns, a short last row's Down to its last cell", () => {
    const cells = ["0", "1", "2", "3", "4"].map((i) => item(i, `${96 + Number(i) % 3 * 110},${100 + Math.floor(Number(i) / 3) * 200},96,144`));
    const grid = group("grid", cells, { columns: 3 });
    const { focus } = page(grid);
    focus.focus(cells[2]!);
    focus.move("down");
    expect(fid(focus)).toBe("4");
    focus.move("up");
    expect(fid(focus)).toBe("1");
    focus.move("left");
    expect(fid(focus)).toBe("0");
    expect(focus.move("left")).toBe(false);
  });

  it("comes back to the element focused last in a group that restores", () => {
    const top = group("row", [item("t1", "96,50,100,40"), item("t2", "210,50,100,40")]);
    const row = group("row", [item("r1", "96,200,100,150"), item("r2", "210,200,100,150"), item("r3", "324,200,100,150")], { restore: true });
    const col = group("col", [top, row]);
    const { focus } = page(col);
    focus.focus(row.children[2] as HTMLElement);
    focus.move("up");
    expect(fid(focus)).toMatch(/^t/);
    focus.move("down");
    expect(fid(focus)).toBe("r3");
  });

  it("enters a list at the item nearest along it", () => {
    const top = group("row", [item("t1", "96,50,100,40"), item("t2", "500,50,100,40")]);
    const row = group("row", [item("r1", "96,200,100,150"), item("r2", "300,200,100,150"), item("r3", "500,200,100,150")]);
    const { focus } = page(group("col", [top, row]));
    focus.focus(top.children[1] as HTMLElement);
    focus.move("down");
    expect(fid(focus)).toBe("r3");
  });

  it("finds the nearest element in the direction among free-form buttons", () => {
    const free = group("free", [item("left", "100,100,40,40"), item("right", "300,100,40,40"), item("below", "110,300,40,40"), item("far", "600,320,40,40")]);
    const { focus } = page(free);
    focus.focus(free.children[0] as HTMLElement);
    focus.move("right");
    expect(fid(focus)).toBe("right");
    focus.move("left");
    focus.move("down");
    expect(fid(focus)).toBe("below");
  });

  it("skips disabled elements", () => {
    const a = item("a", "96,100,100,40");
    const b = item("b", "210,100,100,40");
    b.setAttribute("data-fdisabled", "");
    const c = item("c", "324,100,100,40");
    const { focus } = page(group("row", [a, b, c]));
    focus.focus(a);
    focus.move("right");
    expect(fid(focus)).toBe("c");
    expect(focus.focus(b)).toBe(false);
  });
});

describe("focus traps", () => {
  it("keeps focus inside a panel, then gives it back to its opener", () => {
    const opener = item("opener", "96,100,100,40");
    const { root, focus } = page(group("row", [opener]));
    focus.focus(opener);
    const panel = group("col", [item("p1", "600,50,300,44"), item("p2", "600,100,300,44")]);
    root.appendChild(panel);
    focus.trap(panel);
    expect(focus.inScope(opener)).toBe(false);
    focus.move("down");
    expect(fid(focus)).toBe("p1");
    focus.move("down");
    expect(fid(focus)).toBe("p2");
    expect(focus.move("down")).toBe(false);
    focus.release(panel);
    expect(fid(focus)).toBe("opener");
  });

  it("marks the focused element, and only it", () => {
    const a = item("a", "0,0,10,10");
    const b = item("b", "20,0,10,10");
    const { focus } = page(group("row", [a, b]));
    focus.focus(a);
    focus.focus(b);
    expect(a.hasAttribute("data-focused")).toBe(false);
    expect(b.hasAttribute("data-focused")).toBe(true);
    focus.blur();
    expect(b.hasAttribute("data-focused")).toBe(false);
    expect(focus.current).toBeNull();
  });
});

describe("a TV's screen reader", () => {
  it("has the browser's focus follow the app's while it is on, and only then", () => {
    const a = item("a", "96,100,100,40");
    const b = item("b", "210,100,100,40");
    for (const x of [a, b]) x.tabIndex = -1;
    const { focus } = page(group("row", [a, b]));
    focus.focus(a);
    expect(document.activeElement).toBe(document.body);
    // Turned on: at once on what has focus, then on each move.
    focus.dom = true;
    expect(document.activeElement).toBe(a);
    focus.move("right");
    expect(document.activeElement).toBe(b);
    focus.dom = false;
    focus.move("left");
    expect(fid(focus)).toBe("a");
    expect(document.activeElement).toBe(b);
  });
});

describe("the remote's keys", () => {
  function setup() {
    const a = item("a", "96,100,100,40");
    const b = item("b", "210,100,100,40");
    const { focus } = page(group("row", [a, b]));
    let now = 0;
    const gate = new KeyRepeatGate(() => now);
    const input = new Input(focus, undefined, gate);
    focus.focus(a);
    return { a, b, focus, input, tick: (ms: number) => (now += ms) };
  }

  it("selects on OK's release, and long-presses after half a second", async () => {
    const { a, input } = setup();
    let selected = 0;
    let held = 0;
    setHandlers(a, { onSelect: () => selected++, onLongSelect: () => held++ });
    input.dispatch("ok", "down", false);
    input.dispatch("ok", "up", false);
    expect(selected).toBe(1);
    input.dispatch("ok", "down", false);
    await new Promise((r) => setTimeout(r, 560));
    input.dispatch("ok", "up", false);
    expect(held).toBe(1);
    expect(selected).toBe(1);
  });

  it("says when a hold starts and ends, and ends one whose key-up was lost", async () => {
    const { input, tick } = setup();
    const said: boolean[] = [];
    input.onHolding = (h) => said.push(h);
    input.dispatch("right", "down", false);
    input.dispatch("right", "down", true);
    input.dispatch("right", "down", true);
    input.dispatch("right", "up", false);
    expect(said).toEqual([true, false]);
    input.dispatch("left", "down", false);
    input.dispatch("left", "down", true);
    // No key-up: once no key has come for the gate's 500 ms, the hold ends by itself.
    tick(600);
    await new Promise((r) => setTimeout(r, 600));
    expect(said).toEqual([true, false, true, false]);
  });

  it("lets a held direction move once per 100 ms", () => {
    const { focus, input, tick } = setup();
    input.dispatch("right", "down", false);
    expect(fid(focus)).toBe("b");
    input.dispatch("left", "down", true);
    expect(fid(focus)).toBe("b");
    tick(120);
    input.dispatch("left", "down", true);
    expect(fid(focus)).toBe("a");
  });

  it("gives keys to the handlers on top first, newest first", () => {
    const { input, focus } = setup();
    const seen: string[] = [];
    input.push((key) => {
      seen.push(`old:${key}`);
      return false;
    });
    const off = input.push((key) => {
      seen.push(`new:${key}`);
      return key === "right";
    });
    input.dispatch("right", "down", false);
    expect(seen).toEqual(["new:right"]);
    expect(fid(focus)).toBe("a");
    off();
    input.dispatch("right", "down", false);
    expect(fid(focus)).toBe("b");
  });

  it("opens the menu on the menu key's release", () => {
    const { a, input } = setup();
    let menus = 0;
    setHandlers(a, { onMenu: () => menus++ });
    input.dispatch("menu", "down", false);
    expect(menus).toBe(0);
    input.dispatch("menu", "up", false);
    expect(menus).toBe(1);
  });

  it("tells when a move found nothing, so Left can open the rail", () => {
    const { input } = setup();
    const failed: string[] = [];
    input.onMoveFailed = (dir) => failed.push(dir);
    input.dispatch("left", "down", false);
    expect(failed).toEqual(["left"]);
  });
});
