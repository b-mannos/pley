import { signal } from "@preact/signals";
import { Api, unreachable } from "./api/client";
import { EventStream } from "./api/events";
import type { Hub, HouseholdProfile, ItemDetail, Library, ServerEvent, User } from "./api/types";
import { origin } from "./auth/addresses";
import { SignedOutError, Tokens, type KeyValue } from "./auth/tokens";
import { xhrTransport, type Transport } from "./net/transport";
import type { Platform, Probe } from "./platform";
import { Reporter } from "./player/reporter";
import type { SubtitleFont, SubtitlePosition } from "./subs/style";
import { OffsetStore } from "./subs/sync";

/** What a person sets on this TV (Settings). */
export interface Settings {
  subtitleSize: "small" | "normal" | "large" | "largest";
  subtitleBox: boolean;
  subtitleFont: SubtitleFont;
  subtitlePosition: SubtitlePosition;
  /** Up Next plays after its 10 s countdown. */
  autoplayNext: boolean;
  reduceMotion: boolean;
  /** The compact readout over the video while the controls are hidden. */
  playbackDetails: boolean;
  /** The quality last picked ("auto", "original", "8m"…); Automatic until one is. */
  quality: string;
  /** Send TrueHD to the receiver as it is (the TV can't promise it). */
  passthroughTrueHd: boolean;
  /** Send DTS to the receiver as it is. */
  passthroughDts: boolean;
}

const DEFAULT_SETTINGS: Settings = {
  subtitleSize: "normal",
  subtitleBox: false,
  subtitleFont: "default",
  subtitlePosition: "bottom",
  autoplayNext: true,
  reduceMotion: false,
  playbackDetails: false,
  quality: "auto",
  passthroughTrueHd: false,
  passthroughDts: false,
};

const SERVER_KEY = "pley.server";
const SETTINGS_KEY = "pley.settings";
/** Set once a quality is picked: the settings saved before Automatic held Original whether picked or not. */
const QUALITY_PICKED_KEY = "pley.qualityPicked";
const HOME_KEY = "pley.home";
/** How many profiles the server's household has, as last heard; kept per server (`pley.household.<server>`). */
const HOUSEHOLD_KEY = "pley.household";
/** Who is watching and their libraries, as last heard: the rail and the library pages before the server answers. */
const ME_KEY = "pley.me";
const LIBRARIES_KEY = "pley.libraries";
const SERVERS_KEY = "pley.servers";
/** The household's profiles, as last heard; kept per server (`pley.householdProfiles.<server>`). */
const PROFILES_KEY = "pley.householdProfiles";
/** How long an item's details are kept for pages and playbacks. */
const ITEM_TTL_MS = 5 * 60_000;
/** Watch state changes come in bursts (an episode, its season and show; a play reaching 90 %): gathered this long. */
const STATE_GATHER_MS = 300;
/** How long "Showing what pley had saved. Reconnecting…" shows: as long as an error's toast. */
const OFFLINE_NOTICE_MS = 6_000;
/** While the server is out of reach it is asked this often (and given as long to answer). */
const OFFLINE_ASK_MS = 5_000;

/** A server this TV signed in to or chose, newest first on the server list. */
export interface SavedServer {
  url: string;
  name: string;
  lastUsed: number;
}

function readJson<T>(store: KeyValue, key: string): T | null {
  try {
    const text = store.get(key);
    return text ? (JSON.parse(text) as T) : null;
  } catch {
    return null;
  }
}

/** What the first screen is, from what the TV kept (no network): the Android app's StartGate. */
export type Start = { page: "servers" } | { page: "link"; server: string } | { page: "profiles" } | { page: "home" };

/**
 * The app's state: the server signed in to, its API and tokens, who is
 * watching, the libraries, the settings, Home as last seen (drawn first at
 * the next launch), the TV's capabilities, and the server's events.
 */
export class AppState {
  readonly platform: Platform;
  readonly http: Transport;
  readonly store: KeyValue;
  readonly reporter = new Reporter();
  readonly offsets: OffsetStore;

