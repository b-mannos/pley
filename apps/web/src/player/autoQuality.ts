/**
 * Automatic quality (docs/QUALITY.md): the original, unless it doesn't
 * play well; then the quality that does. The player tells it what it sees
 * (waits for data, how fast video arrives, how fast the network loads,
 * a decoder falling behind) and starts again at the quality it names.
 * Nothing here knows the player or the clock: times come in, in ms.
 */

export const ORIGINAL_ID = "original";

/** A wait for data this long is a play issue; two shorter ones within a minute are one too. */
export const LONG_WAIT_MS = 2_000;
export const SHORT_WAITS_WITHIN_MS = 60_000;
/** Delivery is measured over this much time, once there is at least the second figure of it. */
export const DELIVERY_WINDOW_MS = 20_000;
export const DELIVERY_MIN_SPAN_MS = 5_000;
/** Video arriving slower than 1.1 s a second, with under 10 s ahead and falling, for 15 s: a play issue before it waits. */
export const DELIVERY_LOW = 1.1;
export const DELIVERY_LOW_FOR_MS = 15_000;
export const DELIVERY_AHEAD_S = 10;
/** The new ceiling keeps a quarter in hand. */
export const SAFETY = 0.75;
/** No change comes sooner than this after the last. */
export const CHANGE_GAP_MS = 20_000;
/** A step back up needs this long without an issue, and this long since the last step down. */
export const CALM_MS = 3 * 60_000;
export const AFTER_DOWN_MS = 5 * 60_000;
/** The network must carry this many times the next quality up. */
export const HEADROOM = 1.6;
/** A step up a step down undoes this soon was one too far: it isn't tried again during the title. */
export const SETTLE_MS = 10 * 60_000;
/** Each step up undone doubles the calm the next one waits for, up to this. */
export const MAX_CALM_MS = 4 * CALM_MS;
/** A slow network is remembered this long, per device and server. */
export const MEMORY_MS = 30 * 60_000;
/** Network figures older than this go. */
const KEEP_MS = 6 * 60_000;

/** A quality Automatic can play below the original: highest first, each lower than the one before. */
export interface Rung {
  id: string;
  label: string;
  kbps: number;
  height: number;
}

/** Why Automatic went below the original. */
export type Lowered = "network" | "device";

/** A change Automatic decided on: the player starts again at `to`. */
export interface AutoStep {
  /** "original" or a rung's id. */
  to: string;
  /** Why it went down; `null` for a step back up. */
  why: Lowered | null;
}

/** What the network carried when it last stepped down (kbps), and when (ms since 1970). */
export interface Remembered {
  kbps: number;
  at: number;
}

export interface AutoMemory {
  get(): Remembered | null;
  set(value: Remembered | null): void;
}

/** A memory kept in a key-value store under `key` (one per device and server). */
export function storedMemory(store: { get(key: string): string | null; set(key: string, value: string): void; remove(key: string): void }, key: string): AutoMemory {
  return {
    get() {
      try {
        const text = store.get(key);
        const v = text ? (JSON.parse(text) as Remembered) : null;
        return v && typeof v.kbps === "number" && typeof v.at === "number" ? v : null;
      } catch {
        return null;
      }
    },
    set(value) {
      try {
        if (value) store.set(key, JSON.stringify(value));
        else store.remove(key);
      } catch {
        // Storage full or blocked: the next title starts at the original.
      }
    },
  };
}

export interface AutoOptions {
  /** The qualities below the original it may play (see `Rung`). */
  ladder: Rung[];
  /** What the original plays at, kbps: the file's bitrate, or the server's quality when it converts it; `null` when unknown. */
  originalKbps: number | null;
  memory?: AutoMemory | null;
  /** The time the memory is kept in (ms since 1970); `Date.now` unless a test says. */
  wall?: () => number;
  /**
   * The remembered ceiling changed (kbps; `null`: forgotten), for the
   * server to keep for this network too (docs/QUALITY.md).
   */
  onCeiling?: (kbps: number | null) => void;
}

/**
 * One title's Automatic: where it plays now, and when to change. Created
 * for each title; `start` says where it begins.
 */
export class AutoQuality {
  /** "original" or a rung's id. */
  playing = ORIGINAL_ID;
  /** Why it plays below the original; `null` at the original. */
  lowered: Lowered | null = null;

