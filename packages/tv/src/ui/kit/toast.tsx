import { useEffect, useState } from "preact/hooks";

export interface ToastMessage {
  text: string;
  error: boolean;
  /** A new one (the same words again included) shows anew. */
  id: number;
}

/**
 * A toast at the top right (TvToastHost): it fades in and out over 150 ms
 * (reveal), keeping its words while it fades out; another toast takes its
 * place without fading. `low` puts it under the player's top bar.
 */
export function Toast(p: { toast: ToastMessage | null; low?: boolean }) {
  const [shown, setShown] = useState<ToastMessage | null>(p.toast);
  const [visible, setVisible] = useState(false);
  useEffect(() => {
    if (p.toast) {
      setShown(p.toast);
      // A frame at nothing first, so that it fades in.
      const t = setTimeout(() => setVisible(true), 16);
      return () => clearTimeout(t);
    }
    setVisible(false);
    const t = setTimeout(() => setShown(null), 150);
    return () => clearTimeout(t);
  }, [p.toast?.id ?? null]);
  if (!shown) return null;
  return (
    <div class={`toast t-label${visible ? " in" : ""}${p.low ? " low" : ""}${shown.error ? " error" : ""}`} role="status">
      {shown.text}
    </div>
  );
}
