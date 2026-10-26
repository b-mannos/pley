// Packs dist/ into out/app.pley.tv_<version>_all.ipk with LG's CLI
// (@webos-tools/cli). LG signs store builds itself; a TV in Developer Mode
// installs this one with `ares-install -d <tv> out/<file>.ipk`.

import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const out = join(root, "out");
mkdirSync(out, { recursive: true });
const bin = process.platform === "win32" ? "ares-package.cmd" : "ares-package";
execFileSync(bin, [join(root, "dist"), "--outdir", out, "--no-minify"], { stdio: "inherit", shell: process.platform === "win32" });
