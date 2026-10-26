// Packs dist/ into out/pley.wgt with Samsung's Tizen CLI, signed with a
// security profile: PLEY_TIZEN_PROFILE (default "pley-dev"). A TV in
// developer mode, and Samsung's TV emulator, install only packages signed
// with a Samsung certificate (made in Tizen's Certificate Manager with a
// Samsung account); a generic Tizen author certificate packages it for
// everything else. The CLI is found on PATH, or in TIZEN_STUDIO.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync, statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dist = join(root, "dist");
const out = join(root, "out");
const profile = process.env.PLEY_TIZEN_PROFILE ?? "pley-dev";
const win = process.platform === "win32";
const studio = process.env.TIZEN_STUDIO;
const cli = studio ? join(studio, "tools", "ide", "bin", win ? "tizen.bat" : "tizen") : win ? "tizen.bat" : "tizen";
if (studio && !existsSync(cli)) {
  console.error(`No Tizen CLI at ${cli}`);
  process.exit(1);
}
mkdirSync(out, { recursive: true });
// A package from an earlier run would end up inside this one.
for (const f of readdirSync(dist)) if (f.endsWith(".wgt")) rmSync(join(dist, f));
const args = ["package", "-t", "wgt", "-s", profile, "--", dist];
// Windows runs the CLI's .bat through cmd.
if (win) execFileSync("cmd.exe", ["/d", "/c", cli].concat(args), { stdio: "inherit" });
else execFileSync(cli, args, { stdio: "inherit" });
// The CLI writes the package into dist/; it goes to out/ as pley.wgt.
const wgt = readdirSync(dist)
  .filter((f) => f.endsWith(".wgt"))
  .map((f) => join(dist, f))
  .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
if (!wgt) {
  console.error("The Tizen CLI made no .wgt");
  process.exit(1);
}
copyFileSync(wgt, join(out, "pley.wgt"));
console.log(`out/pley.wgt (signed with the "${profile}" profile)`);