  readonly server = signal<string | null>(null);
  readonly api = signal<Api | null>(null);
  readonly me = signal<User | null>(null);
  readonly libraries = signal<Library[] | null>(null);
  readonly profiles = signal<HouseholdProfile[] | null>(null);
  readonly settings = signal<Settings>(DEFAULT_SETTINGS);
  readonly home = signal<Hub[] | null>(null);
  readonly homeError = signal<unknown>(null);
  /** The last try at who is watching and the libraries failed (and none were saved): the library pages offer Retry. */
  readonly accountError = signal<unknown>(null);
  /** Bumped by the server's events, so screens that list things fetch again. */
  readonly changes = signal(0);
  /** The server signed the TV out: show sign-in. */
  readonly signedOut = signal(false);
  /** Whether the TV shows the app (not another app, its home screen or standby). */
  readonly visible = signal(true);
  /**
   * The server answered a screen nothing (the Android app's TvOfflineState):
   * what shows is what was saved. Raised by a load the server couldn't
   * answer, lowered by its events connecting or a screen's fresh answer
   * after a failure.
   */
  readonly offline = signal(false);
  /** "Showing what pley had saved. Reconnecting…" shows, for 6 s from going offline; then the rail's avatar keeps a mark. */
  readonly offlineNotice = signal(false);
  /**
   * "Who's watching?" was answered in this launch (a profile picked, a PIN
   * checked, or a sign-in approved on a phone): the current profile's PIN
   * isn't asked again until the app starts over.
   */
  profileGatePassed = false;

  private tokens: Tokens | null = null;
  private events: EventStream | null = null;
  private readonly eventListeners: ((e: ServerEvent) => void)[] = [];
  private capsPromise: Promise<Probe | null> | null = null;
  private readonly items = new Map<number, { detail: ItemDetail; at: number }>();
  private libraryTimer: ReturnType<typeof setTimeout> | null = null;
  /** Watch state changes heard in the last 0.3 s, asked for again in one round. */
  private stateTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly stateIds = new Set<number>();
  private readonly stateParents = new Set<number>();
  /** Playbacks on top: Home and the pages wait to ask again until they end (their own reports change the watch state too). */
  private holds = 0;
  /** The libraries' order changes on their way to the server, one at a time. */
  private orderQueue: Promise<unknown> = Promise.resolve();
  private noticeTimer: ReturnType<typeof setTimeout> | null = null;
  private awayTimer: ReturnType<typeof setTimeout> | null = null;
  private readonly leavers: (() => void)[] = [];

  constructor(platform: Platform, http: Transport = xhrTransport) {
    this.platform = platform;
    this.http = http;
    this.store = platform.storage;
    this.offsets = new OffsetStore(platform.storage, () => this.me.value?.id ?? null);
    const kept = readJson<Partial<Settings>>(this.store, SETTINGS_KEY) ?? {};
    // Original was the default before Automatic, and was saved along with any other setting: unless picked, it is Automatic now.
    if (kept.quality === "original" && this.store.get(QUALITY_PICKED_KEY) !== "1") delete kept.quality;
    this.settings.value = { ...DEFAULT_SETTINGS, ...kept };
    const server = this.store.get(SERVER_KEY);
    if (server) {
      // Kept for the server in use before they were kept per server.
      for (const key of [HOUSEHOLD_KEY, PROFILES_KEY]) {
        const old = this.store.get(key);
        if (old === null) continue;
        this.store.remove(key);
        const home = origin(server);
        if (home) this.store.set(`${key}.${home}`, old);
      }
      this.use(server);
    }
    // What was saved is trusted only in shape: a store another build wrote, or a damaged one, is left out.
    const home = readJson<Hub[]>(this.store, HOME_KEY);
    const me = readJson<User>(this.store, ME_KEY);
    const libraries = readJson<Library[]>(this.store, LIBRARIES_KEY);
    this.home.value = Array.isArray(home) ? home : null;
    this.me.value = me && typeof me === "object" && typeof me.id === "number" ? me : null;
    this.libraries.value = Array.isArray(libraries) ? libraries : null;
  }

  /** The first screen, from what the TV kept. */
  start(): Start {
    const server = this.server.value;
    if (!server) return { page: "servers" };
    if (!this.tokens?.signedIn) return { page: "link", server };
    return this.householdSize() > 1 && this.alwaysStartAs() === null ? { page: "profiles" } : { page: "home" };
  }

