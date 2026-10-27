import { useLayoutEffect, useRef } from "preact/hooks";
import { useUi } from "../context";

/**
 * A list's items moved to new places slide there: each child of `box`
 * with a `data-key` sits at its `data-place` (its transform), and one
 * whose place changed slides from where it was. The function returned,
 * called just before a change, makes items in the middle of a slide go
 * on from where they show (keys pressed in quick succession). A child the
 * list moved in the page loses its transition, so each starts from its
 * old place by hand.
 */
export function useSlides(box: { current: HTMLElement | null }): () => void {
  const ui = useUi();
  const placed = useRef(new Map<string, string>());
  const from = useRef<Map<string, string> | null>(null);
  useLayoutEffect(() => {
    const el = box.current;
    const start = from.current;
    from.current = null;
    if (!el) return;
    const next = new Map<string, string>();
    const moving: [HTMLElement, string, string][] = [];
    for (let i = 0; i < el.children.length; i++) {
      const c = el.children[i] as HTMLElement;
      const key = c.getAttribute("data-key");
      const place = c.getAttribute("data-place");
      if (key === null || place === null) continue;
      next.set(key, place);
      const was = placed.current.get(key);
      if (was !== undefined && was !== place) moving.push([c, start?.get(key) ?? was, place]);
    }
    placed.current = next;
    if (!moving.length) return;
    for (const [c, s] of moving) {
      c.style.transition = "none";
      c.style.transform = s;
    }
    void el.offsetHeight;
    for (const [c, , e] of moving) {
      c.style.transition = "";
      c.style.transform = e;
    }
    // A screen reader's element keeps the browser's focus, moved in the page as it was.
    ui.focus.syncDom();
  });
  return () => {
    const el = box.current;
    if (!el) return;
    const now = new Map<string, string>();
    for (let i = 0; i < el.children.length; i++) {
      const c = el.children[i] as HTMLElement;
      const key = c.getAttribute("data-key");
      const t = key === null ? "none" : getComputedStyle(c).transform;
      if (key !== null && t && t !== "none") now.set(key, t);
    }
    from.current = now;
  };
}
