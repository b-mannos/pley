#!/usr/bin/env bun
// pley's version, one for every app: VERSION at the repository's root.
//
//   bun scripts/version.mjs                 prints it
//   bun scripts/version.mjs set 0.3.0       writes it into every manifest
//   bun scripts/version.mjs check           fails when a manifest disagrees
//   bun scripts/version.mjs check v0.3.0    ...or when the tag does
//
// The Android app reads VERSION itself. Builds add the commit they were made
// from (scripts/commit.mjs), which every app shows beside the version.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (path) => readFileSync(join(root, path), "utf8");
const write = (path, text) => writeFileSync(join(root, path), text);
const SEMVER = /^\d+\.\d+\.\d+$/;

/** A manifest's version, found and replaced by a pattern whose second group is the version. */
function pattern(path, regex) {
  return {
    path,
    get: (text) => text.match(regex)?.[2],
    set: (text, v) => text.replace(regex, (_, before, _old, after) => `${before}${v}${after}`),
  };
}

const firstJsonVersion = (path) => pattern(path, /("version":\s*")([^"]+)(")/);
const cargoPackage = (path) => pattern(path, /(\[package\][^[]*?\nversion = ")([^"]+)(")/);
const cargoLock = (path, name) => pattern(path, new RegExp(`(\\[\\[package\\]\\]\\nname = "${name}"\\nversion = ")([^"]+)(")`));

const WORKSPACES = ["apps/web", "apps/desktop", "packages/tv", "apps/tizen", "apps/webos"];

/** Bun's lock records each workspace's version (not the root's), after its name. */
const bunLock = (path, workspace) =>
  pattern(path, new RegExp(`("${workspace}": \\{\\s*"name": "[^"]+",\\s*"version": ")([^"]+)(")`));

const roku = {
  path: "apps/roku/manifest",
  get(text) {
    const n = (key) => text.match(new RegExp(`^${key}=(\\d+)$`, "m"))?.[1];
    return `${n("major_version")}.${n("minor_version")}.${n("build_version")}`;
  },
  set(text, v) {
    const [major, minor, patch] = v.split(".");
    return text
      .replace(/^major_version=\d+$/m, `major_version=${major}`)
      .replace(/^minor_version=\d+$/m, `minor_version=${minor}`)
      .replace(/^build_version=\d+$/m, `build_version=${patch}`);
  },
};

const MANIFESTS = [
  firstJsonVersion("package.json"),
  ...WORKSPACES.map((w) => firstJsonVersion(`${w}/package.json`)),
  ...WORKSPACES.map((w) => bunLock("bun.lock", w)),
  firstJsonVersion("apps/roku/package.json"),
  roku,
  firstJsonVersion("apps/desktop/src-tauri/tauri.conf.json"),
  firstJsonVersion("apps/webos/appinfo.json"),
  pattern("apps/tizen/config.xml", /(<widget [^>]*?\bversion=")([^"]+)(")/),
  pattern("apps/kodi/plugin.video.pley/addon.xml", /(<addon [^>]*?\bversion=")([^"]+)(")/),
  cargoPackage("apps/server/Cargo.toml"),
  cargoPackage("apps/transcoder/Cargo.toml"),
  pattern("apps/transcoder/Cargo.toml", /(pley-transcoder-api = \{[^}]*\bversion = ")([^"]+)(")/),
  cargoPackage("packages/transcoder-api/Cargo.toml"),
  cargoPackage("apps/desktop/src-tauri/Cargo.toml"),
  cargoLock("Cargo.lock", "pley-server"),
  cargoLock("Cargo.lock", "pley-transcoder"),
  cargoLock("Cargo.lock", "pley-transcoder-api"),
  cargoLock("apps/desktop/src-tauri/Cargo.lock", "pley-desktop"),
];

const version = () => read("VERSION").trim();

function check(tag) {
  const want = version();
  const wrong = [];
  if (!SEMVER.test(want)) wrong.push(`VERSION: "${want}" is not major.minor.patch`);
  if (tag && tag !== `v${want}`) wrong.push(`the tag ${tag} is not v${want}`);
  for (const m of MANIFESTS) {
    const got = m.get(read(m.path));
    if (got !== want) wrong.push(`${m.path}: ${got ?? "no version"}`);
  }
  if (wrong.length) {
    console.error(`Pley ${want}, but:\n  ${wrong.join("\n  ")}\nRun: bun scripts/version.mjs set ${want}`);
    process.exit(1);
  }
  console.log(`Pley ${want}: every manifest agrees`);
}

function set(v) {
  if (!SEMVER.test(v ?? "")) {
    console.error("usage: bun scripts/version.mjs set <major.minor.patch>");
    process.exit(2);
  }
  write("VERSION", `${v}\n`);
  for (const m of MANIFESTS) {
    const before = read(m.path);
    const after = m.set(before, v);
    if (m.get(after) !== v) throw new Error(`${m.path}: the version wasn't found`);
    if (after !== before) write(m.path, after);
  }
  console.log(`Pley ${v}`);
}

const [command, arg] = process.argv.slice(2);
if (!command) console.log(version());
else if (command === "check") check(arg);
else if (command === "set") set(arg);
else {
  console.error("usage: bun scripts/version.mjs [check [vX.Y.Z] | set X.Y.Z]");
  process.exit(2);
}
