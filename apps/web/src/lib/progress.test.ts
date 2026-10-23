import { describe, expect, it, vi } from "vitest";
import { apiError, mockApi } from "@/test/render";

/** A fresh copy of the module (its client id lives in module state). */
async function freshModule() {
  vi.resetModules();
  return import("./progress");
}

describe("progress reports", () => {
  it("survive page unloads and never fail", async () => {
    const { reportProgress } = await freshModule();
    const requests = mockApi({ "POST /api/items/5/progress": () => apiError(503, "unavailable", "restarting") });

    await expect(reportProgress(5, { position_ms: 1_000, state: "playing" }, true)).resolves.toBeUndefined();
    expect(requests[0]?.init.keepalive).toBe(true);
    expect(requests[0]?.body).toEqual({ position_ms: 1_000, state: "playing" });
  });

  it("end a session with its final position, releasing it at the same time", async () => {
    const { endSession } = await freshModule();
    const requests = mockApi({
      "POST /api/items/5/progress": () => undefined,
      "DELETE /api/playback/a%20b": () => undefined,
    });

    await endSession(5, { session_id: "a b", duration_ms: 0 }, -20.4);

    expect(requests.map((r) => `${r.method} ${r.path}`).sort()).toEqual([
      "DELETE /api/playback/a%20b",
      "POST /api/items/5/progress",
    ]);
    expect(requests.find((r) => r.method === "POST")?.body).toEqual({ position_ms: 0, state: "stopped", session_id: "a b" });
  });

  it("start playback as this tab", async () => {
    const { clientId, startPlayback } = await freshModule();
    const requests = mockApi({ "POST /api/playback": () => ({ session_id: "s1" }) });

    await startPlayback({ item_id: 5 } as Parameters<typeof startPlayback>[0]);
    expect(requests[0]?.body).toEqual({ item_id: 5, client_id: clientId() });
  });
});

describe("clientId", () => {
  it("stays the same in a tab, and across reloads", async () => {
    const first = await freshModule();
    const id = first.clientId();
    expect(id).toMatch(/^[0-9a-f]{32}$/);
    expect(first.clientId()).toBe(id);

    // Unloading keeps it for the reload, which takes it back out.
    window.dispatchEvent(new Event("pagehide"));
    const reloaded = await freshModule();
    expect(reloaded.clientId()).toBe(id);
    expect(window.sessionStorage.length).toBe(0);
  });

  it("is new in a duplicated tab", async () => {
    const first = await freshModule();
    const id = first.clientId();
    const duplicate = await freshModule();
    expect(duplicate.clientId()).not.toBe(id);
  });
});
