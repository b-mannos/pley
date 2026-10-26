import { form, NetworkError, type Transport } from "../net/transport";
import { AuthError, SWITCH_PROFILE_GRANT } from "./metadata";

/** Where small things are kept between launches: localStorage on both TVs. */
export interface KeyValue {
  get(key: string): string | null;
  set(key: string, value: string): void;
  remove(key: string): void;
}

export const localStore: KeyValue = {
  get: (key) => {
    try {
      return window.localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  set: (key, value) => {
    try {
      window.localStorage.setItem(key, value);
    } catch {
      // Full or refused: the TV signs in again next time.
    }
  },
  remove: (key) => {
    try {
      window.localStorage.removeItem(key);
    } catch {
      // Nothing to forget.
    }
  },
};

/** The tokens of one server, as kept. */
export interface StoredTokens {
  issuer: string;
  accessToken: string;
  refreshToken: string | null;
  /** When the access token ends, in ms since the epoch. */
  expiresAt: number;
  scope: string;
  tokenEndpoint: string;
  revocationEndpoint: string;
}

/** A token endpoint's error (RFC 6749 §5.2), with pley's `reason` for profile switches. */
export interface OAuthError {
  error: string;
  description: string | null;
  reason: string | null;
}

export class TokenError extends AuthError {
  readonly status: number;
  readonly oauth: OAuthError | null;
  constructor(status: number, oauth: OAuthError | null) {
    super(oauth?.description || oauth?.error || `the server refused (HTTP ${status})`);
    this.name = "TokenError";
    this.status = status;
    this.oauth = oauth;
  }
}

export class SignedOutError extends AuthError {
  constructor() {
    super("you were signed out: sign in again");
    this.name = "SignedOutError";
  }
}

export const WIDER_SCOPE_MESSAGE = "the server allowed more than this app asks for: signed out";

/** How long before its end an access token is refreshed. */
const EARLY_MS = 60_000;
/** How long a refreshed token counts as just issued: refused within this, the sign-in is over. */
export const JUST_ISSUED_MS = 60_000;
/** RFC 6749 §5.1: without `expires_in`, an hour, pley's own. */
const DEFAULT_EXPIRES_IN = 3600;

export interface TokenResponse {
  accessToken: string;
  refreshToken: string | null;
  expiresIn: number;
  scope: string;
}

export function parseTokenResponse(body: unknown, askedScope: string): TokenResponse {
  const o = typeof body === "string" ? safeJson(body) : body;
  if (!o || typeof o !== "object") throw new AuthError("the server's answer can't be read: try again");
  const r = o as Record<string, unknown>;
  const access = r.access_token;
  if (typeof access !== "string" || !access) throw new AuthError("the server's answer can't be read: try again");
  if (typeof r.token_type === "string" && r.token_type.toLowerCase() !== "bearer") {
    throw new AuthError("the server's answer can't be read: try again");
  }
  return {
    accessToken: access,
    refreshToken: typeof r.refresh_token === "string" && r.refresh_token ? r.refresh_token : null,
    expiresIn: typeof r.expires_in === "number" && r.expires_in > 0 ? r.expires_in : DEFAULT_EXPIRES_IN,
    // RFC 6749 §5.1: a scope left out is the one asked for.
    scope: typeof r.scope === "string" ? r.scope : askedScope,
  };
}

export function parseOAuthError(body: unknown): OAuthError | null {
  const o = typeof body === "string" ? safeJson(body) : body;
  if (!o || typeof o !== "object") return null;
  const r = o as Record<string, unknown>;
  if (typeof r.error !== "string") return null;
  return {
    error: r.error,
    description: typeof r.error_description === "string" ? r.error_description : null,
    reason: typeof r.reason === "string" ? r.reason : null,
  };
}

function safeJson(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** The scope names in `answer` that `asked` doesn't hold. */
export function extraScopes(answer: string, asked: string): string[] {
  const have = asked.split(" ").filter(Boolean);
  return answer.split(" ").filter((s) => s && have.indexOf(s) < 0);
}

/** POSTs `fields` to a token endpoint; the answer's JSON, or a [TokenError]. */
export async function postForm(http: Transport, url: string, fields: [string, string][]): Promise<unknown> {
  const res = await http({
    method: "POST",
    url,
    headers: { "Content-Type": "application/x-www-form-urlencoded", Accept: "application/json" },
    body: form(fields),
    responseType: "json",
    timeoutMs: 15_000,
  });
  if (res.status < 200 || res.status >= 300) throw new TokenError(res.status, parseOAuthError(res.body));
  return res.body;
}

export function stored(answer: TokenResponse, issuer: string, tokenEndpoint: string, revocationEndpoint: string, now: number): StoredTokens {
  return {
    issuer,
    accessToken: answer.accessToken,
    refreshToken: answer.refreshToken,
    expiresAt: now + answer.expiresIn * 1000,
    scope: answer.scope,
    tokenEndpoint,
    revocationEndpoint,
  };
}

/** Why the TV is signed out. */
export type SignOutReason = "ended" | "wider_scope";

/**
 * The tokens of one server: the access token while it works, a new pair
 * from the refresh token when it doesn't. Refreshes happen one at a time
 * (the server ends the sign-in when a refresh token is used twice), and the
 * new refresh token is saved before the new access token is used.
 */
export class Tokens {
  private queue: Promise<unknown> = Promise.resolve();
  /** The access token the last refresh or profile switch gave, and when. */
  private lastIssued: { token: string; at: number } | null = null;

  readonly issuer: string;
  readonly clientId: string;
  private readonly store: KeyValue;
  private readonly http: Transport;
  /** Told when the server ends the sign-in, or the scope guard does. */
  private readonly onSignedOut: (reason: SignOutReason) => void;
  private readonly clock: () => number;

  constructor(
    issuer: string,
    clientId: string,
    store: KeyValue,
    http: Transport,
    onSignedOut: (reason: SignOutReason) => void = () => {},
    clock: () => number = Date.now,
  ) {
    this.issuer = issuer;
    this.clientId = clientId;
    this.store = store;
    this.http = http;
    this.onSignedOut = onSignedOut;
    this.clock = clock;
  }

  private get key(): string {
    return `pley.tokens.${this.issuer}`;
  }

  load(): StoredTokens | null {
    const text = this.store.get(this.key);
    if (!text) return null;
    const o = safeJson(text) as StoredTokens | null;
    return o && typeof o.accessToken === "string" && o.issuer === this.issuer ? o : null;
  }

  private save(tokens: StoredTokens): void {
    this.store.set(this.key, JSON.stringify(tokens));
  }

  private wipe(): void {
    this.store.remove(this.key);
  }

  get signedIn(): boolean {
    return this.load() !== null;
  }

  scopes(): string[] {
    return (this.load()?.scope ?? "").split(" ").filter(Boolean);
  }

  /** Runs `work` after every earlier refresh or switch: one at a time. */
  private serial<T>(work: () => Promise<T>): Promise<T> {
    const run = this.queue.then(work, work);
    this.queue = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

  /** The access token to send, refreshed when it ends within a minute; `null` when signed out. */
  async accessToken(): Promise<string | null> {
    const tokens = this.load();
    if (!tokens) return null;
    if (tokens.expiresAt - this.clock() > EARLY_MS) return tokens.accessToken;
    return this.refresh(tokens.accessToken);
  }

  /**
   * A new access token in place of `stale` (the one the server refused, or
   * that is about to end). When another caller refreshed meanwhile, its
   * token is used and nothing is sent.
   */
  refresh(stale: string | null): Promise<string> {
    return this.serial(async () => {
      const tokens = this.load();
      if (!tokens) throw new SignedOutError();
      if (tokens.accessToken !== stale && tokens.expiresAt - this.clock() > EARLY_MS) return tokens.accessToken;
      if (!tokens.refreshToken) this.signedOut();
      let answer: unknown;
      try {
        answer = await postForm(this.http, tokens.tokenEndpoint, [
          ["grant_type", "refresh_token"],
          ["refresh_token", tokens.refreshToken!],
          ["client_id", this.clientId],
        ]);
      } catch (e) {
        if (e instanceof TokenError && e.oauth?.error === "invalid_grant") this.signedOut();
        throw e;
      }
      const parsed = parseTokenResponse(answer, tokens.scope);
      const fresh = stored(parsed, this.issuer, tokens.tokenEndpoint, tokens.revocationEndpoint, this.clock());
      if (extraScopes(parsed.scope, tokens.scope).length) await this.widened(fresh);
      // A refresh token is good once: the new one is kept before anything else.
      this.save(fresh);
      this.lastIssued = { token: fresh.accessToken, at: this.clock() };
      return fresh.accessToken;
    });
  }

  /**
   * The server answered invalid_token to `sent`. When `sent` is the token
   * a refresh gave moments ago, another refresh won't do better: the
   * sign-in is over, and this answers true. False when `sent` is older.
   */
  refusedAfterRefresh(sent: string): Promise<boolean> {
    return this.serial(async () => {
      const last = this.lastIssued;
      if (!last || last.token !== sent || this.clock() - last.at > JUST_ISSUED_MS) return false;
      if (this.load()?.accessToken !== sent) return false;
      this.end("ended");
      return true;
    });
  }

  /**
   * Moves the sign-in to another profile of the household (the
   * switch-profile grant of pley's own apps), or checks the PIN of the one
   * it acts as. A wrong PIN changes nothing.
   */
  switchProfile(profile: number, pin: string | null): Promise<void> {
    return this.serial(async () => {
      const tokens = this.load();
      if (!tokens) throw new SignedOutError();
      if (!tokens.refreshToken) this.signedOut();
      const fields: [string, string][] = [
        ["grant_type", SWITCH_PROFILE_GRANT],
        ["client_id", this.clientId],
        ["refresh_token", tokens.refreshToken!],
        ["profile", String(profile)],
      ];
      if (pin !== null) fields.push(["pin", pin]);
      let answer: unknown;
      try {
        answer = await postForm(this.http, tokens.tokenEndpoint, fields);
      } catch (e) {
        // Without a reason the refresh token itself is spent or unknown.
        if (e instanceof TokenError && e.oauth?.error === "invalid_grant" && !e.oauth.reason) this.signedOut();
        throw e;
      }
      const parsed = parseTokenResponse(answer, tokens.scope);
      const fresh = stored(parsed, this.issuer, tokens.tokenEndpoint, tokens.revocationEndpoint, this.clock());
      if (extraScopes(parsed.scope, tokens.scope).length) await this.widened(fresh);
      this.save(fresh);
      this.lastIssued = { token: fresh.accessToken, at: this.clock() };
    });
  }

  /** Ends this app's sign-in on the server (RFC 7009), then forgets the tokens whatever it answered. */
  signOut(): Promise<void> {
    return this.serial(async () => {
      const tokens = this.load();
      try {
        if (tokens) await revoke(this.http, this.clientId, tokens);
      } finally {
        this.wipe();
      }
    });
  }

  /**
   * Keeps the tokens of a sign-in that just finished, after the scope
   * guard; the sign-in they replace on this server is ended.
   */
  install(fresh: StoredTokens, askedScope: string): Promise<void> {
    return this.serial(async () => {
      if (fresh.issuer !== this.issuer) throw new Error("tokens of another server");
      if (extraScopes(fresh.scope, askedScope).length) {
        await revoke(this.http, this.clientId, fresh);
        throw new AuthError(WIDER_SCOPE_MESSAGE);
      }
      const previous = this.load();
      this.save(fresh);
      if (previous && previous.refreshToken !== fresh.refreshToken) await revoke(this.http, this.clientId, previous);
    });
  }

  private signedOut(): never {
    this.end("ended");
    throw new SignedOutError();
  }

  private end(reason: SignOutReason): void {
    this.wipe();
    this.lastIssued = null;
    this.onSignedOut(reason);
  }

  /** A refresh or switch gave more than this sign-in holds: its grant is ended and the app signed out. */
  private async widened(fresh: StoredTokens): Promise<never> {
    await revoke(this.http, this.clientId, fresh);
    this.wipe();
    this.onSignedOut("wider_scope");
    throw new AuthError(WIDER_SCOPE_MESSAGE);
  }
}

/** Ends the grant of `tokens` on its server; trouble is ignored (the person can sign the TV out in Account → Apps). */
export async function revoke(http: Transport, clientId: string, tokens: StoredTokens): Promise<void> {
  // The refresh token ends the whole grant; the access token too, when there is no other.
  const token = tokens.refreshToken ?? tokens.accessToken;
  try {
    await postForm(http, tokens.revocationEndpoint, [
      ["token", token],
      ["client_id", clientId],
    ]);
  } catch (e) {
    if (!(e instanceof NetworkError) && !(e instanceof TokenError)) throw e;
  }
}
