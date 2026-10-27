import { FocusManager, handlersOf } from "./focus";
import { isDirection, KeyRepeatGate, standardKey, type Key } from "./keys";

/** OK held this long is a long press (Android's ViewConfiguration, older versions). */
export const LONG_PRESS_MS = 500;
/** One OK press that webOS sends as the key and as a click: the two come this close. */
const TWIN_MS = 400;
/** An OK key down longer than this lost its key-up: clicks count again. */
const OK_HELD_MAX_MS = 10_000;
/** Wheel notches closer than this are a run: a held key. */
const WHEEL_RUN_MS = 300;
/** The pointer counts as showing this long after it last moved (webOS hides it a few seconds after). */
const POINTER_SHOWN_MS = 3_000;

/** The focusable element an event happened in, unless it is disabled. */
function focusableAt(target: EventTarget | null): HTMLElement | null {
  const t = target as Element | null;
  const el = t && t.closest ? t.closest<HTMLElement>("[data-f]") : null;
  return el && !el.hasAttribute("data-fdisabled") ? el : null;
}

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
  /**
   * Whether the mouse event being handled moved the pointer (LG's Magic
   * Remote, a mouse): what the page slides under a still pointer doesn't
   * take focus from the keys.
   */
  pointerMoved = false;
  private pointerX = NaN;
  private pointerY = NaN;
  private pressed: HTMLElement | null = null;
  /** When the pointer last moved (0: the keys took over since), and the focusable element it moved onto. */
  private pointerAt = 0;
  private pointerOver: HTMLElement | null = null;
  /** The pointer moved onto something outside where focus may go now (the rail while a page has it). */
  onPointOutside: ((el: HTMLElement) => void) | null = null;
  // One OK press may come as the key and as a click (webOS, the pointer showing): the second does nothing.
  private okHeld = false;
  private okPressAt = 0;
  private okReleaseAt = 0;
  private okSwallowed = false;
  private clickAt = 0;
  private wheelAt = 0;
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
    const move = (e: Event) => this.onPointer(e as MouseEvent);
    const press = (e: Event) => this.onPress(e as MouseEvent);
    const click = (e: Event) => {
      const el = focusableAt(e.target);
      if (el && this.focus.inScope(el)) this.click(el, e as MouseEvent);
    };
    const wheel = (e: Event) => this.onWheel(e as WheelEvent);
    target.addEventListener("keydown", down, true);
    target.addEventListener("keyup", up, true);
    target.addEventListener("mousemove", move, true);
    target.addEventListener("mousedown", press, true);
    target.addEventListener("mouseup", press, true);
    target.addEventListener("click", click, true);
    target.addEventListener("wheel", wheel, { capture: true, passive: true });
    return () => {
      target.removeEventListener("keydown", down, true);
      target.removeEventListener("keyup", up, true);
      target.removeEventListener("mousemove", move, true);
      target.removeEventListener("mousedown", press, true);
      target.removeEventListener("mouseup", press, true);
      target.removeEventListener("click", click, true);
      target.removeEventListener("wheel", wheel, true);
    };
  }

  /** The pointer moved: what it moved onto takes focus, by pointer (see `FocusManager.byPointer`). */
  private onPointer(e: MouseEvent): void {
    this.pointerMoved = e.clientX !== this.pointerX || e.clientY !== this.pointerY;
    this.pointerX = e.clientX;
    this.pointerY = e.clientY;
    if (!this.pointerMoved) return;
    const el = focusableAt(e.target);
    this.pointerAt = Date.now();
    this.pointerOver = el;
    // While a field types, focus stays on it.
    if (!el || el === this.focus.current || this.typing) return;
    if (this.focus.inScope(el)) this.focus.focus(el, true);
    else this.onPointOutside?.(el);
  }

  /** A press shows on what it is on, as OK's does. */
  private onPress(e: MouseEvent): void {
    if (this.pressed) this.pressed.removeAttribute("data-pressed");
    this.pressed = null;
    if (e.type !== "mousedown") return;
    const el = focusableAt(e.target);
    if (!el || !this.focus.inScope(el)) return;
    el.setAttribute("data-pressed", "");
    this.pressed = el;
  }

  /**
   * The Magic Remote's wheel (a mouse's) moves focus up and down as the
   * keys do, a notch a step; notches in a quick run are a held key, one
   * move every 100 ms, without animating.
   */
  private onWheel(e: WheelEvent): void {
    if (!e.deltaY || this.typing) return;
    const now = Date.now();
    if (now - this.wheelAt < KeyRepeatGate.MOVE_INTERVAL) return;
    const repeat = now - this.wheelAt < WHEEL_RUN_MS;
    this.wheelAt = now;
    this.dispatch(e.deltaY > 0 ? "down" : "up", "down", repeat);
  }

  /**
   * A click on `el` (the pointer) focuses and selects it, unless it is the
   * click half of an OK press the key made or is making: with the Magic
   * Remote's pointer showing, webOS may send one press both ways, and the
   * key's select may already have opened a page whose button is now under
   * the pointer.
   */
  click(el: HTMLElement, e?: MouseEvent): void {
    const now = Date.now();
    if ((this.okHeld && now - this.okPressAt < OK_HELD_MAX_MS) || now - this.okReleaseAt < TWIN_MS) {
      this.okReleaseAt = 0;
      return;
    }
    this.clickAt = now;
    this.focus.focus(el, true);
    const h = handlersOf(el);
    if (h?.onClick && e) h.onClick(e);
    else h?.onSelect?.();
  }

  /**
   * Whether an OK key event belongs to the pointer and does nothing as a
   * key: the key half of a press whose click came first, or OK while the
   * pointer shows over nothing that takes focus (OK clicks where the
   * pointer is: on a panel's scrim it closes the panel, not the row
   * focused in it).
   */
  private okTwin(type: "down" | "up", repeat: boolean): boolean {
    const now = Date.now();
    if (type === "down") {
      if (repeat) return this.okSwallowed;
      const pointing = this.pointerAt > 0 && now - this.pointerAt < POINTER_SHOWN_MS;
      this.okSwallowed = now - this.clickAt < TWIN_MS || (pointing && !this.pointerOver);
      this.clickAt = 0;
      if (!this.okSwallowed) {
        this.okHeld = true;
        this.okPressAt = now;
      }
      return this.okSwallowed;
    }
    const swallowed = this.okSwallowed;
    this.okSwallowed = false;
    if (!swallowed) {
      this.okHeld = false;
      this.okReleaseAt = now;
    }
    return swallowed;
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
    if (key === "ok" && this.okTwin(type, repeat)) return;
    if (isDirection(key)) {
      // The keys take over from the pointer (webOS hides it).
      if (type === "down") this.pointerAt = 0;
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
