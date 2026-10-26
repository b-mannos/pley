import { displayAddress, normalizeUrl, type SavedServer } from "./servers";

/** A file the app downloaded from a server, or is downloading (src-tauri/src/downloads.rs). */
export interface Download {
  id: number;
  /** The server's origin. */
  server: string;
  name: string;
  size: number | null;
  received: number;
  state: "queued" | "downloading" | "waiting" | "failed" | "done";
  error: string | null;
}

/** `1.5 KB`, `150 MB`, `8.2 GB`: as the web app says sizes. */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

/** The name of the server a download came from, as saved (else its address). */
export function serverName(origin: string, saved: SavedServer[]): string {
  return saved.find((s) => normalizeUrl(s.url) === origin)?.name ?? displayAddress(origin);
}

/** Whether `d` is still to come. */
export const coming = (d: Download) => d.state === "queued" || d.state === "downloading" || d.state === "waiting";

/** How far `d` has come, 0 to 1 (`null` before its size is known). */
export const fraction = (d: Download) => (d.size ? Math.min(1, d.received / d.size) : null);

/** The line under a download's name. */
export function describe(d: Download, from: string): string {
  switch (d.state) {
    case "done":
      return `${formatBytes(d.size ?? d.received)} · ${from}`;
    case "downloading": {
      const part = fraction(d);
      return part == null
        ? `${formatBytes(d.received)} · ${from}`
        : `${Math.floor(part * 100)}% · ${formatBytes(d.received)} of ${formatBytes(d.size ?? 0)}`;
    }
    case "queued":
      return `Waiting its turn · ${from}`;
    case "waiting":
      return `Waiting for ${from} to answer`;
    case "failed":
      return d.error ?? "Couldn't download it.";
  }
}
