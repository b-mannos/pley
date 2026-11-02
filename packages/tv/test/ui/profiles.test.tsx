import { render } from "preact";
import { act } from "preact/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import type { HouseholdProfile } from "../../src/api/types";
import type { KeyValue } from "../../src/auth/tokens";
import type { Platform } from "../../src/platform";
import { AppState } from "../../src/state";
import { UiContext, type Ui } from "../../src/ui/context";
import { FocusManager } from "../../src/ui/focus";
import { Input } from "../../src/ui/input";
import { AppContext } from "../../src/ui/screens/common";
import { ProfilesScreen } from "../../src/ui/screens/onboarding/profiles";

/** "Who's watching?" as a start opens it: on the pad of the profile the TV was left on, when that comes first. */

const SERVER = "http://192.168.1.20:7539";

const household = [
  { id: 1, name: "Anna", avatar_url: null, avatar_color: null, current: true, needs: "none" },
  { id: 2, name: "Sam", avatar_url: null, avatar_color: null, current: false, needs: "pin" },
] as HouseholdProfile[];

function memoryStore(): KeyValue {
  const kept = new Map<string, string>();
  return {
    get: (k) => kept.get(k) ?? null,
    set: (k, v) => void kept.set(k, v),
    remove: (k) => void kept.delete(k),
  };
}

let root: HTMLElement | null = null;
let detach: (() => void) | null = null;

/** The screen over a TV that kept the household and who it was left on (Anna); the server never answers. */
function show(p: { pin?: boolean; hasPin: boolean; passed?: boolean }) {
  document.body.innerHTML = "";
  root = document.createElement("div");
  document.body.appendChild(root);
  const focus = new FocusManager();
  focus.setPage(root);
  const input = new Input(focus);
  detach = input.attach(document);
  const ui = { focus, input, claim: () => {}, nav: { restart: () => {} }, reducedMotion: true } as unknown as Ui;
  const store = memoryStore();
  store.set("pley.server", SERVER);
  store.set(`pley.householdProfiles.${SERVER}`, JSON.stringify(household));
  store.set("pley.me", JSON.stringify({ id: 1, name: "Anna", has_pin: p.hasPin }));
  const platform = { name: "tizen", clientId: "test", storage: store } as unknown as Platform;
  const app = new AppState(platform, () => new Promise(() => undefined));
  (app.api as { value: unknown }).value = { profiles: () => new Promise(() => undefined) };
  app.profileGatePassed = !!p.passed;
  act(() =>
    render(
      <AppContext.Provider value={app}>
        <UiContext.Provider value={ui}>
          <ProfilesScreen pin={p.pin} />
        </UiContext.Provider>
      </AppContext.Provider>,
      root!,
    ),
  );
  return input;
}

afterEach(() => {
  if (root) render(null, root);
  detach?.();
  detach = null;
  root = null;
});

describe("Who's watching? at a start", () => {
  it("opens on the pad of the profile the TV was left on when it comes first", () => {
    show({ pin: true, hasPin: true });
    expect(document.body.textContent).toContain("Enter Anna's PIN");
    expect(document.body.textContent).not.toContain("Who's watching?");
  });

  it("shows the profiles again on Back", () => {
    show({ pin: true, hasPin: true });
    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", keyCode: 27 }));
    });
    expect(document.body.textContent).toContain("Who's watching?");
  });

  it("shows the profiles when that profile has no PIN, the pad isn't asked for, or the question was answered", () => {
    for (const p of [{ pin: true, hasPin: false }, { hasPin: true }, { pin: true, hasPin: true, passed: true }]) {
      show(p);
      expect([p, document.body.textContent?.includes("Who's watching?")]).toEqual([p, true]);
      render(null, root!);
      detach?.();
    }
  });
});
