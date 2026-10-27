import { Component, createElement, type ComponentType, type FunctionComponent } from "preact";

type Props = Record<string, unknown>;
type Call = (...args: unknown[]) => unknown;

/**
 * `C`, drawn again only when what it shows changes: its props compared one
 * by one (arrays by their elements, functions by whether there is one), the
 * `ignored` ones not at all. A page that draws again on every move (a row's
 * tiles, a grid's cells) then costs the TV a comparison per tile, not a
 * render. Callbacks stay current all the same: `C` gets stable ones that
 * call the newest props' own, which Preact keeps up to date on a component
 * it skipped.
 */
export function memo<P extends object>(C: FunctionComponent<P>, ignored: string[] = []): ComponentType<P> {
  class Memo extends Component<P> {
    private readonly calls: Record<string, Call> = {};

    override shouldComponentUpdate(next: P): boolean {
      return differs(this.props as Props, next as Props, ignored);
    }

    override render(props: P) {
      const out: Props = {};
      for (const k in props) {
        const v = (props as Props)[k];
        out[k] = typeof v === "function" ? this.call(k) : v;
      }
      return createElement(C, out as P);
    }

    private call(k: string): Call {
      let f = this.calls[k];
      if (!f) {
        f = (...args) => ((this.props as Props)[k] as Call | undefined)?.(...args);
        this.calls[k] = f;
      }
      return f;
    }
  }
  return Memo as unknown as ComponentType<P>;
}

/** Whether `a` and `b` show something different (see `memo`). */
export function differs(a: Props, b: Props, ignored: string[] = []): boolean {
  for (const k in a) if (!(k in b) && a[k] !== undefined && ignored.indexOf(k) < 0) return true;
  for (const k in b) {
    if (ignored.indexOf(k) >= 0) continue;
    const x = a[k];
    const y = b[k];
    if (x === y) continue;
    if (typeof x === "function" && typeof y === "function") continue;
    if (Array.isArray(x) && Array.isArray(y) && x.length === y.length && x.every((v, i) => v === y[i])) continue;
    return true;
  }
  return false;
}