  /** Signs in to `server` from now on (its tokens, if the TV has them). */
  use(server: string): void {
    const home = origin(server);
    if (!home) return;
    if (this.server.value === home && this.api.value) return;
    this.events?.stop();
    this.events = null;
    if (this.server.value && this.server.value !== home) this.leaveServer();
    this.server.value = home;
    this.store.set(SERVER_KEY, home);
    this.tokens = new Tokens(home, this.platform.clientId, this.store, this.http, () => {
      this.signedOut.value = true;
      this.events?.stop();
    });
    this.api.value = new Api(home, this.tokens, this.http);
    this.signedOut.value = false;
  }

  get signedIn(): boolean {
    return !!this.tokens?.signedIn;
  }

  get tokenStore(): Tokens | null {
    return this.tokens;
  }

  /** The servers this TV chose, most recently used first. */
  savedServers(): SavedServer[] {
    const list = readJson<SavedServer[]>(this.store, SERVERS_KEY);
    if (!Array.isArray(list)) return [];
    return list.filter((s) => s && typeof s.url === "string").sort((a, b) => b.lastUsed - a.lastUsed);
  }

  /** Keeps `url` on the server list, as used now. */
  rememberServer(url: string, name: string): void {
    const home = origin(url);
    if (!home) return;
    const list = this.savedServers().filter((s) => s.url !== home);
    list.unshift({ url: home, name, lastUsed: Date.now() });
    this.store.set(SERVERS_KEY, JSON.stringify(list.slice(0, 20)));
  }

  /** Whether the TV has a sign-in for `url`. */
  hasTokens(url: string): boolean {
    const home = origin(url);
    return !!home && new Tokens(home, this.platform.clientId, this.store, this.http).signedIn;
  }

  /**
   * This TV's own id for its playbacks ("webos-1f3a…"), kept for good: the
   * server ends a client's earlier sessions when it starts another, so two
   * TVs of a household must never share one.
   */
  installId(): string {
    let id = this.store.get("pley.installId");
    if (!id) {
      const bytes = new Uint8Array(16);
      try {
        crypto.getRandomValues(bytes);
      } catch {
        for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
      }
      id = `${this.platform.name}-${Array.prototype.map.call(bytes, (b: number) => (b < 16 ? "0" : "") + b.toString(16)).join("")}`;
      this.store.set("pley.installId", id);
    }
    return id;
  }

  /** The profile "Always start as" names for this server; `null` asks who's watching. */
  alwaysStartAs(): number | null {
    const v = this.store.get(`pley.alwaysStartAs.${this.server.value ?? ""}`);
    return v ? Number(v) : null;
  }

  setAlwaysStartAs(id: number | null): void {
    const key = `pley.alwaysStartAs.${this.server.value ?? ""}`;
    if (id === null) this.store.remove(key);
    else this.store.set(key, String(id));
  }

  /** How many profiles the household had when last asked (1 while unknown). */
  householdSize(): number {
    const n = Number(this.store.get(`${HOUSEHOLD_KEY}.${this.server.value ?? ""}`) ?? "1");
    return n > 0 ? n : 1;
  }

  /** The household as last fetched, for "Who's watching?" before the server answers. */
  savedHousehold(): HouseholdProfile[] {
    const list = readJson<HouseholdProfile[]>(this.store, `${PROFILES_KEY}.${this.server.value ?? ""}`);
    return Array.isArray(list) ? list : [];
  }

  keepHousehold(profiles: HouseholdProfile[]): void {
    this.profiles.value = profiles;
    const server = this.server.value ?? "";
    this.store.set(`${HOUSEHOLD_KEY}.${server}`, String(profiles.length));
    this.store.set(`${PROFILES_KEY}.${server}`, JSON.stringify(profiles));
  }

  /** `fn` runs when what plays must end: a profile switch, signing out, another server, the app closing. */
  onLeave(fn: () => void): void {
    this.leavers.push(fn);
  }

  /** Ends what plays in the background (the music's session) before the profile, the sign-in, the server or the app goes. */
  endPlayback(): void {
    for (const fn of this.leavers.slice()) fn();
  }

