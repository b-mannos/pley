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
    .map((s) => ({ url: s.url, name: s.name, id: s.id, saved: true, found: found.some((f) => sameServer(s, f)) }));
  for (const f of found) {
    if (!entries.some((e) => sameServer(e, f))) entries.push({ url: f.url, name: f.name, id: f.id, saved: false, found: true });
  }
  return entries;
}

/** The server to open by itself at start: the one opened last. */
export function lastUsed(saved: SavedServer[]): SavedServer | null {
  return saved.reduce<SavedServer | null>((best, s) => (!best || s.last_used > best.last_used ? s : best), null);
}
