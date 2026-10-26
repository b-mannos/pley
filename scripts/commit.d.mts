/** The commit a build is made from: PLEY_COMMIT, else git's short hash, else "unknown". */
export function commit(): string;
/** pley's version (VERSION at the repository's root). */
export function version(): string;
