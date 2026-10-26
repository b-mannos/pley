import { signal } from "@preact/signals";
import { Api } from "./api/client";
import { EventStream } from "./api/events";
import type { Hub, HouseholdProfile, ItemDetail, Library, ServerEvent, User } from "./api/types";
import { origin } from "./auth/addresses";
import { Tokens, type KeyValue } from "./auth/tokens";
import { xhrTransport, type Transport } from "./net/transport";
import type { Platform, Probe } from "./platform";
import { Reporter } from "./player/reporter";
import { OffsetStore } from "./subs/sync";

/** What a person sets on this TV (Settings). */
export interface Settings {
  subtitleSize: "small" | "normal" | "large" | "largest";
  subtitleBox: boolean;
  /** Up Next plays after its 10 s countdown. */
  autoplayNext: boolean;
  reduceMotion: boolean;
  /** The compact readout over the video while the controls are hidden. */
  playbackDetails: boolean;
  /** The quality last picked ("original", "8m"…). */
  quality: string;
  /** Send TrueHD to the receiver as it is (the TV can't promise it). */
  passthroughTrueHd: boolean;
  /** Send DTS to the receiver as it is. */
  passthroughDts: boolean;
}

const DEFAULT_SETTINGS: Settings = {
  subtitleSize: "normal",
  subtitleBox: false,
  autoplayNext: true,
  reduceMotion: false,
  playbackDetails: false,
  quality: "original",
  passthroughTrueHd: false,
  passthroughDts: false,
};

const SERVER_KEY = "pley.server";
const SETTINGS_KEY = "pley.settings";
const HOME_KEY = "pley.home";
const HOUSEHOLD_KEY = "pley.household";
const SERVERS_KEY = "pley.servers";
const PROFILES_KEY = "pley.householdProfiles";

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
  /** Bumped by the server's events, so screens that list things fetch again. */
  readonly changes = signal(0);
  /** The server signed the TV out: show sign-in. */
  readonly signedOut = signal(false);
  /** Whether the TV shows the app (not another app, its home screen or standby). */
  readonly visible = signal(true);
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
  private readonly items = new Map<number, ItemDetail>();
  private libraryTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(platform: Platform, http: Transport = xhrTransport) {
    this.platform = platform;
    this.http = http;
    this.store = platform.storage;
    this.offsets = new OffsetStore(platform.storage);
    this.settings.value = { ...DEFAULT_SETTINGS, ...(readJson<Partial<Settings>>(this.store, SETTINGS_KEY) ?? {}) };
    const server = this.store.get(SERVER_KEY);
    if (server) this.use(server);
    this.home.value = readJson<Hub[]>(this.store, HOME_KEY);
  }

  /** The first screen, from what the TV kept. */
  start(): Start {
    const server = this.server.value;
    if (!server) return { page: "servers" };
    if (!this.tokens?.signedIn) return { page: "link", server };
    return this.householdSize() > 1 ? { page: "profiles" } : { page: "home" };
  }

  /** Signs in to `server` from now on (its tokens, if the TV has them). */
  use(server: string): void {
    const home = origin(server);
    if (!home) return;
    if (this.server.value === home && this.api.value) return;
    this.events?.stop();
    this.events = null;
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
    const list = readJson<SavedServer[]>(this.store, SERVERS_KEY) ?? [];
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

  /** How many profiles the household had when last asked (1 while unknown). */
  householdSize(): number {
    const n = Number(this.store.get(HOUSEHOLD_KEY) ?? "1");
    return n > 0 ? n : 1;
  }

  /** The household as last fetched, for "Who's watching?" before the server answers. */
  savedHousehold(): HouseholdProfile[] {
    return readJson<HouseholdProfile[]>(this.store, PROFILES_KEY) ?? [];
  }

  keepHousehold(profiles: HouseholdProfile[]): void {
    this.profiles.value = profiles;
    this.store.set(HOUSEHOLD_KEY, String(profiles.length));
    this.store.set(PROFILES_KEY, JSON.stringify(profiles));
  }

  /** The app acts as another profile now: what was shown for the one before goes. */
  async switched(): Promise<void> {
    this.store.remove(HOME_KEY);
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
    this.events?.stop();
    this.events = null;
    this.store.remove(SERVER_KEY);
    this.store.remove(HOME_KEY);
    this.store.remove(HOUSEHOLD_KEY);
    this.store.remove(PROFILES_KEY);
    this.server.value = null;
    this.api.value = null;
    this.tokens = null;
    this.home.value = null;
    this.me.value = null;
    this.libraries.value = null;
  }

  async signOut(): Promise<void> {
    this.events?.stop();
    await this.tokens?.signOut();
    this.store.remove(HOME_KEY);
    this.home.value = null;
    this.signedOut.value = true;
  }

  updateSettings(patch: Partial<Settings>): void {
    if ("passthroughTrueHd" in patch || "passthroughDts" in patch) this.capsPromise = null;
    this.settings.value = { ...this.settings.value, ...patch };
    this.store.set(SETTINGS_KEY, JSON.stringify(this.settings.value));
  }

  // ---- what the server has

  /** Who is watching, the libraries and the household, after sign-in or a profile switch. */
  async loadAccount(): Promise<void> {
    const api = this.api.value;
    if (!api) return;
    const [me, libraries] = await Promise.all([api.me(), api.libraries()]);
    this.me.value = me;
    this.libraries.value = libraries;
    api.profiles().then(
      (profiles) => this.keepHousehold(profiles),
      () => undefined,
    );
  }

  async loadHome(): Promise<void> {
    const api = this.api.value;
    if (!api) return;
    try {
      const { hubs } = await api.home();
      this.home.value = hubs;
      this.homeError.value = null;
      this.store.set(HOME_KEY, JSON.stringify(hubs));
    } catch (e) {
      this.homeError.value = e;
    }
  }

  /** Item details the app saw lately, to start a page or a playback without waiting. */
  cachedItem(id: number): ItemDetail | null {
    return this.items.get(id) ?? null;
  }

  async item(id: number): Promise<ItemDetail> {
    const api = this.api.value!;
    const detail = await api.item(id);
    this.items.delete(id);
    this.items.set(id, detail);
    if (this.items.size > 40) this.items.delete(this.items.keys().next().value!);
    return detail;
  }

  // ---- the TV's capabilities

  /** The capabilities, probed once (after the first screen is drawn), again after Settings change them. */
  caps(): Promise<Probe | null> {
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
    // Nothing is replayed: what is older than 30 s is fetched again.
    if (Date.now() - this.lastSync > 30_000) {
      this.lastSync = Date.now();
      void this.loadHome();
      this.changes.value++;
    }
  }

  private handleEvent(e: ServerEvent): void {
    switch (e.type) {
      case "library_changed":
        // Changes come in bursts during scans: once 1.5 s after the last.
        if (this.libraryTimer !== null) clearTimeout(this.libraryTimer);
        this.libraryTimer = setTimeout(() => {
          this.libraryTimer = null;
          void this.loadHome();
          this.changes.value++;
        }, 1_500);
        break;
      case "libraries_changed":
        void this.loadAccount();
        void this.loadHome();
        break;
      case "watchlist_changed":
        void this.loadHome();
        this.changes.value++;
        break;
      default:
        break;
    }
    for (const l of this.eventListeners.slice()) l(e);
  }
}
