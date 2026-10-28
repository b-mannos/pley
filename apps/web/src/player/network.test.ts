import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { mockApi } from "@/test/render";
import {
  forgetNetwork,
  measureSpeed,
  measureWhole,
  networkClientId,
  networkStart,
  reportCeiling,
  speedOf,
  startCeiling,
  type Download,
  type NetworkAnswer,
  type NetworkIo,
  type SpeedResult,
  type Timers,
} from "./network";

const answer = (over: Partial<NetworkAnswer> = {}): NetworkAnswer => ({
  local: false,
  ceiling_kbps: null,
  source: null,
  since: null,
  probe: false,
  ...over,
});

/** Timers on a clock the test moves (`advance`). */
function fakeTimers() {
  let t = 0;
  let next = 1;
  const due = new Map<number, { at: number; fn: () => void }>();
  const timers: Timers = {
    now: () => t,
    set: (fn, ms) => {
      const id = next++;
      due.set(id, { at: t + ms, fn });
      return id;
    },
    clear: (id) => void due.delete(id as number),
  };
  const advance = (ms: number) => {
    const end = t + ms;
    for (;;) {
      const first = [...due.entries()].sort((a, b) => a[1].at - b[1].at)[0];
      if (!first || first[1].at > end) break;
      due.delete(first[0]);
      t = first[1].at;
      first[1].fn();
    }
    t = end;
  };
  return { timers, advance };
}

/** A download the test feeds: `give` bytes, `end` it, or `fail` it. */
function fakeDownload() {
  let onData: (n: number) => void = () => undefined;
  let finish: () => void = () => undefined;
  let fail: (e: Error) => void = () => undefined;
  const state = { stopped: false };
  const download: Download = (listener) => {
    onData = listener;
    const done = new Promise<void>((resolve, reject) => {
      finish = resolve;
      fail = reject;
    });
    return { done, stop: () => (state.stopped = true) };
  };
  return { download, state, give: (n: number) => onData(n), end: () => finish(), fail: (e: Error) => fail(e) };
}

const MiB = 1024 * 1024;

describe("a speed test", () => {
  it("reads kbps as bytes × 8 / seconds / 1000, from the first byte", () => {
    expect(speedOf(4 * MiB, 2_000, false)).toEqual({ kbps: 16_777, complete: false });
    expect(speedOf(256 * 1024 - 1, 2_000, true)).toBeNull();
  });

  it("stops 2 s after the first byte, not counting the wait for it", async () => {
    const { timers, advance } = fakeTimers();
    const d = fakeDownload();
    const result = measureSpeed(d.download, timers);
    advance(400);
    d.give(MiB);
    advance(1_000);
    d.give(MiB);
    advance(1_000);
    d.give(MiB);
    expect(d.state.stopped).toBe(true);
    // 2 MiB in 2 s: the piece after the stop isn't counted.
    expect(await result).toEqual({ kbps: Math.round((2 * MiB * 8) / 2_000), complete: false });
  });

  it("is complete when all of it arrived within the 2 s", async () => {
    const { timers, advance } = fakeTimers();
    const d = fakeDownload();
    const result = measureSpeed(d.download, timers);
    d.give(16 * MiB);
    advance(500);
    d.give(16 * MiB);
    d.end();
    expect(await result).toEqual({ kbps: Math.round((32 * MiB * 8) / 500), complete: true });
  });

  it("says nothing without a first byte within 1 s, or with under 256 KiB", async () => {
    const { timers, advance } = fakeTimers();
    const silent = fakeDownload();
    const none = measureSpeed(silent.download, timers);
    advance(1_000);
    expect(silent.state.stopped).toBe(true);
    expect(await none).toBeNull();

    const trickle = fakeDownload();
    const little = measureSpeed(trickle.download, timers);
    trickle.give(100_000);
    advance(2_000);
    expect(await little).toBeNull();
  });

  it("counts what came before a failure", async () => {
    const { timers, advance } = fakeTimers();
    const d = fakeDownload();
    const result = measureSpeed(d.download, timers);
    d.give(MiB);
    advance(1_000);
    d.fail(new Error("reset"));
    expect(await result).toEqual({ kbps: Math.round((MiB * 8) / 1_000), complete: false });
  });

  it("read whole is timed from the request and never complete", async () => {
    let t = 0;
    const result = await measureWhole(() => {
      t += 1_600;
      return Promise.resolve(8 * MiB);
    }, () => t);
    expect(result).toEqual({ kbps: Math.round((8 * MiB * 8) / 1_600), complete: false });
  });
});

/** The server's side for `startCeiling`: what it answers, and what it was told. */
function io(asked: NetworkAnswer | Error, opts: { speed?: SpeedResult | null; told?: (r: unknown) => NetworkAnswer | Error } = {}) {
  const told: unknown[] = [];
  let tests = 0;
  const value: NetworkIo = {
    ask: () => (asked instanceof Error ? Promise.reject(asked) : Promise.resolve(asked)),
    tell: (r) => {
      told.push(r);
      const a = opts.told ? opts.told(r) : answer();
      return a instanceof Error ? Promise.reject(a) : Promise.resolve(a);
    },
    speedTest: () => {
      tests++;
      return Promise.resolve(opts.speed ?? null);
    },
    wall: () => 5_000,
  };
  return { io: value, told, tests: () => tests };
}