  private readonly ladder: Rung[];
  private originalKbps: number | null;
  /** The original plays converted (`setOriginalKbps`), not as the file is. */
  private converted = false;
  private readonly memory: AutoMemory | null;
  private readonly wall: () => number;
  private readonly onCeiling: ((kbps: number | null) => void) | null;
  private begun = false;

  private lastChange = -1e15;
  private lastDown = -1e15;
  private lastIssue = -1e15;
  private pending: Lowered | null = null;
  /** The last step up, until a step down or `SETTLE_MS` says whether it held. */
  private lastUp: { at: number; to: string } | null = null;
  /** Qualities a step down undid soon after a step up to them: not tried again this title. */
  private readonly tooHigh = new Set<string>();
  /** The calm a step up waits for: `CALM_MS`, doubled by each step up undone. */
  private calmMs = CALM_MS;

  private waitFrom: number | null = null;
  private shortWaits: number[] = [];
  private fills: { at: number; end: number }[] = [];
  /** The last delivery measured, seconds of video a second; `null` while unknown. */
  private delivery: number | null = null;
  private lowFrom: number | null = null;
  private lowAhead = 0;
  private loads: { at: number; bytes: number; ms: number }[] = [];
  private rates: { at: number; kbps: number }[] = [];

  constructor(o: AutoOptions) {
    this.ladder = o.ladder;
    this.originalKbps = o.originalKbps && o.originalKbps > 0 ? o.originalKbps : null;
    this.memory = o.memory ?? null;
    this.wall = o.wall ?? (() => Date.now());
    this.onCeiling = o.onCeiling ?? null;
  }

  /** Whether `start` has said where it begins. */
  get started(): boolean {
    return this.begun;
  }

  /**
   * Where it starts: the original, or below it when a network too slow for
   * it was found in the last 30 minutes. `now` is the player's clock.
   * `known`: what the server knows of this network instead (a ceiling, or
   * `null` for none), which keeps its own time; left out when it can't say.
   */
  start(now: number, known?: Remembered | null): string {
    this.begun = true;
    this.lastIssue = now;
    const fromServer = known !== undefined;
    const kept = fromServer ? known : this.memory ? this.memory.get() : null;
    // A clock set back a little since doesn't make it older than now.
    const age = kept ? Math.max(0, this.wall() - kept.at) : 0;
    if (kept && !fromServer && (age >= MEMORY_MS || kept.at - this.wall() >= MEMORY_MS)) this.memory!.set(null);
    else if (kept && this.ladder.length && (this.originalKbps === null || kept.kbps < this.originalKbps)) {
      this.playing = this.highestAtMost(kept.kbps, null)!.id;
      this.lowered = "network";
      this.lastDown = now - age;
    }
    return this.playing;
  }

  /** The bitrate playing now (kbps); `null` for an original of unknown bitrate. */
  playingKbps(): number | null {
    const rung = this.rung(this.playing);
    return rung ? rung.kbps : this.originalKbps;
  }

  /** The original plays converted (a fallback of the player's, or a file it can't play), at `kbps`. */
  setOriginalKbps(kbps: number | null): void {
    this.converted = true;
    if (kbps && kbps > 0) this.originalKbps = kbps;
  }

  /**
   * The original as the file is (direct play or a remux): only a picture
   * that stopped, a wait of 2 s or more, moves it. Short waits and slow
   * delivery don't: a browser keeps only seconds of a big file loaded
   * ahead, so they say little of the network, and a conversion is a worse
   * picture for good.
   */
  private get patient(): boolean {
    return this.playing === ORIGINAL_ID && !this.converted;
  }

  /** A new session began (a change, a recovery): what was measured of the one before goes, but the network's speed. */
  began(): void {
    this.waitFrom = null;
    this.shortWaits = [];
    this.pending = null;
    this.interrupted();
  }

  /** Paused, seeking, or out of sight: what it was measuring starts over, and a wait under way isn't one. */
  interrupted(): void {
    this.waitFrom = null;
    this.fills = [];
    this.delivery = null;
    this.lowFrom = null;
  }

  /** The picture stopped for want of data while meant to play (not after a seek, a pause or a new session). */
  waiting(now: number): void {
    if (this.waitFrom === null) this.waitFrom = now;
  }

