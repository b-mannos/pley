/** pley's web port, for addresses typed without one. */
export const DEFAULT_PORT = 7539;

/** A server the app has opened before (kept in its settings). */
export interface SavedServer {
  /** Where its web app is: `http://192.168.1.20:7539`. */
  url: string;
  name: string;
  /** Stable across addresses (the server's machine id), when known. */
  id: string | null;
  /** When it was last opened (Unix ms). */
  last_used: number;
}

/** A server that answered on the local network. */
export interface FoundServer {
  url: string;
  name: string;
  id: string | null;
  version: string;
}

/** What `/api/server` says. */
export interface ServerInfo {
  name: string;
  version: string;
  setup_required: boolean;
}

/** One row of the list: a saved server, a server found on the network, or both. */
export interface ServerEntry {
  url: string;
  name: string;
  id: string | null;
  saved: boolean;
  /** Answered the search on the local network. */
  found: boolean;
  /** Found, not saved, and named like a saved server it isn't: maybe an impostor. */
  lookalike: boolean;
}

/**
 * `nas`, `nas:8080`, `192.168.1.20` or `https://pley.example.com/` as a
 * server's address (its origin); `null` when it can't be one. Without a
 * scheme it's http, and without a port pley's.
 */
export function normalizeUrl(input: string): string | null {
  const text = input.trim();
  if (!text || /\s/.test(text)) return null;
  const hasScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(text);
  let url: URL;
  try {
    url = new URL(hasScheme ? text : `http://${text}`);
  } catch {
    return null;
  }
  if ((url.protocol !== "http:" && url.protocol !== "https:") || !url.hostname) return null;
  if (!hasScheme && !url.port) url.port = String(DEFAULT_PORT);
  return url.origin;
}

/**
 * Whether `hostname` is this computer or on its local network (a private
 * address, a name without dots, `.local`…) or a Tailscale name: where plain
 * http is how pley is reached, and the network itself is trusted.
 */
export function isLocalHost(hostname: string): boolean {
  const host = hostname.toLowerCase().replace(/^\[|\]$/g, "");
  if (host === "localhost" || (!host.includes(".") && !host.includes(":"))) return true;
  if (/\.(local|lan|home|internal|localdomain|home\.arpa|ts\.net)$/.test(host)) return true;
  const v4 = host.match(/^(\d+)\.(\d+)\.\d+\.\d+$/);
  if (v4) {
    const [a, b] = [Number(v4[1]), Number(v4[2])];
    return a === 10 || a === 127 || (a === 172 && b >= 16 && b < 32) || (a === 192 && b === 168) || (a === 169 && b === 254) || (a === 100 && b >= 64 && b < 128);
  }
  return host === "::1" || /^f[cd][0-9a-f]{0,2}:/.test(host) || /^fe[89ab][0-9a-f]?:/.test(host);
}

/** Whether a connection to `url` travels unencrypted beyond the local network: passwords and all. */
export function unencrypted(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === "http:" && !isLocalHost(u.hostname);
  } catch {
    return false;
  }
}

/**
 * The addresses to try, in turn, for what someone typed: a name on the
 * internet typed without `http://` is tried with https first (pley behind a
 * proxy with a certificate), then with plain http on pley's port.
 */
export function candidates(input: string): string[] {
  const url = normalizeUrl(input);
  if (!url) return [];
  const text = input.trim();
  const typedScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(text);
  const typedPort = /^[^/]*:\d+$/.test(text.split("/")[0] ?? "");
  const host = new URL(url).hostname;
  if (typedScheme || typedPort || isLocalHost(host)) return [url];
  return [`https://${host.includes(":") ? `[${host}]` : host}`, url];
}

/** The address to show for a server: its host and, unless it's the usual one, its port. */
export function displayAddress(url: string): string {
  try {
    const u = new URL(url);
    return u.protocol === "https:" || u.port !== String(DEFAULT_PORT) ? u.host : u.hostname;
  } catch {
    return url;
  }
}

function sameServer(a: { url: string; id: string | null }, b: { url: string; id: string | null }): boolean {
  return (!!a.id && a.id === b.id) || a.url === b.url;
}

/** Saved servers, most recently opened first, then the ones found on the network that aren't saved. */
export function mergeServers(saved: SavedServer[], found: FoundServer[]): ServerEntry[] {
  const entries: ServerEntry[] = [...saved]
    .sort((a, b) => b.last_used - a.last_used)
    .map((s) => ({
      url: s.url,
      name: s.name,
      id: s.id,
      saved: true,
      found: found.some((f) => sameServer(s, f)),
      lookalike: false,
    }));
  for (const f of found) {
    if (entries.some((e) => sameServer(e, f))) continue;
    const lookalike = saved.some((s) => s.name.trim().toLowerCase() === f.name.trim().toLowerCase());
    entries.push({ url: f.url, name: f.name, id: f.id, saved: false, found: true, lookalike });
  }
  return entries;
}

/** The server to open by itself at start: the one opened last. */
export function lastUsed(saved: SavedServer[]): SavedServer | null {
  return saved.reduce<SavedServer | null>((best, s) => (!best || s.last_used > best.last_used ? s : best), null);
}
