// Builds pley for Samsung TVs into dist/: the app, its fonts and icon
// (scripts/render-icons.mjs draws it), Samsung's webapis.js (which the TV
// serves itself), and config.xml with the package's version. `npm run
// package` makes the .wgt from it.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { buildApp, versionOf } from "../../../packages/tv/build/build-app.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = versionOf(join(root, "package.json"));
const dist = join(root, "dist");

const { bytes } = await buildApp({
  entry: join(root, "src/main.ts"),
  outdir: dist,
  // Tizen 4.0 (2018) runs Chromium 56.
  target: "chrome56",
  version,
  minify: process.argv.indexOf("--dev") < 0,
  tsconfig: join(root, "tsconfig.json"),
  // The TV's own Product API, at a path only the TV resolves.
  head: '<script src="$WEBAPIS/webapis/webapis.js"></script>',
  files: [{ from: join(root, "assets/icon.png"), to: "icon.png" }],
});
const config = readFileSync(join(root, "config.xml"), "utf8").replace(/(<widget[^>]*\sversion=")[^"]*"/, `$1${version}"`);
writeFileSync(join(dist, "config.xml"), config);
console.log(`pley for Samsung TV ${version}: dist/ (${(bytes / 1024).toFixed(0)} KB of script and styles)`);
