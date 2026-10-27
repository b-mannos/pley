/**
 * D-pad focus, kept by the app rather than the browser: DOM focus scrolls
 * clipped containers by itself on old TV engines, and remote keys need
 * rules a browser doesn't have. Focusable elements carry `data-f`; groups
 * carry `data-fg` ("row", "col", "grid" or "free"), and decide moves
 * inside them in order (rows, columns, grids) or by geometry (free).
 * A move that leaves a group asks its edge handler, then the group above.
 * Groups with `restore` come back to the element focused last in them,
 * as the Android app's focusRestorer does.
 */

export type Dir = "left" | "right" | "up" | "down";

export interface FocusHandlers {
  onSelect?: () => void;
  /** OK held for half a second. */
  onLongSelect?: () => void;
  /** The remote's menu key, on key-up. */
  onMenu?: () => void;
  onFocus?: (el: HTMLElement) => void;
  onBlur?: () => void;
  /** Key presses the element handles itself before focus moves (a field, the jump control). */
  onKey?: (key: string, e: { repeat: boolean }) => boolean;
  /** The pointer's click, in place of `onSelect`, where the place clicked matters (the scrubber seeks there). */
  onClick?: (e: MouseEvent) => void;
}

export interface GroupOptions {
  kind: "row" | "col" | "grid" | "free";
  /** Columns of a grid. */
  columns?: number;
  /** Re-entering the group returns to the element focused last in it. */
  restore?: boolean;
  /**
   * A move would leave the group from `from`: true when handled (focus
   * stays, or went elsewhere), false to let the group above decide.
   */
  onEdge?: (dir: Dir, from: HTMLElement) => boolean;
  /** Where focus lands when it enters the group; `null` for the default rule. */
  onEnter?: (dir: Dir | null, from: DOMRect | null) => HTMLElement | null;
}

const handlers = new WeakMap<Element, FocusHandlers>();
const groups = new WeakMap<Element, GroupOptions>();
const remembered = new WeakMap<Element, HTMLElement>();

export function setHandlers(el: Element, h: FocusHandlers): void {
  handlers.set(el, h);
}

export function handlersOf(el: Element | null): FocusHandlers | undefined {
  return el ? handlers.get(el) : undefined;
}

export function setGroup(el: Element, opts: GroupOptions): void {
  groups.set(el, opts);
}

export function groupOf(el: Element): GroupOptions | undefined {
  return groups.get(el);
}

function focusable(el: Element): el is HTMLElement {
  return el.hasAttribute("data-f") && !el.hasAttribute("data-fdisabled");
}

/** The nearest group element above `el`, stopping at `root`. */
function parentGroup(el: Element, root: Element): Element | null {
  let p = el.parentElement;
  while (p && p !== root) {
    if (p.hasAttribute("data-fg")) return p;
    p = p.parentElement;
  }
  return null;
}

/** The items of a group: focusables and groups whose nearest group is this one, in document order. */
export function itemsOf(group: Element): Element[] {
  const out: Element[] = [];
  const walk = (el: Element) => {
    const children = el.children;
    for (let i = 0; i < children.length; i++) {
      const c = children[i]!;
      if (c.hasAttribute("data-fg")) {
        if (hasFocusable(c)) out.push(c);
      } else if (c.hasAttribute("data-f")) {
        if (!c.hasAttribute("data-fdisabled")) out.push(c);
      } else {
        walk(c);
      }
    }
  };
  walk(group);
  return out;
}

function hasFocusable(el: Element): boolean {
  return !!el.querySelector("[data-f]:not([data-fdisabled])");
}

function rect(el: Element): DOMRect {
  return el.getBoundingClientRect();
}