  /** The app acts as another profile now: what was shown for the one before goes. */
  async switched(): Promise<void> {
    this.endPlayback();
    this.forgetAccount();
    this.home.value = null;
    this.homeError.value = null;
    this.items.clear();
    this.me.value = null;
    await this.loadAccount();
    void this.loadHome();
  }

  /** Forgets a server of the list: its sign-in ends, and what was saved from it goes. */
  async forget(url: string): Promise<void> {
    const home = origin(url);
    if (!home) return;
    this.store.set(SERVERS_KEY, JSON.stringify(this.savedServers().filter((s) => s.url !== home)));
    const tokens = home === this.server.value && this.tokens ? this.tokens : new Tokens(home, this.platform.clientId, this.store, this.http);
    if (home === this.server.value) this.forgetServer();
    await tokens.signOut().catch(() => undefined);
  }

  /** Forgets the server in use (Change server). */
  forgetServer(): void {
    this.endPlayback();
    this.events?.stop();
    this.events = null;
    this.store.remove(SERVER_KEY);
    const server = this.server.value ?? "";
    this.store.remove(`${HOUSEHOLD_KEY}.${server}`);
    this.store.remove(`${PROFILES_KEY}.${server}`);
    this.leaveServer();
    this.server.value = null;
    this.api.value = null;
    this.tokens = null;
  }

  /** What was shown from the server in use goes: another one comes, or none. */
  private leaveServer(): void {
    this.endPlayback();
    this.forgetAccount();
    this.items.clear();
    this.home.value = null;
    this.homeError.value = null;
    this.accountError.value = null;
    this.me.value = null;
    this.libraries.value = null;
    this.profiles.value = null;
    this.offline.value = false;
    this.offlineNotice.value = false;
    this.profileGatePassed = false;
  }

  async signOut(): Promise<void> {
    this.events?.stop();
    // The music's session ends while the sign-in that made it still works.
    this.endPlayback();
    await this.reporter.flush(2_000);
    await this.tokens?.signOut();
    this.forgetAccount();
    this.home.value = null;
    this.signedOut.value = true;
  }

  updateSettings(patch: Partial<Settings>): void {
    if ("passthroughTrueHd" in patch || "passthroughDts" in patch) this.capsPromise = null;
    this.settings.value = { ...this.settings.value, ...patch };
    this.store.set(SETTINGS_KEY, JSON.stringify(this.settings.value));
    if ("quality" in patch) this.store.set(QUALITY_PICKED_KEY, "1");
  }

  // ---- what the server has

  /** What was saved of the profile signed in (Home, who it is, its libraries): another profile, sign-in or server comes. */
  private forgetAccount(): void {
    this.store.remove(HOME_KEY);
    this.store.remove(ME_KEY);
    this.store.remove(LIBRARIES_KEY);
  }

  /**
   * Who is watching, the libraries and the household, after sign-in or a
   * profile switch; kept, so the rail and the library pages have them
   * before the server answers next time. A failure is kept for the library
   * pages' Retry, and the server out of reach shows as such.
   */
  async loadAccount(): Promise<void> {
    const api = this.api.value;
    if (!api) return;
    let me: User;
    let libraries: Library[];
    try {
      [me, libraries] = await Promise.all([api.me(), api.libraries()]);
    } catch (e) {
      this.accountError.value = e;
      if (unreachable(e)) this.serverAway();
      throw e;
    }
    this.accountError.value = null;
    this.me.value = me;
    this.store.set(ME_KEY, JSON.stringify(me));
    this.keepLibraries(libraries);
    api.profiles().then(
      (profiles) => this.keepHousehold(profiles),
      () => undefined,
    );
  }

  private keepLibraries(libraries: Library[]): void {
    this.libraries.value = libraries;
    this.store.set(LIBRARIES_KEY, JSON.stringify(libraries));
  }

  /**
   * The profile's own order of its libraries (`ids`, each once), or the
   * order everyone sees again (`null`), on every list of them on all its
   * devices. Changes go one at a time, in the order they were made; the
   * server's answer is the list from then on, and Home asks again, since
   * its rows of each library follow the order.
   */
  orderLibraries(ids: number[] | null): Promise<Library[]> {
    const run = this.orderQueue.then(async () => {
      const api = this.api.value;
      const who = this.me.value?.id;
      if (!api) throw new SignedOutError();
      const libraries = ids ? await api.sortLibraries(ids) : await api.forgetLibraryOrder();
      // Another profile, sign-in or server since: the answer was the one before's.
      if (this.api.value === api && this.me.value?.id === who) {
        this.keepLibraries(libraries);
        void this.loadHome();
      }
      return libraries;
    });
    this.orderQueue = run.catch(() => undefined);
    return run;
  }

