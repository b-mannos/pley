import { NetworkError, type Transport } from "../net/transport";
import { origin } from "./addresses";

/** A sign-in problem, in a few words for people. */
export class AuthError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "AuthError";
  }
}

/** What older servers answer: no app sign-in at all. */
/** The server didn't answer at all: worth trying again by itself. */
export class UnreachableError extends AuthError {
  constructor(message = "the server can't be reached: check its address and that it's on") {
    super(message);
    this.name = "UnreachableError";
  }
}

export const TOO_OLD = "this server is too old for this app";

/** RFC 8628's grant: a TV collects its tokens with the code it was given. */
export const DEVICE_CODE_GRANT = "urn:ietf:params:oauth:grant-type:device_code";

/** pley's own apps move between the household's profiles with this grant. */
export const SWITCH_PROFILE_GRANT = "urn:pley:params:oauth:grant-type:switch-profile";

/** What a server without sign-in with a code answers. */
export const NO_CODES = "this server can't sign in TVs with a code yet: update it";

/**
 * Where a server signs apps in (RFC 8414), read from
 * `<server>/.well-known/oauth-authorization-server`. Only kept when it is
 * this server's own: its issuer is the address the app used and every
 * endpoint is on it, so an answer can't send the app's codes or tokens
 * anywhere else.
 */
export interface ServerMetadata {
  issuer: string;
  tokenEndpoint: string;
  revocationEndpoint: string;
  deviceAuthorizationEndpoint: string | null;
  scopesSupported: string[] | null;
  grantTypesSupported: string[] | null;
}

export const METADATA_PATH = "/.well-known/oauth-authorization-server";

/** Whether a TV can sign in at this server with a code. */
export function signsInWithCodes(m: ServerMetadata): boolean {
  return !!m.deviceAuthorizationEndpoint && (m.grantTypesSupported ?? []).indexOf(DEVICE_CODE_GRANT) >= 0;
}

export async function fetchMetadata(http: Transport, server: string): Promise<ServerMetadata> {
  const home = origin(server);
  if (!home) throw new AuthError("that isn't a server address");
  let res;
  try {
    res = await http({ method: "GET", url: home + METADATA_PATH, headers: { Accept: "application/json" }, timeoutMs: 8000 });
  } catch (e) {
    if (e instanceof NetworkError) throw new UnreachableError();
    throw e;
  }
  if (res.status === 404) throw new AuthError(TOO_OLD);
  if (res.status < 200 || res.status >= 300) throw new AuthError(`the server couldn't answer (HTTP ${res.status})`);
  const type = (res.header("Content-Type") ?? "").toLowerCase();
  // Older servers answer their web app's page here.
  if (type.indexOf("json") < 0) throw new AuthError(TOO_OLD);
  return parseMetadata(String(res.body), home);
}

/** The checks of [fetchMetadata], on the answer `text` of the server at `server`. */
export function parseMetadata(text: string, server: string): ServerMetadata {
  const home = origin(server);
  if (!home) throw new AuthError("that isn't a server address");
  let o: Record<string, unknown>;
  try {
    o = JSON.parse(text) as Record<string, unknown>;
  } catch {
    throw new AuthError(TOO_OLD);
  }
  if (!o || typeof o !== "object") throw new AuthError(TOO_OLD);
  const issuer = typeof o.issuer === "string" ? o.issuer : "";
  if (!issuer) throw new AuthError(TOO_OLD);
  // RFC 8414 §3.3: the issuer is exactly the address asked at.
  if (issuer !== home) {
    throw new AuthError(
      `this server calls itself ${issuer.slice(0, 100)}: open it by that address, or check the proxy in front of it passes Host`,
    );
  }
  const strings = (v: unknown): string[] | null =>
    Array.isArray(v) ? v.filter((s): s is string => typeof s === "string" && s.length > 0) : null;
  const endpoint = (name: string): string => {
    const url = o[name];
    if (typeof url !== "string" || !url) throw new AuthError(TOO_OLD);
    if (origin(url) !== issuer || url.indexOf(`${issuer}/`) !== 0) {
      throw new AuthError("this server sends apps to another address to sign in: Pley doesn't");
    }
    return url;
  };
  const device = typeof o.device_authorization_endpoint === "string" && o.device_authorization_endpoint
    ? endpoint("device_authorization_endpoint")
    : null;
  return {
    issuer,
    tokenEndpoint: endpoint("token_endpoint"),
    revocationEndpoint: endpoint("revocation_endpoint"),
    deviceAuthorizationEndpoint: device,
    scopesSupported: strings(o.scopes_supported),
    grantTypesSupported: strings(o.grant_types_supported),
  };
}