  /** It plays on after a wait. */
  resumed(now: number): void {
    const from = this.waitFrom;
    if (from === null) return;
    this.waitFrom = null;
    if (now - from >= LONG_WAIT_MS) {
      this.pending = this.pending || "network";
      return;
    }
    if (this.patient) return;
    const recent = this.shortWaits.filter((t) => now - t < SHORT_WAITS_WITHIN_MS);
    recent.push(now);
    if (recent.length >= 2) {
      this.pending = this.pending || "network";
      this.shortWaits = [];
    } else {
      this.shortWaits = recent;
    }
  }

  /**
   * Where the playhead is and where what's loaded ahead of it ends, in
   * seconds, while it plays; `bufferedEnd` `null` when the player can't say.
   */
  sample(now: number, position: number, bufferedEnd: number | null): void {
    if (bufferedEnd === null || !isFinite(bufferedEnd)) return;
    const last = this.fills[this.fills.length - 1];
    if (last && bufferedEnd < last.end - 0.5) this.fills = [];
    this.fills.push({ at: now, end: bufferedEnd });
    while (this.fills.length > 2 && now - this.fills[1]!.at >= DELIVERY_WINDOW_MS) this.fills.shift();
    const first = this.fills[0]!;
    const span = now - first.at;
    if (span < DELIVERY_MIN_SPAN_MS) return;
    const delivery = Math.max(0, (bufferedEnd - first.end) / (span / 1000));
    this.delivery = delivery;
    const ahead = Math.max(0, bufferedEnd - position);
    const kbps = this.playingKbps();
    // Filling faster than it plays: the network carries at least that much.
    if (delivery > 1 && kbps !== null) this.bandwidth(now, delivery * kbps);
    if (delivery < DELIVERY_LOW && ahead < DELIVERY_AHEAD_S) {
      if (this.lowFrom === null) {
        this.lowFrom = now;
        this.lowAhead = ahead;
      } else if (now - this.lowFrom >= DELIVERY_LOW_FOR_MS) {
        if (ahead < this.lowAhead && !this.patient) this.pending = this.pending || "network";
        this.lowFrom = now;
        this.lowAhead = ahead;
      }
    } else {
      this.lowFrom = null;
    }
  }

  /** A download of `bytes` took `ms` (an HLS segment). */
  loaded(now: number, bytes: number, ms: number): void {
    if (bytes > 0 && ms > 0) this.loads.push({ at: now, bytes, ms });
    this.trim(now);
  }

  /** The player measured its network at `kbps`. */
  bandwidth(now: number, kbps: number): void {
    if (kbps > 0) this.rates.push({ at: now, kbps });
    this.trim(now);
  }

  /** The decoder fell behind (frames dropped, a decoder failing) while it converts. */
  decoding(): void {
    this.pending = "device";
  }

  /** The player itself turned the original into a conversion (it couldn't play it well): a change, as one of its own. */
  fellBack(now: number): void {
    this.lastChange = now;
    this.lastIssue = now;
    this.pending = null;
  }

  /** How fast the network carried data since `since`, kbps; `null` when nothing says. */
  network(since: number): number | null {
    let bytes = 0;
    let ms = 0;
    let count = 0;
    for (const l of this.loads) {
      if (l.at < since) continue;
      bytes += l.bytes;
      ms += l.ms;
      count++;
    }
    let best: number | null = count >= 2 && ms > 0 ? (bytes * 8) / ms : null;
    const said = this.rates.filter((r) => r.at >= since).map((r) => r.kbps);
    if (said.length) {
      said.sort((a, b) => a - b);
      const median = said[Math.floor(said.length / 2)]!;
      if (best === null || median > best) best = median;
    }
    return best;
  }

  /**
   * What to do now: a step down after a play issue, a step back up after a
   * calm spell, or nothing. The step is taken here; the player starts again
   * at `to`. `sizes`: the stream comes in several sizes the player moves
   * between as the network allows, which answer the network's issues: those
   * only put a step up off.
   */
  check(now: number, sizes = false): AutoStep | null {
    if (this.waitFrom !== null && now - this.waitFrom >= LONG_WAIT_MS) {
      this.pending = this.pending || "network";
      // Waiting on: another 2 s is another issue.
      this.waitFrom = now;
    }
    const issue = this.pending;
    this.pending = null;
    if (issue) {
      this.lastIssue = now;
      if (sizes && issue === "network") return null;
      return now - this.lastChange < CHANGE_GAP_MS ? null : this.stepDown(now, issue);
    }
    return this.stepUp(now);
  }