  async loadHome(): Promise<void> {
    const api = this.api.value;
    if (!api) return;
    try {
      const { hubs } = await api.home();
      const failedBefore = this.homeError.value !== null;
      this.home.value = hubs;
      this.homeError.value = null;
      this.store.set(HOME_KEY, JSON.stringify(hubs));
      if (failedBefore) this.serverAnswered();
    } catch (e) {
      this.homeError.value = e;
      if (unreachable(e)) this.serverAway();
    }
  }

  // ---- the server out of reach

  /**
   * A screen's load got no answer: the notice shows for 6 s (again each
   * time the server goes), then the rail's mark. Meanwhile the server is
   * asked every 5 s, so the event stream, backing off up to 30 s between
   * tries, reconnects as soon as it answers and the screens come back.
   */
  serverAway(): void {
    if (this.offline.value) return;
    this.offline.value = true;
    this.offlineNotice.value = true;
    if (this.noticeTimer !== null) clearTimeout(this.noticeTimer);
    this.noticeTimer = setTimeout(() => {
      this.noticeTimer = null;
      this.offlineNotice.value = false;
    }, OFFLINE_NOTICE_MS);
    this.askWhileAway();
  }

  private askWhileAway(): void {
    if (this.awayTimer !== null) clearTimeout(this.awayTimer);
    this.awayTimer = setTimeout(() => {
      this.awayTimer = null;
      const api = this.api.value;
      if (!this.offline.value || !api) return;
      if (!this.visible.value) {
        this.askWhileAway();
        return;
      }
      api.reachable(OFFLINE_ASK_MS).then(
        () => {
          if (!this.offline.value) return;
          // A stream waiting to reconnect connects now, and its connection lowers it; one still
          // connected (a page failed in a short blip) can't, so the app does, and asks again.
          if (this.events?.retryNow()) {
            this.askWhileAway();
            return;
          }
          this.serverAnswered();
          this.items.clear();
          this.refresh();
        },
        () => this.askWhileAway(),
      );
    }, OFFLINE_ASK_MS);
  }

  /** The server answers again: its events connected, or a screen that had failed got a fresh answer. */
  serverAnswered(): void {
    if (!this.offline.value) return;
    if (this.noticeTimer !== null) clearTimeout(this.noticeTimer);
    this.noticeTimer = null;
    if (this.awayTimer !== null) clearTimeout(this.awayTimer);
    this.awayTimer = null;
    this.offline.value = false;
    this.offlineNotice.value = false;
  }

  /**
   * Item details the app saw lately, to start a page or a playback without
   * waiting. The server's events take out what changed; a detail older than
   * 5 minutes goes anyway, in case one was missed (a resume point moved on
   * another device while the event stream was down).
   */
  cachedItem(id: number): ItemDetail | null {
    const kept = this.items.get(id);
    if (!kept) return null;
    if (Date.now() - kept.at > ITEM_TTL_MS) {
      this.items.delete(id);
      return null;
    }
    return kept.detail;
  }

  async item(id: number): Promise<ItemDetail> {
    const api = this.api.value!;
    const detail = await api.item(id);
    this.keepItem(detail);
    return detail;
  }

  /** Keeps a detail fetched elsewhere (an item page) for the next page or playback. */
  keepItem(detail: ItemDetail): void {
    this.items.delete(detail.id);
    this.items.set(detail.id, { detail, at: Date.now() });
    if (this.items.size > 40) this.items.delete(this.items.keys().next().value!);
  }

  /**
   * Tracks were picked for `detail`'s title (the whole show for an
   * episode), or the profile's defaults changed (`detail` null): the
   * details kept name the tracks they started with before. `detail`, with
   * the choice in it, is kept; the others go.
   */
  tracksChanged(detail: ItemDetail | null): void {
    this.items.clear();
    if (detail) this.keepItem(detail);
  }

