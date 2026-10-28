import { isApiError, type Api } from "../api/client";
import type { NetworkAnswer, NetworkReport } from "../api/types";
import { Cancel } from "../net/transport";
import type { Remembered } from "./autoquality";

/**
 * What the server knows of the network the TV plays on, for Automatic's
 * start (docs/QUALITY.md, "Automatic"), and the speed test it asks for on a
 * remote network it doesn't know yet. The TVs read the test as it arrives
 * through XMLHttpRequest's progress, which every TV's engine has (fetch's
 * streamed bodies can't be stopped cleanly before Chromium 66).
 */

/** How long the server is given to say what it knows; past that, the TV's own memory. */
export const NETWORK_ASK_MS = 1_500;
export const SPEED_TEST_BYTES = 32 * 1024 * 1024;
/** The first byte must come this soon; from it, the test reads this long. */
export const SPEED_TEST_FIRST_BYTE_MS = 1_000;
export const SPEED_TEST_WINDOW_MS = 2_000;
/** Less than this arrived says nothing of the network. */
export const SPEED_TEST_MIN_BYTES = 256 * 1024;
/** A test that couldn't read all of it keeps 30 % in hand, as the server does. */
export const PROBE_SHARE = 0.7;
/** The longest a speed test's report is waited for before starting. */
export const REPORT_WAIT_MS = 500;
/** A speed test isn't run again this soon, whatever came of the last. */
export const PROBE_AGAIN_MS = 10 * 60_000;
const MAX_KBPS = 10_000_000;

export interface SpeedResult {
  kbps: number;
  complete: boolean;
}

export interface Timers {
  now(): number;
  set(fn: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const realTimers: Timers = {
  now: () => (typeof performance !== "undefined" ? performance.now() : Date.now()),
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/** What `bytes` read over `ms` (from the first byte) say; `null` when too little arrived to say anything. */
export function speedOf(bytes: number, ms: number, complete: boolean): SpeedResult | null {
  if (bytes < SPEED_TEST_MIN_BYTES) return null;
  // bytes × 8 / seconds / 1000 = bits per millisecond.
  const kbps = Math.round((bytes * 8) / Math.max(1, ms));
  return { kbps: Math.max(1, Math.min(MAX_KBPS, kbps)), complete };
}

/**
 * A download read as it arrives: `onData` hears each piece's size, `done`
 * settles when all of it arrived (or it failed), `stop` ends it early.
 */
export type Download = (onData: (bytes: number) => void) => { done: Promise<unknown>; stop: () => void };

/**
 * A speed test over `download`: timed from the first byte, stopped 2 s
 * after it (or 1 s without one), complete when all of it came within the
 * 2 s. `null` when it says nothing (too little arrived).
 */
export function measureSpeed(download: Download, timers: Timers = realTimers): Promise<SpeedResult | null> {
  return new Promise((resolve) => {
    let first: number | null = null;
    let bytes = 0;
    let settled = false;
    let timer: unknown = null;
    let stop: () => void = () => undefined;
    const finish = (complete: boolean) => {
      if (settled) return;
      settled = true;
      timers.clear(timer);
      stop();
      resolve(first === null ? null : speedOf(bytes, timers.now() - first, complete));
    };
    timer = timers.set(() => finish(false), SPEED_TEST_FIRST_BYTE_MS);
    const run = download((n) => {
      if (settled || n <= 0) return;
      if (first === null) {
        first = timers.now();
        timers.clear(timer);
        timer = timers.set(() => finish(false), SPEED_TEST_WINDOW_MS);
      }
      bytes += n;
    });
    stop = run.stop;
    run.done.then(
      () => finish(first !== null && timers.now() - first <= SPEED_TEST_WINDOW_MS),
      () => finish(false),
    );
  });
}

export interface NetworkIo {
  ask(): Promise<NetworkAnswer>;
  tell(report: NetworkReport): Promise<NetworkAnswer>;
  speedTest(): Promise<SpeedResult | null>;
  /** The time remembered ceilings are kept in (ms since 1970). */
  wall(): number;
}

function isAnswer(a: unknown): a is NetworkAnswer {
  const v = a as Partial<NetworkAnswer> | null;
  return !!v && typeof v === "object" && typeof v.probe === "boolean" && (v.ceiling_kbps === null || typeof v.ceiling_kbps === "number");
}

/**
 * Where Automatic may start, from the server: the ceiling it knows for
 * this network (`null`: none), after a speed test when it asks for one;
 * `undefined` when it can't say (no answer, an older server, a test that
 * measured nothing), and the TV's own memory decides as before.
 */
export async function startCeiling(io: NetworkIo): Promise<Remembered | null | undefined> {
  let answer: NetworkAnswer;
  try {
    const asked = await io.ask();
    if (!isAnswer(asked)) return undefined;
    answer = asked;
  } catch {
    return undefined;
  }
  if (answer.probe && !answer.local) {
    const speed = await io.speedTest().catch(() => null);
    if (!speed) return undefined;
    try {
      const told = await io.tell({ probe_kbps: speed.kbps, probe_complete: speed.complete });
      if (!isAnswer(told)) throw new Error("no answer");
      answer = told;
    } catch {
      // Not heard in time: what the server would make of it.
      return speed.complete ? null : { kbps: Math.round(speed.kbps * PROBE_SHARE), at: io.wall() };
    }
  }
  return answer.ceiling_kbps && answer.ceiling_kbps > 0 ? { kbps: answer.ceiling_kbps, at: answer.since ?? io.wall() } : null;
}

/** What the player asks of the server's memory of networks. */
export interface NetworkMemory {
  /** Where Automatic may start (`startCeiling`). */
  start(): Promise<Remembered | null | undefined>;
  /** Automatic's remembered ceiling changed (`null`: forgotten): fire and forget. */
  report(kbps: number | null): void;
}

/** What was heard of each server in this run of the app: an older one (404) isn't asked again. */
const servers = new Map<string, { older: boolean; lastProbe: number }>();

/** The server's memory of networks, for this TV (`clientId`, the one it plays as). */
export function serverNetwork(api: Api, clientId: string, timers: Timers = realTimers): NetworkMemory {
  let heard = servers.get(api.server);
  if (!heard) {
    heard = { older: false, lastProbe: -Infinity };
    servers.set(api.server, heard);
  }
  const server = heard;
  const older = (e: unknown) => {
    if (isApiError(e, 404)) server.older = true;
  };
  const io: NetworkIo = {
    ask: () =>
      api.network(clientId, NETWORK_ASK_MS).catch((e: unknown) => {
        older(e);
        throw e;
      }),
    tell: (report) => api.tellNetwork(clientId, report, REPORT_WAIT_MS),
    speedTest: () => {
      const now = timers.now();
      if (now - server.lastProbe < PROBE_AGAIN_MS) return Promise.resolve(null);
      server.lastProbe = now;
      return measureSpeed((onData) => {
        const cancel = new Cancel();
        let seen = 0;
        const done = api.speedTest(
          SPEED_TEST_BYTES,
          (loaded) => {
            onData(loaded - seen);
            seen = loaded;
          },
          cancel,
        );
        return { done, stop: () => cancel.cancel() };
      }, timers);
    },
    wall: () => Date.now(),
  };
  return {
    start: () => (server.older ? Promise.resolve(undefined) : startCeiling(io)),
    report: (kbps) => {
      if (server.older) return;
      api.tellNetwork(clientId, { ceiling_kbps: kbps === null ? null : Math.round(kbps) }).catch(older);
    },
  };
}

/** Forgets what was heard of servers (tests). */
export function forgetServers(): void {
  servers.clear();
}
