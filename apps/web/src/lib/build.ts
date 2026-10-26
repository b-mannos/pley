declare const __PLEY_VERSION__: string;
declare const __PLEY_COMMIT__: string;

/** This web app's version and the commit it was built from (vite.config.ts). */
export const WEB_VERSION = __PLEY_VERSION__;
export const WEB_COMMIT = __PLEY_COMMIT__;

/** "0.2.0 (1a2b3c4d5)", or the version alone when the commit isn't known. */
export function versionLabel(version: string, commit?: string | null): string {
  return commit && commit !== "unknown" ? `${version} (${commit})` : version;
}