  private stepDown(now: number, why: Lowered): AutoStep | null {
    const kbps = this.playingKbps();
    let ceiling: number | null = null;
    if (kbps !== null) {
      let delivery = 1;
      if (why === "network") {
        const net = this.network(now - DELIVERY_WINDOW_MS);
        if (this.delivery !== null) delivery = this.delivery;
        else if (net !== null) delivery = net / kbps;
        delivery = Math.min(1, delivery);
      }
      ceiling = Math.round(kbps * delivery * SAFETY);
    } else {
      // An original of unknown bitrate: what the network carries, when it says.
      const net = this.network(now - DELIVERY_WINDOW_MS);
      if (net !== null) ceiling = Math.round(net * SAFETY);
    }
    const target = this.highestAtMost(ceiling, kbps);
    if (!target || target.id === this.playing) return null;
    // Down again from where it just stepped up to: that was one step too far.
    if (this.lastUp && this.lastUp.to === this.playing && now - this.lastUp.at < SETTLE_MS) {
      this.tooHigh.add(this.playing);
      this.calmMs = Math.min(MAX_CALM_MS, this.calmMs * 2);
    }
    this.lastUp = null;
    this.playing = target.id;
    this.lowered = why;
    this.lastChange = now;
    this.lastDown = now;
    this.interrupted();
    if (why === "network") this.remember(ceiling !== null ? Math.max(ceiling, target.kbps) : target.kbps);
    return { to: target.id, why };
  }

  private stepUp(now: number): AutoStep | null {
    if (this.playing === ORIGINAL_ID) return null;
    const calmFrom = Math.max(this.lastIssue, this.lastChange);
    if (now - calmFrom < this.calmMs || now - this.lastDown < AFTER_DOWN_MS) return null;
    const at = this.indexOf(this.playing);
    const next = at > 0 ? this.ladder[at - 1]! : null;
    // Settled: the step above didn't hold this title.
    if (this.tooHigh.has(next ? next.id : ORIGINAL_ID)) return null;
    const need = next ? next.kbps : this.originalKbps;
    const net = this.network(calmFrom);
    if (need === null || net === null || net < HEADROOM * need) return null;
    this.playing = next ? next.id : ORIGINAL_ID;
    this.lastChange = now;
    this.lastUp = { at: now, to: this.playing };
    this.interrupted();
    if (!next) {
      this.lowered = null;
      this.remember(null);
    } else if (this.lowered === "network") {
      // The network carries this one now: the next title starts here rather than lower.
      this.remember(next.kbps);
    }
    return { to: this.playing, why: null };
  }

  /** Remembers `kbps` as the network's ceiling (`null`: forgets it), here and on the server. */
  private remember(kbps: number | null): void {
    this.memory?.set(kbps === null ? null : { kbps, at: this.wall() });
    this.onCeiling?.(kbps);
  }

  /**
   * The highest rung at or below `ceiling` kbps (any, when `null`) and
   * below `under` kbps when given: at least one step down. The lowest rung
   * when none is that low; `null` when there is no rung below `under`.
   */
  private highestAtMost(ceiling: number | null, under: number | null): Rung | null {
    const below = this.ladder.filter((r) => under === null || r.kbps < under);
    const fits = below.filter((r) => ceiling === null || r.kbps <= ceiling);
    return fits[0] ?? below[below.length - 1] ?? null;
  }

  private rung(id: string): Rung | null {
    return this.ladder.filter((r) => r.id === id)[0] ?? null;
  }

  private indexOf(id: string): number {
    for (let i = 0; i < this.ladder.length; i++) if (this.ladder[i]!.id === id) return i;
    return -1;
  }

  private trim(now: number): void {
    while (this.loads.length && now - this.loads[0]!.at > KEEP_MS) this.loads.shift();
    while (this.rates.length && now - this.rates[0]!.at > KEEP_MS) this.rates.shift();
  }
}
