// The NOTICES.txt a TV app's package carries: each package of others' work
// in it, with its license's text, as those licenses ask. build-app.mjs
// finds the packages (the bundle's inputs and the font) and reads their
// files; this puts the notice together.

import { THIRD_PARTY } from "../src/licenses.ts";

/** A package the app ships, with the text of its license. */
export interface Shipped {
  /** The npm package, "@preact/signals". */
  id: string;
  version: string;
  homepage: string | null;
  text: string;
}

/** Where a package keeps its license, by the names packages use. */
export const LICENSE_FILES = ["LICENSE", "LICENSE.md", "LICENSE.txt", "LICENCE", "LICENCE.md", "COPYING"];

/** The npm package a bundle input belongs to ("…/node_modules/@preact/signals/dist/x.mjs" → "@preact/signals"), else `null`. */
export function packageOf(input: string): string | null {
  const parts = input.replace(/\\/g, "/").split("/node_modules/");
  if (parts.length < 2) return null;
  const rest = parts[parts.length - 1]!.split("/");
  return rest[0]!.startsWith("@") ? `${rest[0]}/${rest[1]}` : rest[0]!;
}

const MIT = `Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;

/**
 * License texts of packages that publish none as a file: qrcode-generator
 * says "Licensed under the MIT license" in its source, with its copyright.
 */
export const TEXTS: Record<string, string> = {
  "qrcode-generator": `The MIT License (MIT)\n\nCopyright (c) 2009 Kazuhiko Arase\n\n${MIT}\n\nThe word "QR Code" is a registered trademark of DENSO WAVE INCORPORATED.`,
};

/** The notice for `app` ("pley for Samsung TV"): a list of what it ships, then each license in full. Stops on a package nobody named. */
export function noticesText(app: string, shipped: Shipped[]): string {
  const sorted = shipped.slice().sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  for (const p of sorted) {
    if (!THIRD_PARTY[p.id]) throw new Error(`${p.id} is in the bundle: name it in packages/tv/src/licenses.ts`);
    if (!p.text.trim()) throw new Error(`${p.id} has no license text: add it to TEXTS in packages/tv/build/notices.ts`);
  }
  const line = "-".repeat(72);
  const parts = [
    `${app} includes the following open-source software, each under its`,
    "license, copied in full below.",
    "",
    ...sorted.map((p) => `- ${THIRD_PARTY[p.id]!.name}: ${p.id} ${p.version} (${THIRD_PARTY[p.id]!.license})`),
  ];
  for (const p of sorted) {
    parts.push("", line, `${p.id} ${p.version}${p.homepage ? `, ${p.homepage}` : ""}`, line, "", p.text.replace(/\r\n/g, "\n").trim());
  }
  return parts.join("\n") + "\n";
}
