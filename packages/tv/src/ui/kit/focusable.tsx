import type { ComponentChildren, CSSProperties } from "preact";
import { useLayoutEffect, useRef } from "preact/hooks";
import { setGroup, setHandlers, type Dir, type FocusHandlers, type GroupOptions } from "../focus";
import { useUi } from "../context";

interface FProps extends FocusHandlers {
  class?: string;
  style?: CSSProperties | string;
  /** The page's memory key ("play", "grid/42"): Back lands on it again. */
  fid?: string;
  /** Takes focus when the page opens, unless the page remembers another element. */
  initial?: boolean;
  disabled?: boolean;
  children?: ComponentChildren;
  /** Accessible name. */
  label?: string;
  elRef?: (el: HTMLDivElement | null) => void;
}

/**
 * Something the remote focuses: a tile, a button, a row of a panel. The
 * pointer (LG's Magic Remote, a mouse) focuses what it moves onto and
 * selects what it clicks, through Input's listeners on the document.
 */
export function F(props: FProps) {
  const ref = useRef<HTMLDivElement>(null);
  const ui = useUi();
  const { onSelect, onLongSelect, onMenu, onFocus, onBlur, onKey, onClick } = props;
  useLayoutEffect(() => {
    const el = ref.current;
    if (el) setHandlers(el, { onSelect, onLongSelect, onMenu, onFocus, onBlur, onKey, onClick });
  });
  useLayoutEffect(() => {
    const el = ref.current;
    props.elRef?.(el);
    if (el && !props.disabled) ui.claim(el, props.fid ?? null, !!props.initial);
    return () => props.elRef?.(null);
  }, [props.fid, props.initial, props.disabled]);
  return (
    <div
      ref={ref}
      class={props.class}
      style={props.style}
      data-f=""
      data-fid={props.fid}
      data-fdisabled={props.disabled ? "" : undefined}
      aria-label={props.label}
      role="button"
    >
      {props.children}
    </div>
  );
}

interface GProps extends Omit<GroupOptions, "kind"> {
  kind: GroupOptions["kind"];
  class?: string;
  style?: CSSProperties | string;
  children?: ComponentChildren;
  elRef?: (el: HTMLDivElement | null) => void;
}

/** A focus group: a row, a column, a grid, or free-form buttons. */
export function G(props: GProps) {
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el) setGroup(el, { kind: props.kind, columns: props.columns, restore: props.restore, onEdge: props.onEdge, onEnter: props.onEnter });
  });
  useLayoutEffect(() => {
    props.elRef?.(ref.current);
    return () => props.elRef?.(null);
  }, []);
  return (
    <div ref={ref} class={props.class} style={props.style} data-fg={props.kind}>
      {props.children}
    </div>
  );
}

/** Edge handlers the Android app uses everywhere: Left to the rail, Right stops. */
export function railOnLeft(toRail: () => boolean, stopRight = true): (dir: Dir) => boolean {
  return (dir) => {
    if (dir === "left") return toRail();
    if (dir === "right") return stopRight;
    return false;
  };
}

/** A box that says when focus comes into it (a page's section that isn't a row). */
export function FocusWatch(p: { onFocus: () => void; class?: string; children?: ComponentChildren }) {
  const ui = useUi();
  const box = useRef<HTMLDivElement>(null);
  const on = useRef(p.onFocus);
  on.current = p.onFocus;
  useLayoutEffect(
    () =>
      ui.focus.onChange((el) => {
        if (el && box.current && box.current.contains(el)) on.current();
      }),
    [],
  );
  return (
    <div ref={box} class={p.class}>
      {p.children}
    </div>
  );
}
