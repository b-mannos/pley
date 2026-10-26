/**
 * Calls to the TV's system services (Luna), as LG's webOSTV.js makes them:
 * through the PalmServiceBridge the web engine gives packaged apps. No
 * library: the few calls the app makes are written out here.
 */

interface Bridge {
  onservicecallback: ((message: string) => void) | null;
  call(uri: string, params: string): void;
  cancel(): void;
}

declare global {
  interface Window {
    PalmServiceBridge?: new () => Bridge;
    PalmSystem?: {
      deviceInfo?: string;
      activate?: () => void;
      platformBack?: () => void;
      setWindowProperty?: (name: string, value: string) => void;
    };
    webOSSystem?: {
      deviceInfo?: string;
      activate?: () => void;
      platformBack?: () => void;
    };
  }
}

export interface LunaAnswer {
  returnValue?: boolean;
  errorCode?: number | string;
  errorText?: string;
  [key: string]: unknown;
}

/** Whether the app runs on a TV (or LG's simulator), with Luna at hand. */
export function hasLuna(): boolean {
  return typeof window !== "undefined" && typeof window.PalmServiceBridge === "function";
}

/** One call; its answer, or an error after `timeoutMs` or a refusal. */
export function luna(uri: string, params: Record<string, unknown> = {}, timeoutMs = 3_000): Promise<LunaAnswer> {
  return new Promise((resolve, reject) => {
    if (!hasLuna()) {
      reject(new Error("no Luna bridge"));
      return;
    }
    const bridge = new window.PalmServiceBridge!();
    let done = false;
    const finish = () => {
      done = true;
      clearTimeout(timer);
      try {
        bridge.cancel();
      } catch {
        // Already gone.
      }
    };
    const timer = setTimeout(() => {
      if (done) return;
      finish();
      reject(new Error(`${uri}: no answer`));
    }, timeoutMs);
    bridge.onservicecallback = (message) => {
      if (done) return;
      finish();
      let answer: LunaAnswer;
      try {
        answer = JSON.parse(message) as LunaAnswer;
      } catch {
        reject(new Error(`${uri}: unreadable answer`));
        return;
      }
      if (answer.returnValue === false) reject(new Error(`${uri}: ${answer.errorText ?? answer.errorCode ?? "refused"}`));
      else resolve(answer);
    };
    bridge.call(uri, JSON.stringify(params));
  });
}

/** A subscription: every answer until the returned function is called. */
export function lunaSubscribe(uri: string, params: Record<string, unknown>, onAnswer: (a: LunaAnswer) => void): () => void {
  if (!hasLuna()) return () => {};
  const bridge = new window.PalmServiceBridge!();
  bridge.onservicecallback = (message) => {
    try {
      onAnswer(JSON.parse(message) as LunaAnswer);
    } catch {
      // Ignored.
    }
  };
  bridge.call(uri, JSON.stringify({ ...params, subscribe: true }));
  return () => {
    try {
      bridge.cancel();
    } catch {
      // Already gone.
    }
  };
}
