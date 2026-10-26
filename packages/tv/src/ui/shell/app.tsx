import { useSignal, useSignalEffect } from "@preact/signals";
import { useEffect, useLayoutEffect, useMemo, useRef } from "preact/hooks";
import type { AppState } from "../../state";
import { UiContext, type Ui } from "../context";
import { FocusManager } from "../focus";
import { ImageLoader } from "../images";
import { Input } from "../input";
import { initMusic } from "../../player/music";
import { AppContext, Dialog } from "../screens/common";
import { Screen } from "../screens/screens";
import { Navigator, type Entry, type PageKey } from "./navigator";
import { currentEntry, Rail, type RailApi } from "./rail";

/** Pages without the rail: sign-in, "Who's watching?" and the player. */
function railless(key: PageKey | undefined): boolean {
  return !key || key.name === "servers" || key.name === "link" || key.name === "profiles" || key.name === "player";
}

export interface ShellProps {
  state: AppState;
  /** Pixels per canvas px on this screen. */
  scale: number;
}

/**
 * The app: one page at a time from the back stack, the rail beside the
 * pages that have it, the screen-change curtain, toasts, and the remote's
 * Back by the Android app's order: what is open on top first, then the
 * rail exits, a pushed page pops, a root page opens the rail.
 */
export function App(p: ShellProps) {
  const state = p.state;
  const focus = useMemo(() => new FocusManager(), []);
  const input = useMemo(() => new Input(focus, (e) => state.platform.mapKey(e)), []);
  const nav = useMemo(() => new Navigator(), []);
  useMemo(() => initMusic(state), []);
  const rail = useRef<RailApi | null>(null);
  const toast = useSignal<{ text: string; error: boolean; id: number } | null>(null);
  const curtain = useSignal(0);
  const exiting = useSignal(false);
  const pageRoot = useRef<HTMLDivElement>(null);
  const api = state.api.value;
  const tier = useMemo(() => state.platform.tier(), []);
  const images = useMemo(
    () => (api ? new ImageLoader(api, { concurrency: tier === "low" ? 4 : 6, cap: tier === "low" ? 1280 : p.scale * 960 >= 1900 ? 1920 : 1280 }) : null),
    [api],
  );
  const reduced = state.settings.value.reduceMotion;
  /** When the page on top was shown, and whether it got its remembered element back. */
  const shownAt = useRef(0);
  const restored = useRef(new Set<number>());

  // The first page, from what the TV kept: no network before the first frame.
  if (!nav.stack.value.length) {
    const start = state.start();
    nav.restart(start.page === "link" ? { name: "link", server: start.server } : { name: start.page });
  }

  // A sign-in the server ended: back to signing in.
  useSignalEffect(() => {
    if (state.signedOut.value && state.server.value) {
      const top = nav.top?.key;
      if (top?.name !== "link" && top?.name !== "servers") nav.restart({ name: "link", server: state.server.value });
    }
  });

  const toRail = () => !railless(nav.root?.key) && nav.top?.key.name !== "player" && !!rail.current?.focusCurrent();
  const showToast = (text: string, error = false) => (toast.value = { text, error, id: Date.now() });
  const claim = (el: HTMLElement, fid: string | null, initial: boolean) => {
    const entry = nav.top;
    if (!entry || !focus.inScope(el)) return;
    // The person moved to the rail while the page loaded: it keeps focus.
    if (rail.current?.hasFocus() && focus.root !== el.closest(".panel-root")) return;
    const current = focus.current;
    const scopeHasFocus = !!current && current.isConnected && focus.inScope(current);
    const touched = input.lastKeyAt > shownAt.current;
    const remembering = !!entry.memory && !touched && !restored.current.has(entry.id);
    if (remembering && fid === entry.memory) {
      restored.current.add(entry.id);
      focus.focus(el);
      return;
    }
    if (!initial || scopeHasFocus) return;
    if (remembering) {
      // Something else is remembered: it gets a moment to come back first.
      setTimeout(() => {
        const now = focus.current;
        if (!(now && now.isConnected && focus.inScope(now)) && el.isConnected) focus.focus(el);
      }, 60);
      return;
    }
    focus.focus(el);
  };
  const ui: Ui = {
    input,
    focus,
    nav,
    api: api as Ui["api"],
    images: images as Ui["images"],
    scale: p.scale,
    tier,
    reducedMotion: reduced,
    toast: showToast,
    toRail,
    claim,
  };

  // Keys, Back last of all.
  useEffect(() => {
    const detach = input.attach(document);
    input.onMoveFailed = (dir) => {
      // Left with nothing further left, anywhere on a page, goes to the rail.
      if (dir === "left" && !rail.current?.hasFocus() && focus.root === pageRoot.current) toRail();
    };
    const exit = () => {
      if (state.platform.asksBeforeExit) exiting.value = true;
      else state.platform.exit();
    };
    const off = input.push((key, e) => {
      if (key !== "back") return false;
      if (e.type !== "down" || e.repeat) return true;
      if (rail.current?.hasFocus()) {
        exit();
        return true;
      }
      if (nav.pop()) return true;
      if (toRail()) return true;
      exit();
      return true;
    });
    state.platform.registerKeys?.();
    return () => {
      off();
      detach();
    };
  }, []);

  // A held direction key: rows and tiles move without animating.
  useEffect(() => {
    const t = setInterval(() => {
      const holding = input.gate.holding;
      const root = document.getElementById("app");
      if (root && root.classList.contains("holding") !== holding) root.classList.toggle("holding", holding);
    }, 50);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    document.getElementById("app")?.classList.toggle("reduced", reduced);
  }, [reduced]);

  // Page focus memory: what the page focused last, so Back lands on it.
  useEffect(
    () =>
      focus.onChange((el) => {
        const entry = nav.top;
        const root = pageRoot.current;
        if (!entry || !el || !root || !root.contains(el)) return;
        entry.memory = el.getAttribute("data-fid");
      }),
    [],
  );

  // Who is watching and the libraries, once signed in; the events stream while the app shows.
  useEffect(() => {
    if (api && state.signedIn) void state.loadAccount().catch(() => undefined);
  }, [api]);
  useEffect(() => {
    state.platform.onVisibility((visible) => {
      state.visible.value = visible;
      state.listen(visible);
    });
  }, []);
  useEffect(() => {
    state.listen(state.visible.value);
    return () => state.listen(false);
  }, [api, state.signedOut.value]);

  const stack = nav.stack.value;
  const top = stack[stack.length - 1] as Entry | undefined;
  const lastTop = useRef<number | null>(null);
  /**
   * The page's root, as soon as it exists: before the page's own elements
   * claim focus (refs are set before any layout effect runs).
   */
  const attachPage = (el: HTMLDivElement | null) => {
    pageRoot.current = el;
    if (!el) return;
    focus.setPage(el);
    const id = Number(el.getAttribute("data-entry"));
    if (lastTop.current === id) return;
    shownAt.current = Date.now();
    restored.current.delete(id);
    // Focus left over from the page before is no use here.
    if (focus.current && !el.contains(focus.current)) focus.blur();
  };
  useLayoutEffect(() => {
    if (!top) return;
    if (lastTop.current !== null && lastTop.current !== top.id && !reduced) curtain.value++;
    lastTop.current = top.id;
  }, [top?.id]);

  useEffect(() => {
    if (!toast.value) return;
    const t = setTimeout(() => (toast.value = null), toast.value.error ? 6000 : 3500);
    return () => clearTimeout(t);
  }, [toast.value?.id]);

  const showRail = !railless(stack[0]?.key) && top?.key.name !== "player" && !!api;
  return (
    <AppContext.Provider value={state}>
      <UiContext.Provider value={ui}>
        <div ref={attachPage} class="screen" key={top?.id} data-entry={top?.id}>
          {top && <Screen entry={top} state={state} />}
        </div>
        {showRail && (
          <Rail
            libraries={state.libraries.value}
            me={state.me.value}
            current={currentEntry(stack[0]?.key ?? null, state.libraries.value)}
            railRef={(r) => (rail.current = r)}
          />
        )}
        {exiting.value && (
          <Dialog
            title="Exit pley?"
            onDismiss={() => (exiting.value = false)}
            buttons={[
              { label: "Cancel", onSelect: () => (exiting.value = false) },
              { label: "Exit", initial: true, onSelect: () => state.platform.exit() },
            ]}
          />
        )}
        {curtain.value > 0 && <div key={curtain.value} class="curtain fade" />}
        {toast.value && (
          <div class={`toast in t-label${toast.value.error ? " error" : ""}`} role="status">
            {toast.value.text}
          </div>
        )}
      </UiContext.Provider>
    </AppContext.Provider>
  );
}