describe("where Automatic starts, from the server", () => {
  it("is the server's ceiling, with its time; none when it knows of none", async () => {
    expect(await startCeiling(io(answer({ ceiling_kbps: 8_000, source: "playback", since: 1_000 })).io)).toEqual({ kbps: 8_000, at: 1_000 });
    expect(await startCeiling(io(answer()).io)).toBeNull();
  });

  it("is the browser's own memory when the server doesn't answer, or answers something else", async () => {
    expect(await startCeiling(io(new Error("404")).io)).toBeUndefined();
    expect(await startCeiling({ ...io(answer()).io, ask: () => Promise.resolve("<html>" as unknown as NetworkAnswer) })).toBeUndefined();
  });

  it("runs a speed test when asked, reports it, and starts as the server then says", async () => {
    const s = io(answer({ probe: true }), {
      speed: { kbps: 20_000, complete: false },
      told: () => answer({ ceiling_kbps: 14_000, source: "probe", since: 4_000 }),
    });
    expect(await startCeiling(s.io)).toEqual({ kbps: 14_000, at: 4_000 });
    expect(s.told).toEqual([{ probe_kbps: 20_000, probe_complete: false }]);
  });

  it("never tests the server's own network", async () => {
    const s = io(answer({ probe: true, local: true }), { speed: { kbps: 1, complete: false } });
    expect(await startCeiling(s.io)).toBeNull();
    expect(s.tests()).toBe(0);
  });

  it("reports nothing of a test that measured nothing", async () => {
    const s = io(answer({ probe: true }), { speed: null });
    expect(await startCeiling(s.io)).toBeUndefined();
    expect(s.told).toEqual([]);
  });

  it("goes by the test itself when its report isn't heard in time: 70 %, or no limit when all of it arrived", async () => {
    const slow = io(answer({ probe: true }), { speed: { kbps: 20_000, complete: false }, told: () => new Error("timed out") });
    expect(await startCeiling(slow.io)).toEqual({ kbps: 14_000, at: 5_000 });
    const fast = io(answer({ probe: true }), { speed: { kbps: 300_000, complete: true }, told: () => new Error("timed out") });
    expect(await startCeiling(fast.io)).toBeNull();
  });
});

describe("this browser and the server", () => {
  beforeEach(() => {
    forgetNetwork();
    window.localStorage.removeItem("pley.networkClient");
  });
  afterEach(() => vi.useRealTimers());

  it("is one client for good, not one per tab", () => {
    const id = networkClientId();
    expect(id).toMatch(/^web-[0-9a-f]{32}$/);
    forgetNetwork();
    expect(networkClientId()).toBe(id);
  });

  it("asks once for the playbacks starting now", async () => {
    const requests = mockApi({ "GET /api/playback/network": () => answer({ ceiling_kbps: 4_000, since: 9 }) });
    const [a, b] = [networkStart(), networkStart()];
    expect(await a).toEqual({ kbps: 4_000, at: 9 });
    expect(await b).toEqual({ kbps: 4_000, at: 9 });
    expect(requests).toHaveLength(1);
  });

  it("streams the speed test, then reports it", async () => {
    const chunk = new Uint8Array(512 * 1024);
    const requests = mockApi({
      "GET /api/playback/network": () => answer({ probe: true }),
      "GET /api/playback/speedtest": () =>
        new Response(
          new ReadableStream({
            start(c) {
              c.enqueue(chunk);
              c.enqueue(chunk);
              c.close();
            },
          }),
          { status: 200, headers: { "content-type": "application/octet-stream" } },
        ),
      "PUT /api/playback/network": () => answer({ ceiling_kbps: null, source: "probe", since: 1 }),
    });
    expect(await networkStart()).toBeNull();
    expect(requests.map((r) => `${r.method} ${r.path.split("?")[0]}`)).toEqual([
      "GET /api/playback/network",
      "GET /api/playback/speedtest",
      "PUT /api/playback/network",
    ]);
    expect(requests[1]!.path).toBe("/api/playback/speedtest?bytes=33554432");
    expect(requests[2]!.body).toMatchObject({ client_id: networkClientId(), probe_complete: true });
  });

  it("tells each new ceiling, and stops asking and telling an older server that answers 404", async () => {
    const requests = mockApi({ "PUT /api/playback/network": () => answer() });
    reportCeiling(7_500.4);
    reportCeiling(null);
    await vi.waitFor(() => expect(requests).toHaveLength(2));
    expect(requests.map((r) => r.body)).toEqual([
      { client_id: networkClientId(), ceiling_kbps: 7_500 },
      { client_id: networkClientId(), ceiling_kbps: null },
    ]);

    const older = mockApi({});
    expect(await networkStart()).toBeUndefined();
    reportCeiling(4_000);
    expect(await networkStart()).toBeUndefined();
    expect(older).toHaveLength(1);
  });

  it("gives the server 1.5 s to answer", async () => {
    vi.useFakeTimers();
    mockApi({ "GET /api/playback/network": () => new Promise(() => undefined) });
    const start = networkStart();
    await vi.advanceTimersByTimeAsync(1_500);
    expect(await start).toBeUndefined();
  });
});