/** Android's FocusFinder, simplified: candidates in the direction, preferring those in the source's beam, nearest by 13·major² + minor². */
export function nearest(from: DOMRect, candidates: Element[], dir: Dir | null): Element | null {
  let best: Element | null = null;
  let bestScore = Infinity;
  let bestInBeam = false;
  const cx = from.left + from.width / 2;
  const cy = from.top + from.height / 2;
  for (const c of candidates) {
    const r = rect(c);
    if (!r.width && !r.height) continue;
    let major = 0;
    let minor = 0;
    let inBeam = false;
    if (dir === "left" || dir === "right") {
      major = dir === "right" ? r.left - from.right : from.left - r.right;
      if (dir === "right" ? r.right <= from.right + 0.5 && r.left < from.right - 0.5 : r.left >= from.left - 0.5 && r.right > from.left + 0.5) continue;
      if (major < -Math.min(r.width, from.width) / 2) continue;
      minor = Math.abs(r.top + r.height / 2 - cy);
      inBeam = r.bottom > from.top && r.top < from.bottom;
    } else if (dir === "up" || dir === "down") {
      major = dir === "down" ? r.top - from.bottom : from.top - r.bottom;
      if (dir === "down" ? r.bottom <= from.bottom + 0.5 && r.top < from.bottom - 0.5 : r.top >= from.top - 0.5 && r.bottom > from.top + 0.5) continue;
      if (major < -Math.min(r.height, from.height) / 2) continue;
      minor = Math.abs(r.left + r.width / 2 - cx);
      inBeam = r.right > from.left && r.left < from.right;
    } else {
      major = Math.hypot(r.left + r.width / 2 - cx, r.top + r.height / 2 - cy);
      inBeam = true;
    }
    const score = 13 * Math.max(0, major) * Math.max(0, major) + minor * minor;
    if ((inBeam && !bestInBeam) || (inBeam === bestInBeam && score < bestScore)) {
      best = c;
      bestScore = score;
      bestInBeam = inBeam;
    }
  }
  return best;
}

export class FocusManager {
  current: HTMLElement | null = null;
  /**
   * Whether the pointer (LG's Magic Remote, a mouse) gave the current
   * focus: rows, pages and lists stay where they are then, moving only to
   * bring in an element cut off at their edge (`nudge`), so what the
   * pointer is on stays under it.
   */
  byPointer = false;
  /**
   * Whether the browser's focus follows the app's: while the TV's screen
   * reader is on, which reads the element the browser has focused.
   */
  get dom(): boolean {
    return this.domFocus;
  }
  set dom(on: boolean) {
    this.domFocus = on;
    this.syncDom();
  }
  private domFocus = false;

  /** The browser's focus back on the app's (a field that typed let it go). */
  syncDom(): void {
    if (this.domFocus && this.current && this.current.isConnected) giveDomFocus(this.current);
  }
  /** Focus traps, innermost last: the page, then panels and dialogs on top of it. */
  private traps: { root: HTMLElement; opener: HTMLElement | null }[] = [];
  private readonly listeners: ((el: HTMLElement | null) => void)[] = [];

  get root(): HTMLElement | null {
    const t = this.traps[this.traps.length - 1];
    return t ? t.root : null;
  }

  /** Focus stays inside `root` until `release(root)`; the element focused before gets it back then. */
  trap(root: HTMLElement): void {
    this.traps.push({ root, opener: this.current });
  }

  release(root: HTMLElement, restore = true): void {
    const at = this.traps.map((t) => t.root).lastIndexOf(root);
    if (at < 0) return;
    const [t] = this.traps.splice(at, 1);
    if (restore && t?.opener && t.opener.isConnected && at === this.traps.length) this.focus(t.opener);
    else if (this.current && !this.current.isConnected) this.current = null;
  }

  /** The root page, under every trap: what the shell renders. */
  setPage(root: HTMLElement): void {
    if (this.traps.length) this.traps[0] = { root, opener: null };
    else this.traps.push({ root, opener: null });
  }

