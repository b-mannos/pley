import { render } from "preact";
import { act } from "preact/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import { UiContext, type Ui } from "../../src/ui/context";
import { FocusManager } from "../../src/ui/focus";
import { Input } from "../../src/ui/input";
import type { Key } from "../../src/ui/keys";
import { SignOutDialog } from "../../src/ui/screens/settings";

/** Settings → Sign out asks plainly, Cancel first. */

let root: HTMLElement | null = null;
let detach: (() => void) | null = null;

function setup(server: string | null = "pley.home:7539") {
  document.body.innerHTML = "";
  root = document.createElement("div");
  document.body.appendChild(root);
  const focus = new FocusManager();
  focus.setPage(root);
  const input = new Input(focus);
  detach = input.attach(document);
  const ui = {
    focus,
    input,
    claim: (el: HTMLElement, _fid: string | null, initial: boolean) => {
      if (initial && focus.inScope(el)) focus.focus(el);
    },
  } as unknown as Ui;
  const done: string[] = [];
  act(() =>
    render(
      <UiContext.Provider value={ui}>
        <SignOutDialog server={server} onCancel={() => done.push("cancel")} onSignOut={() => done.push("sign out")} />
      </UiContext.Provider>,
      root!,
    ),
  );
  const press = (key: Key) => {
    act(() => input.dispatch(key, "down", false));
    act(() => input.dispatch(key, "up", false));
  };
  return { root: root!, focus, done, press };
}

afterEach(() => {
  detach?.();
  detach = null;
  if (root) act(() => render(null, root!));
  root = null;
});

describe("the sign-out confirmation", () => {
  it("is a short question, with the server, and Cancel and Sign out", () => {
    const { root } = setup();
    const dialog = root.querySelector(".dialog")!;
    expect(dialog.querySelector(".t-title")!.textContent).toBe("Sign out?");
    expect(dialog.querySelector(".t-body")!.textContent).toBe("This TV signs out of pley.home:7539.");
    expect(Array.from(dialog.querySelectorAll(".text-btn")).map((b) => b.textContent)).toEqual(["Cancel", "Sign out"]);
  });

  it("is only the question when the server isn't known", () => {
    const { root } = setup(null);
    expect(root.querySelector(".dialog .t-body")).toBeNull();
  });

  it("starts on Cancel, and Back cancels", () => {
    const { focus, done, press } = setup();
    expect(focus.current?.textContent).toBe("Cancel");
    press("back");
    expect(done).toEqual(["cancel"]);
  });
});
