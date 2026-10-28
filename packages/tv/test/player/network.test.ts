import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ApiError, type Api } from "../../src/api/client";
import type { NetworkAnswer, NetworkReport } from "../../src/api/types";
import { NetworkError, type Cancel } from "../../src/net/transport";
import { forgetServers, measureSpeed, serverNetwork, speedOf, startCeiling, type Download, type NetworkIo, type Timers } from "../../src/player/network";

// The same decisions as the web's (apps/web/src/player/network.test.ts).

const answer = (over: Partial<NetworkAnswer> = {}): NetworkAnswer => ({ local: false, ceiling_kbps: null, source: null, since: null, probe: false, ...over });
const MiB = 1024 * 1024;

/** Timers on a clock the test moves. */
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

function fakeDownload() {
  let onData: (n: number) => void = () => undefined;
  let finish: () => void = () => undefined;
  const state = { stopped: false };
  const download: Download = (listener) => {
    onData = listener;
    return { done: new Promise<void>((resolve) => (finish = resolve)), stop: () => (state.stopped = true) };
  };
  return { download, state, give: (n: number) => onData(n), end: () => finish() };
}

describe("a speed test", () => {
  it("reads kbps from the first byte, and stops 2 s after it", async () => {
    const { timers, advance } = fakeTimers();
    const d = fakeDownload();
    const result = measureSpeed(d.download, timers);
    advance(700);
    d.give(MiB);
    advance(2_000);
    d.give(MiB);
    expect(d.state.stopped).toBe(true);
    expect(await result).toEqual({ kbps: Math.round((MiB * 8) / 2_000), complete: false });
  });

  it("is complete when all of it arrived within the 2 s, and says nothing of too little", async () => {
    const { timers, advance } = fakeTimers();
    const d = fakeDownload();
    const result = measureSpeed(d.download, timers);
    d.give(32 * MiB);
    advance(800);
    d.end();
    expect(await result).toEqual({ kbps: Math.round((32 * MiB * 8) / 800), complete: true });
    expect(speedOf(200_000, 2_000, false)).toBeNull();
  });
});

describe("where Automatic starts, from the server", () => {
  const io = (asked: NetworkAnswer, told: NetworkAnswer | Error, speed = { kbps: 20_000, complete: false }): NetworkIo => ({
    ask: () => Promise.resolve(asked),
    tell: () => (told instanceof Error ? Promise.reject(told) : Promise.resolve(told)),
    speedTest: () => Promise.resolve(speed),
    wall: () => 5_000,
  });

  it("is the server's ceiling after the test it asked for, or the test's own 70 % when the report isn't heard", async () => {
    expect(await startCeiling(io(answer({ probe: true }), answer({ ceiling_kbps: 14_000, since: 4 })))).toEqual({ kbps: 14_000, at: 4 });
    expect(await startCeiling(io(answer({ probe: true }), new Error("late")))).toEqual({ kbps: 14_000, at: 5_000 });
    expect(await startCeiling(io(answer({ ceiling_kbps: 8_000, since: 1 }), answer()))).toEqual({ kbps: 8_000, at: 1 });
  });
});

/** A server's side of the network routes, as the TV's Api asks them. */
function fakeApi(handlers: { network?: () => Promise<NetworkAnswer>; speed?: (progress: (n: number) => void, cancel: Cancel) => Promise<unknown> }) {
  const told: NetworkReport[] = [];
  const api = {
    server: "http://nas:7539",
    network: (_client: string, timeout: number) => {
      expect(timeout).toBe(1_500);
      return handlers.network ? handlers.network() : Promise.reject(new ApiError(404, "not_found", "no"));
    },
    tellNetwork: (_client: string, report: NetworkReport) => {
      told.push(report);
      return Promise.resolve(answer({ ceiling_kbps: "probe_kbps" in report ? Math.round(report.probe_kbps * 0.7) : null, since: 7 }));
    },
    speedTest: (bytes: number, progress: (n: number) => void, cancel: Cancel) => {
      expect(bytes).toBe(32 * MiB);
      return handlers.speed ? handlers.speed(progress, cancel) : Promise.reject(new NetworkError("down"));
    },
  } as unknown as Api;
  return { api, told };
}

describe("the TV and the server", () => {
  beforeEach(() => forgetServers());
  afterEach(() => forgetServers());

  it("reads the test from the request's progress, stops it 2 s after the first byte, and reports it", async () => {
    const { timers, advance } = fakeTimers();
    let cancelled = false;
    const { api, told } = fakeApi({
      network: () => Promise.resolve(answer({ probe: true })),
      speed: (progress, cancel) => {
        cancel.onCancel(() => (cancelled = true));
        // XMLHttpRequest says how much has arrived in all, not each piece.
        setTimeout(() => {
          progress(MiB);
          advance(1_000);
          progress(3 * MiB);
          advance(1_000);
        }, 0);
        return new Promise(() => undefined);
      },
    });
    const start = serverNetwork(api, "tizen-1", timers).start();
    expect(await start).toEqual({ kbps: Math.round(((3 * MiB * 8) / 2_000) * 0.7), at: 7 });
    expect(cancelled).toBe(true);
    expect(told).toEqual([{ probe_kbps: Math.round((3 * MiB * 8) / 2_000), probe_complete: false }]);
  });

  it("doesn't test again soon after a test, whatever came of it", async () => {
    const { timers } = fakeTimers();
    let tests = 0;
    const { api } = fakeApi({
      network: () => Promise.resolve(answer({ probe: true })),
      speed: () => {
        tests++;
        return Promise.reject(new NetworkError("down"));
      },
    });
    expect(await serverNetwork(api, "c", timers).start()).toBeUndefined();
    expect(await serverNetwork(api, "c", timers).start()).toBeUndefined();
    expect(tests).toBe(1);
  });

  it("leaves an older server alone once it answered 404, and tells a newer one each ceiling", async () => {
    const older = fakeApi({});
    const net = serverNetwork(older.api, "c");
    expect(await net.start()).toBeUndefined();
    net.report(4_000);
    expect(older.told).toEqual([]);

    forgetServers();
    const newer = fakeApi({ network: () => Promise.resolve(answer()) });
    const n = serverNetwork(newer.api, "c");
    expect(await n.start()).toBeNull();
    n.report(7_499.6);
    n.report(null);
    expect(newer.told).toEqual([{ ceiling_kbps: 7_500 }, { ceiling_kbps: null }]);
  });
});
