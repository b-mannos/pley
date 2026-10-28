// Builds pley for LG TVs into dist/: the app, its fonts, icons and launch
// picture (scripts/render-icons.mjs draws them), and appinfo.json with the
// package's version. `bun run package` makes the .ipk from it.

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
  // webOS 4 (2018) runs Chromium 53.
  target: "chrome53",
  version,
  minify: process.argv.indexOf("--dev") < 0,
  tsconfig: join(root, "tsconfig.json"),
  files: [
    { from: join(root, "assets/icon.png"), to: "icon.png" },
    { from: join(root, "assets/largeIcon.png"), to: "largeIcon.png" },
    { from: join(root, "assets/splash.png"), to: "splash.png" },
  ],
});
const appinfo = JSON.parse(readFileSync(join(root, "appinfo.json"), "utf8"));
appinfo.version = version;
writeFileSync(join(dist, "appinfo.json"), JSON.stringify(appinfo, null, 2) + "\n");
console.log(`pley for LG TV ${version}: dist/ (${(bytes / 1024).toFixed(0)} KB of script and styles)`);
