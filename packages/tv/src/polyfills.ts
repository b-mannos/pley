/**
 * What the oldest TVs' engines lack (Chromium 53 on webOS 4, 56 on Tizen
 * 4) and the app or its libraries use. esbuild lowers the syntax; these
 * fill in the library functions.
 */

const ap = Array.prototype as unknown as Record<string, unknown>;
if (!ap.flat) {
  Object.defineProperty(Array.prototype, "flat", {
    configurable: true,
    writable: true,
    value: function flat(this: unknown[], depth = 1): unknown[] {
      const out: unknown[] = [];
      const walk = (list: unknown[], d: number) => {
        for (let i = 0; i < list.length; i++) {
          const v = list[i];
          if (Array.isArray(v) && d > 0) walk(v, d - 1);
          else out.push(v);
        }
      };
      walk(this, depth);
      return out;
    },
  });
}
if (!ap.flatMap) {
  Object.defineProperty(Array.prototype, "flatMap", {
    configurable: true,
    writable: true,
    value: function flatMap(this: unknown[], fn: (v: unknown, i: number, a: unknown[]) => unknown, self?: unknown): unknown[] {
      return (this.map(fn, self) as unknown as { flat(): unknown[] }).flat();
    },
  });
}

const o = Object as unknown as Record<string, unknown>;
if (!o.entries) o.entries = (x: Record<string, unknown>) => Object.keys(x).map((k) => [k, x[k]]);
if (!o.values) o.values = (x: Record<string, unknown>) => Object.keys(x).map((k) => x[k]);
if (!o.fromEntries) {
  o.fromEntries = (pairs: Iterable<[string, unknown]>) => {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Array.from(pairs)) out[k] = v;
    return out;
  };
}

const sp = String.prototype as unknown as Record<string, unknown>;
if (!sp.padStart) {
  sp.padStart = function padStart(this: string, length: number, fill = " "): string {
    let s = String(this);
    while (s.length < length) s = fill + s;
    return s.length > length && fill.length > 1 ? s.slice(s.length - length) : s;
  };
}

const pp = Promise.prototype as unknown as Record<string, unknown>;
if (!pp.finally) {
  pp.finally = function fin<T>(this: Promise<T>, fn: () => void): Promise<T> {
    return this.then(
      (v) => {
        fn();
        return v;
      },
      (e) => {
        fn();
        throw e;
      },
    );
  };
}

export {};
