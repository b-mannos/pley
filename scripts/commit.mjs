// The commit a build is made from, which apps show beside their version:
// PLEY_COMMIT when set (builds without the repository, such as Docker's),
// else git's short hash, else "unknown".

import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { readFileSync } from "node:fs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

export function commit() {
  const given = process.env.PLEY_COMMIT?.trim();
  if (given) return given.slice(0, 9);
  try {
    return execFileSync("git", ["rev-parse", "--short=9", "HEAD"], { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return "unknown";
  }
}

/** pley's version (VERSION at the repository's root). */
export function version() {
  return readFileSync(join(root, "VERSION"), "utf8").trim();
}
