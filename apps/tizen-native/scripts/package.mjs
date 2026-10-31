// Builds the native Samsung app and signs it into out/pley-tizen.tpk with
// Samsung's Tizen CLI and a security profile: PLEY_TIZEN_PROFILE (default
// "pley-dev"), as the web app's package script does. A TV in developer mode
// installs only packages signed with a Samsung certificate. The .NET SDK
// builds the .tpk; the CLI, found on PATH or in TIZEN_STUDIO, signs it.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { commit } from "../../../scripts/commit.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const project = join(root, "src", "Pley.Tv.Tizen");
const bin = join(project, "bin", "Release");
const out = join(root, "out");
const profile = process.env.PLEY_TIZEN_PROFILE ?? "pley-dev";
const win = process.platform === "win32";
const studio = process.env.TIZEN_STUDIO;
const cli = studio ? join(studio, "tools", "ide", "bin", win ? "tizen.bat" : "tizen") : win ? "tizen.bat" : "tizen";
if (studio && !existsSync(cli)) {
  console.error(`No Tizen CLI at ${cli}`);
  process.exit(1);
}

// The commit goes into the informational version, which the app shows beside its version.
execFileSync("dotnet", ["build", project, "-c", "Release", "-nologo", `-p:SourceRevisionId=${commit()}`], { stdio: "inherit" });

const tpk = readdirSync(bin, { recursive: true })
  .filter((f) => String(f).endsWith(".tpk"))
  .map((f) => join(bin, String(f)))
  .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
if (!tpk) {
  console.error("The build made no .tpk");
  process.exit(1);
}

// The SDK signs with Tizen's default certificate; this signs it again with the profile's.
const args = ["package", "-t", "tpk", "-s", profile, "--", tpk];
// Windows runs the CLI's .bat through cmd.
if (win) execFileSync("cmd.exe", ["/d", "/c", cli].concat(args), { stdio: "inherit" });
else execFileSync(cli, args, { stdio: "inherit" });

mkdirSync(out, { recursive: true });
copyFileSync(tpk, join(out, "pley-tizen.tpk"));
console.log(`out/pley-tizen.tpk (signed with the "${profile}" profile)`);