  /**
   * A playback is on top (`true`) or has ended (`false`). Meanwhile Home and
   * the pages wait to ask again; once it ends they do, and the details kept
   * go, since its own reports moved resume points and watched marks.
   */
  holdRefresh(on: boolean): void {
    this.holds = Math.max(0, this.holds + (on ? 1 : -1));
    if (on || this.holds > 0) return;
    this.items.clear();
    void this.loadHome();
    this.changes.value++;
  }

  /** Home and the pages that list things ask again; while a playback is on top, once it ends. */
  private refresh(): void {
    if (this.holds > 0) return;
    void this.loadHome();
    // Who is watching and the libraries, when the last try failed or none came yet.
    if (this.accountError.value !== null || !this.libraries.value) void this.loadAccount().catch(() => undefined);
    this.changes.value++;
  }

  // ---- the TV's capabilities

  /** The capabilities, probed once (after the first screen is drawn), again after Settings change them. */
  caps(fresh = false): Promise<Probe | null> {
    // Diagnostics' Check again: asked of the TV anew.
    if (fresh) this.capsPromise = null;
    if (!this.capsPromise) {
      const s = this.settings.value;
      this.capsPromise = this.platform.probe({ truehd: s.passthroughTrueHd, dts: s.passthroughDts }).catch(() => null);
    }
    return this.capsPromise;
  }

  // ---- the server's events

  onEvent(listener: (e: ServerEvent) => void): () => void {
    this.eventListeners.push(listener);
    return () => {
      const i = this.eventListeners.indexOf(listener);
      if (i >= 0) this.eventListeners.splice(i, 1);
    };
  }

  /** Follows the server's events while the app is shown and signed in. */
  listen(on: boolean): void {
    const server = this.server.value;
    if (!on || !server || !this.tokens?.signedIn) {
      this.events?.stop();
      return;
    }
    if (!this.events) {
      this.events = new EventStream(server, this.tokens, (e) => this.handleEvent(e), () => this.reconnected());
    }
    this.events.start();
  }

  private lastSync = 0;

  private reconnected(): void {
    // Nothing is replayed: what is older than 30 s, or anything after the server was out of reach, is fetched again.
    const away = this.offline.value;
    this.serverAnswered();
    if (away || Date.now() - this.lastSync > 30_000) {
      this.lastSync = Date.now();
      this.items.clear();
      this.refresh();
    }
  }

  private handleEvent(e: ServerEvent): void {
    switch (e.type) {
      case "library_changed":
        // Changes come in bursts during scans: once 1.5 s after the last.
        if (this.libraryTimer !== null) clearTimeout(this.libraryTimer);
        this.libraryTimer = setTimeout(() => {
          this.libraryTimer = null;
          this.refresh();
        }, 1_500);
        break;
      case "libraries_changed":
        void this.loadAccount().catch(() => undefined);
        this.refresh();
        break;
      case "watchlist_changed":
        this.refresh();
        break;
      case "user_state_changed": {
        // The item, its parent and grandparent; with `children`, everything under the first as well.
        for (const id of e.item_ids) this.stateIds.add(id);
        if (e.children && e.item_ids.length) this.stateParents.add(e.item_ids[0]!);
        if (this.stateTimer === null) this.stateTimer = setTimeout(() => this.applyStateChanges(), STATE_GATHER_MS);
        break;
      }
      default:
        break;
    }
    for (const l of this.eventListeners.slice()) l(e);
  }

  /** The watch state changes of the last 0.3 s: their details go, and Home and the pages ask again once (with a pending library change). */
  private applyStateChanges(): void {
    this.stateTimer = null;
    for (const [id, kept] of Array.from(this.items.entries())) {
      const d = kept.detail;
      const under = (p: number | null | undefined) => p !== null && p !== undefined && this.stateParents.has(p);
      if (this.stateIds.has(id) || under(d.parent_id) || under(d.grandparent_id)) this.items.delete(id);
    }
    this.stateIds.clear();
    this.stateParents.clear();
    if (this.libraryTimer !== null) {
      clearTimeout(this.libraryTimer);
      this.libraryTimer = null;
    }
    this.refresh();
  }
}
