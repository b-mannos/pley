import { FocusManager, handlersOf } from "./focus";
import { isDirection, KeyRepeatGate, standardKey, type Key } from "./keys";

/** OK held this long is a long press (Android's ViewConfiguration, older versions). */
export const LONG_PRESS_MS = 500;

/**
 * A screen's or overlay's own keys, before focus moves: the player's
 * keys, Back on a panel. Return true when handled. The newest handler
 * runs first, as the Android app's back handlers do.
 */
export type KeyHandler = (key: Key, e: { repeat: boolean; type: "down" | "up" }) => boolean;

/**
 * The remote's keys for the whole app: mapped by the platform, held
 * direction keys gated to one move per 100 ms, OK's tap and long press
 * told apart on key-up, the menu key once on key-up, then the handlers
 * on top, then focus.
 */
export class Input {
  readonly focus: FocusManager;
  readonly gate: KeyRepeatGate;
  private readonly handlers: KeyHandler[] = [];
  private readonly mapKey: (e: KeyboardEvent) => Key | null;
  private okDownAt = 0;
  private okTimer: ReturnType<typeof setTimeout> | null = null;
  private okLong = false;
  private okTarget: HTMLElement | null = null;
  private menuDown = false;
  /** While a text field types, keys go to it. */
  typing: HTMLElement | null = null;
  /** When a key was last pressed (Date.now()): pages that have been touched keep the focus people gave them. */
  lastKeyAt = 0;
  /** A direction nothing on the page took (Left with nothing further left goes to the rail). */
  onMoveFailed: ((dir: "left" | "right" | "up" | "down") => void) | null = null;
  /** A held direction key began or ended: rows and tiles move without animating meanwhile. */
  onHolding: ((holding: boolean) => void) | null = null;
  private wasHolding = false;
  private holdTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(focus: FocusManager, mapKey: (e: KeyboardEvent) => Key | null = standardKey, gate = new KeyRepeatGate()) {
    this.focus = focus;
    this.mapKey = mapKey;
    this.gate = gate;
  }

  /** Adds `handler` on top; returns its removal. */
  push(handler: KeyHandler): () => void {
    this.handlers.push(handler);
    return () => {
      const i = this.handlers.lastIndexOf(handler);
      if (i >= 0) this.handlers.splice(i, 1);
    };
  }

  attach(target: Window | Document = document): () => void {
    const down = (e: Event) => this.onKey(e as KeyboardEvent, "down");
    const up = (e: Event) => this.onKey(e as KeyboardEvent, "up");
    target.addEventListener("keydown", down, true);
    target.addEventListener("keyup", up, true);
    return () => {
      target.removeEventListener("keydown", down, true);
      target.removeEventListener("keyup", up, true);
    };
  }

  private onKey(e: KeyboardEvent, type: "down" | "up"): void {
    const key = this.mapKey(e);
    if (key === null) return;
    if (this.typing) {
      // A field types: only Back, OK, Up and Down are the app's.
      if (key !== "back" && key !== "ok" && key !== "up" && key !== "down") return;
    }
    e.preventDefault();
    e.stopPropagation();
    this.dispatch(key, type, e.repeat);
  }

  /** One key event, already mapped (tests call this). */
  dispatch(key: Key, type: "down" | "up", repeat: boolean): void {
    if (type === "down") this.lastKeyAt = Date.now();
    if (isDirection(key)) {
      const admitted = this.gate.admit(type, repeat);
      this.heldNow();
      if (!admitted) return;
    }
    const event = { repeat, type };
    // The focused element's own keys first (a field, the jump control, the sync row).
    const own = handlersOf(this.focus.current)?.onKey;
    if (own && own(key + (type === "up" ? ":up" : ""), event)) return;
    for (let i = this.handlers.length - 1; i >= 0; i--) {
      if (this.handlers[i]!(key, event)) return;
    }
    if (key === "ok") {
      this.ok(type, repeat);
      return;
    }
    if (key === "menu") {
      if (type === "down") this.menuDown = true;
      else if (this.menuDown) {
        this.menuDown = false;
        handlersOf(this.focus.current)?.onMenu?.();
      }
      return;
    }
    if (type === "down" && isDirection(key) && !this.focus.move(key)) this.onMoveFailed?.(key);
  }

  /**
   * Tells `onHolding` when the gate's hold starts and ends, as keys come
   * rather than on a timer; a key-up lost on its way ends it once no key
   * has come for the gate's LOST_KEY_UP.
   */
  private heldNow(): void {
    const holding = this.gate.holding;
    if (this.holdTimer !== null) clearTimeout(this.holdTimer);
    this.holdTimer = null;
    if (holding) {
      this.holdTimer = setTimeout(() => {
        this.holdTimer = null;
        this.gate.rested();
        this.heldNow();
      }, KeyRepeatGate.LOST_KEY_UP + 50);
    }
    if (holding !== this.wasHolding) {
      this.wasHolding = holding;
      this.onHolding?.(holding);
    }
  }

  private ok(type: "down" | "up", repeat: boolean): void {
    if (type === "down") {
      if (repeat) return;
      this.okDownAt = Date.now();
      this.okLong = false;
      this.okTarget = this.focus.current;
      const target = this.okTarget;
      if (target) target.setAttribute("data-pressed", "");
      const long = handlersOf(target)?.onLongSelect;
      if (long) {
        this.okTimer = setTimeout(() => {
          this.okTimer = null;
          this.okLong = true;
          target?.removeAttribute("data-pressed");
          long();
        }, LONG_PRESS_MS);
      }
      return;
    }
    if (this.okTimer !== null) clearTimeout(this.okTimer);
    this.okTimer = null;
    const target = this.okTarget;
    this.okTarget = null;
    target?.removeAttribute("data-pressed");
    if (this.okLong || !target || target !== this.focus.current || !this.okDownAt) return;
    this.okDownAt = 0;
    handlersOf(target)?.onSelect?.();
  }
}