  onChange(listener: (el: HTMLElement | null) => void): () => void {
    this.listeners.push(listener);
    return () => {
      const i = this.listeners.indexOf(listener);
      if (i >= 0) this.listeners.splice(i, 1);
    };
  }

  /** Whether `el` is inside the innermost trap. */
  inScope(el: Element): boolean {
    const root = this.root;
    return !!root && root.contains(el);
  }

  focus(el: HTMLElement | null, byPointer = false): boolean {
    if (!el || !el.isConnected || !focusable(el)) return false;
    if (el === this.current) return true;
    this.byPointer = byPointer;
    const was = this.current;
    if (was) {
      was.removeAttribute("data-focused");
      handlers.get(was)?.onBlur?.();
    }
    this.current = el;
    el.setAttribute("data-focused", "");
    if (this.domFocus) giveDomFocus(el);
    // Every group above remembers it.
    let p = el.parentElement;
    while (p) {
      if (p.hasAttribute("data-fg")) remembered.set(p, el);
      p = p.parentElement;
    }
    handlers.get(el)?.onFocus?.(el);
    for (const l of this.listeners.slice()) l(el);
    return true;
  }

  /** Nothing focused (the page it was on went away). */
  blur(): void {
    const was = this.current;
    if (!was) return;
    this.current = null;
    was.removeAttribute("data-focused");
    handlers.get(was)?.onBlur?.();
  }

  /** The element last focused inside `group`, if it is still there. */
  rememberedIn(group: Element): HTMLElement | null {
    const el = remembered.get(group);
    return el && el.isConnected && group.contains(el) && focusable(el) ? el : null;
  }

  forget(group: Element): void {
    remembered.delete(group);
  }

  /** Focus into `target`: itself when focusable, else the group's way in. */
  enter(target: Element, dir: Dir | null, from: DOMRect | null): HTMLElement | null {
    if (!target.hasAttribute("data-fg")) return focusable(target) ? target : null;
    const opts = groups.get(target);
    const custom = opts?.onEnter?.(dir, from);
    if (custom && focusable(custom)) return custom;
    if (opts?.restore) {
      const last = this.rememberedIn(target);
      if (last) return last;
    }
    const items = itemsOf(target);
    if (!items.length) return null;
    let pick: Element | null;
    if (!from || opts?.kind === "row" || opts?.kind === "col" || opts?.kind === "grid") {
      // Entering a list: the item nearest the source along the list, else the first.
      pick = from ? nearest(from, items, null) : items[0]!;
      if (from && (opts?.kind === "row" || opts?.kind === "col")) pick = nearestOnAxis(from, items, opts.kind === "row" ? "x" : "y") ?? pick;
    } else {
      pick = nearest(from, items, dir) ?? nearest(from, items, null);
    }
    return pick ? this.enter(pick, dir, from) : null;
  }

  /** Moves focus one step; whether anything handled it. */
  move(dir: Dir): boolean {
    const root = this.root;
    const from = this.current;
    if (!root) return false;
    if (!from || !root.contains(from)) {
      const first = this.enter(root.hasAttribute("data-fg") ? root : firstGroupOrSelf(root), dir, null);
      return first ? this.focus(first) : false;
    }
    const fromRect = rect(from);
    let node: Element = from;
    for (;;) {
      const group: Element | null = parentGroup(node, root) ?? (root.hasAttribute("data-fg") ? root : null);
      const opts = group ? groups.get(group) ?? { kind: "free" as const } : { kind: "free" as const };
      const items = group ? itemsOf(group) : itemsOf(root);
      const next = step(opts, items, node, dir, fromRect);
      if (next) {
        const target = this.enter(next, dir, fromRect);
        if (target) return this.focus(target);
      }
      if (opts.onEdge?.(dir, from)) return true;
      if (!group || group === root) return false;
      node = group;
    }
  }
}

function firstGroupOrSelf(root: HTMLElement): Element {
  return root.querySelector("[data-fg]") ?? root;
}

