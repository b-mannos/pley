import type { ComponentChildren } from "preact";
import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import { useUi } from "../context";
import { G } from "./focusable";
import { TextButton } from "./buttons";

/**
 * Keeps the focused element of a tall column in view, as the Android
 * app's root scrolling does: its top at 30 % of the box, the column's
 * ends never passed. Moves only by transform.
 */
export function useScrollToFocus(inner: { current: HTMLElement | null }, height: number) {
  const ui = useUi();
  const [shift, setShift] = useState(0);
  useEffect(
    () =>
      ui.focus.onChange((el) => {
        const b = inner.current;
        if (!el || !b || !b.contains(el)) return;
        let top = 0;
        for (let n: HTMLElement | null = el; n && n !== b; n = n.offsetParent as HTMLElement | null) top += n.offsetTop;
        const content = b.offsetHeight;
        const want = Math.max(0, Math.min(top - height * 0.3, content - height));
        setShift(want > 0 ? want : 0);
      }),
    [height],
  );
  return shift;
}

/**
 * A side panel (TvSidePanel): 360 wide at the right, sliding in over a
 * scrim, focus trapped inside; Back closes it and focus goes back to what
 * opened it.
 */
export function SidePanel(p: { onClose: () => void; children: ComponentChildren; wide?: boolean; label?: string }) {
  const ui = useUi();
  const root = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLDivElement>(null);
  const [shown, setShown] = useState(false);
  useLayoutEffect(() => {
    const el = root.current!;
    ui.focus.trap(el);
    const off = ui.input.push((key, e) => {
      if (key === "back") {
        if (e.type === "down") p.onClose();
        return true;
      }
      return false;
    });
    const raf = requestAnimationFrame(() => setShown(true));
    return () => {
      cancelAnimationFrame(raf);
      off();
      ui.focus.release(el);
    };
  }, []);
  const shift = useScrollToFocus(inner, 540 - 54);
  return (
    <div ref={root} class="panel-root" style="position:absolute;left:0;top:0;right:0;bottom:0;z-index:30" aria-label={p.label}>
      <div class={`scrim${shown ? " in" : ""}`} />
      <div class={`side-panel${p.wide ? " wide" : ""}${shown ? " in" : ""}`}>
        <div ref={inner} class="panel-scroll" style={`transform:translateY(${-shift}px)`}>
          <G kind="col">{p.children}</G>
        </div>
      </div>
    </div>
  );
}

export function PanelTitle(p: { children: ComponentChildren }) {
  return <div class="panel-title t-title lines-2">{p.children}</div>;
}

export function PanelHeading(p: { children: ComponentChildren }) {
  return <div class="panel-heading t-title">{p.children}</div>;
}

/** A dialog of text with Close (TvTextDialog). */
export function TextDialog(p: { title: string; text: string; onClose: () => void }) {
  const ui = useUi();
  const root = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = root.current!;
    ui.focus.trap(el);
    const off = ui.input.push((key, e) => {
      if (key === "back") {
        if (e.type === "down") p.onClose();
        return true;
      }
      return false;
    });
    return () => {
      off();
      ui.focus.release(el);
    };
  }, []);
  return (
    <div ref={root} style="position:absolute;left:0;top:0;right:0;bottom:0;z-index:40">
      <div class="scrim in" />
      <div class="dialog">
        <div class="t-title">{p.title}</div>
        <div class="t-body-large" style="margin-top:16px;color:rgba(233,235,239,0.85);white-space:pre-line">
          {p.text}
        </div>
        <G kind="row" style="display:flex;justify-content:flex-end;margin-top:16px">
          <TextButton label="Close" initial onSelect={p.onClose} />
        </G>
      </div>
    </div>
  );
}
