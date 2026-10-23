/** A shuffled copy of `list` (Fisher–Yates); `random` returns [0, 1). */
export function shuffled<T>(list: readonly T[], random: () => number = Math.random): T[] {
  const out = list.slice();
  for (let i = out.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1));
    // Both indexes are in range.
    const a = out[i] as T;
    out[i] = out[j] as T;
    out[j] = a;
  }
  return out;
}