function nearestOnAxis(from: DOMRect, items: Element[], axis: "x" | "y"): Element | null {
  const c = axis === "x" ? from.left + from.width / 2 : from.top + from.height / 2;
  let best: Element | null = null;
  let bestD = Infinity;
  for (const it of items) {
    const r = rect(it);
    const lo = axis === "x" ? r.left : r.top;
    const hi = axis === "x" ? r.right : r.bottom;
    const d = c < lo ? lo - c : c > hi ? c - hi : 0;
    if (d < bestD) {
      bestD = d;
      best = it;
    }
  }
  return best;
}

/** The item a move from `node` lands on inside its group, or `null` at the group's edge. */
function step(opts: GroupOptions, items: Element[], node: Element, dir: Dir, fromRect: DOMRect): Element | null {
  const at = items.indexOf(node);
  if (at < 0) return null;
  switch (opts.kind) {
    case "row":
      if (dir === "left") return items[at - 1] ?? null;
      if (dir === "right") return items[at + 1] ?? null;
      return null;
    case "col":
      if (dir === "up") return items[at - 1] ?? null;
      if (dir === "down") return items[at + 1] ?? null;
      return null;
    case "grid": {
      const cols = Math.max(1, opts.columns ?? 1);
      if (dir === "left") return at % cols > 0 ? items[at - 1] ?? null : null;
      if (dir === "right") return at % cols < cols - 1 ? items[at + 1] ?? null : null;
      if (dir === "up") return at - cols >= 0 ? items[at - cols] ?? null : null;
      const below = at + cols;
      if (below < items.length) return items[below] ?? null;
      // A last row shorter than the others: its last cell.
      const lastRowStart = items.length - (items.length % cols || cols);
      return at < lastRowStart ? items[items.length - 1] ?? null : null;
    }
    default: {
      const others = items.filter((i) => i !== node);
      return nearest(fromRect, others, dir);
    }
  }
}

/**
 * Where a scroller goes when the pointer focused an element in it
 * (`FocusManager.byPointer`): nowhere, unless the element is cut off at
 * the view's edge, and then only as far as brings it in whole, so the
 * pointer stays on what it points at, as in LG's own apps. `shift` is how
 * far the content is moved (a content position less `shift` is where it
 * shows); the element spans `lo` to `hi` in the content, and the view
 * shows `viewLo` to `viewHi`. The result stays within `min` and `max`.
 */
export function nudge(shift: number, lo: number, hi: number, viewLo: number, viewHi: number, min = 0, max = Infinity): number {
  let s = shift;
  if (hi - s > viewHi) s = hi - viewHi;
  if (lo - s < viewLo) s = lo - viewLo;
  return Math.max(min, Math.min(max, s));
}

/**
 * The browser's focus to `el` (a screen reader reads it), without the
 * scrolling it brings: engines before Chrome 64 have no preventScroll and
 * scroll every box up to the page to show it, and nothing here scrolls
 * but by transform, so each box goes back to its top. Elements take the
 * browser's focus only from here (their tabindex is given here), so a
 * click doesn't give it them while no reader is on.
 */
function giveDomFocus(el: HTMLElement): void {
  if (document.activeElement === el) return;
  if (!el.hasAttribute("tabindex")) el.tabIndex = -1;
  try {
    el.focus({ preventScroll: true });
  } catch {
    el.focus();
  }
  for (let n: Element | null = el.parentElement; n; n = n.parentElement) {
    if (n.scrollTop) n.scrollTop = 0;
    if (n.scrollLeft) n.scrollLeft = 0;
  }
}

/** `el`'s top in `box`'s content, by layout (what slides `box` is left out); `box` is a positioned ancestor. */
export function topIn(el: HTMLElement, box: HTMLElement): number {
  let top = 0;
  for (let n: HTMLElement | null = el; n && n !== box; n = n.offsetParent as HTMLElement | null) top += n.offsetTop;
  return top;
}
