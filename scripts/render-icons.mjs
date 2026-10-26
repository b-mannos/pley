#!/usr/bin/env node
// Renders the icons the web app and the desktop app ship from the logo
// (apps/web/public/favicon.svg), through a headless Chrome or Edge:
//
//   apps/web/public/icon-maskable-512.png   full bleed, the P as big as the native
//                                           Android app's once Chrome masks it
//   apps/web/public/favicon-32.png          for browsers without SVG favicons
//   apps/desktop/src-tauri/icons/icon.ico   32 px first: Windows takes the first
//                                           for the window, the taskbar and Alt-Tab
//   apps/desktop/src-tauri/icons/icon.icns  on Apple's grid (an 824 plate in 1024,
//                                           a soft shadow), as macOS draws its apps
//
// Chrome comes from $CHROME, else the usual place on Windows, macOS or Linux.
//
//   node scripts/render-icons.mjs

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const logo = readFileSync(join(root, "apps/web/public/favicon.svg"), "utf8");
const defs = logo.match(/<defs>[\s\S]*<\/defs>/)[0];
const glyph = logo.match(/<path[\s\S]*?\/>/)[0];
const TILE = "#0a1622";

/** The P alone on the tile's colour to the edges, scaled [s] about the 64-unit logo's centre. */
function maskable(size, s) {
  const t = size / 2 - 32 * s;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}">${defs}<rect width="${size}" height="${size}" fill="${TILE}"/><g transform="translate(${t} ${t}) scale(${s})">${glyph}</g></svg>`;
}

/** Apple's macOS grid: the plate 824 of 1024 at (100, 100), corners about 185, a shadow 10 down. */
const MAC = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">${defs}
  <filter id="shadow" x="-10%" y="-10%" width="120%" height="125%"><feDropShadow dx="0" dy="10" stdDeviation="10" flood-color="#000" flood-opacity="0.3"/></filter>
  <g filter="url(#shadow)"><g transform="translate(100 100) scale(12.875)"><rect width="64" height="64" rx="14.4" fill="${TILE}"/>${glyph}</g></g></svg>`;

function chrome() {
  const candidates = [
    process.env.CHROME,
    "C:/Program Files/Google/Chrome/Application/chrome.exe",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ];
  const found = candidates.find((p) => p && existsSync(p));
  if (!found) throw new Error("No Chrome or Edge found: set CHROME to one.");
  return found;
}

/** A headless browser page that turns an SVG into a PNG of exactly size × size, transparent around it. */
async function renderer() {
  const profile = mkdtempSync(join(tmpdir(), "pley-icons-"));
  const browser = spawn(
    chrome(),
    ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars", "--force-color-profile=srgb", "about:blank"],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  const url = await new Promise((resolve, reject) => {
    let said = "";
    browser.stderr.on("data", (d) => {
      said += d;
      const m = said.match(/DevTools listening on (ws:\/\/\S+)/);
      if (m) resolve(m[1]);
    });
    browser.on("exit", (code) => reject(new Error(`the browser stopped (${code}): ${said}`)));
  });
  const socket = new WebSocket(url);
  await new Promise((resolve) => socket.addEventListener("open", resolve, { once: true }));
  let next = 0;
  const waiting = new Map();
  socket.addEventListener("message", (e) => {
    const msg = JSON.parse(e.data);
    const call = waiting.get(msg.id);
    if (!call) return;
    waiting.delete(msg.id);
    if (msg.error) call.reject(new Error(msg.error.message));
    else call.resolve(msg.result);
  });
  const send = (method, params = {}, sessionId) =>
    new Promise((resolve, reject) => {
      const id = ++next;
      waiting.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params, sessionId }));
    });

  const { targetId } = await send("Target.createTarget", { url: "about:blank" });
  const { sessionId } = await send("Target.attachToTarget", { targetId, flatten: true });
  await send("Emulation.setDefaultBackgroundColorOverride", { color: { r: 0, g: 0, b: 0, a: 0 } }, sessionId);
  const { frameTree } = await send("Page.getFrameTree", {}, sessionId);

  return {
    async png(svg, size) {
      await send("Emulation.setDeviceMetricsOverride", { width: size, height: size, deviceScaleFactor: 1, mobile: false }, sessionId);
      const sized = svg.replace("<svg", `<svg width="${size}" height="${size}" style="display:block"`);
      await send("Page.setDocumentContent", { frameId: frameTree.frame.id, html: `<!doctype html><body style="margin:0">${sized}</body>` }, sessionId);
      const { data } = await send("Page.captureScreenshot", { format: "png", clip: { x: 0, y: 0, width: size, height: size, scale: 1 } }, sessionId);
      return Buffer.from(data, "base64");
    },
    close() {
      socket.close();
      browser.kill();
      // The browser lets go of its profile a moment after it stops.
      setTimeout(() => rmSync(profile, { recursive: true, force: true }), 500);
    },
  };
}

/** A .ico of PNG entries, in the order given. */
function ico(images) {
  const head = Buffer.alloc(6 + 16 * images.length);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(images.length, 4);
  let offset = head.length;
  images.forEach(({ size, png }, i) => {
    const at = 6 + 16 * i;
    head.writeUInt8(size >= 256 ? 0 : size, at);
    head.writeUInt8(size >= 256 ? 0 : size, at + 1);
    head.writeUInt16LE(1, at + 4);
    head.writeUInt16LE(32, at + 6);
    head.writeUInt32LE(png.length, at + 8);
    head.writeUInt32LE(offset, at + 12);
    offset += png.length;
  });
  return Buffer.concat([head, ...images.map((i) => i.png)]);
}

/** A .icns of PNG chunks. */
function icns(chunks) {
  const parts = chunks.map(({ type, png }) => {
    const head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(png.length + 8, 4);
    return Buffer.concat([head, png]);
  });
  const body = Buffer.concat(parts);
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(body.length + 8, 4);
  return Buffer.concat([head, body]);
}

const r = await renderer();
try {
  const web = join(root, "apps/web/public");
  // Chrome fits the 80 % safe circle to Android's 66 dp safe zone: at 7.449 the P is the native app's 49 dp.
  writeFileSync(join(web, "icon-maskable-512.png"), await r.png(maskable(512, 7.449), 512));
  writeFileSync(join(web, "favicon-32.png"), await r.png(logo, 32));

  const icons = join(root, "apps/desktop/src-tauri/icons");
  const windows = [];
  for (const size of [32, 16, 20, 24, 40, 48, 64, 128, 256]) windows.push({ size, png: await r.png(logo, size) });
  writeFileSync(join(icons, "icon.ico"), ico(windows));

  const mac = {};
  for (const size of [16, 32, 64, 128, 256, 512, 1024]) mac[size] = await r.png(MAC, size);
  writeFileSync(
    join(icons, "icon.icns"),
    icns([
      { type: "icp4", png: mac[16] },
      { type: "icp5", png: mac[32] },
      { type: "icp6", png: mac[64] },
      { type: "ic07", png: mac[128] },
      { type: "ic08", png: mac[256] },
      { type: "ic09", png: mac[512] },
      { type: "ic10", png: mac[1024] },
      { type: "ic11", png: mac[32] },
      { type: "ic12", png: mac[64] },
      { type: "ic13", png: mac[256] },
      { type: "ic14", png: mac[512] },
    ]),
  );
  console.log("rendered the web's maskable icon and PNG favicon, and the desktop's .ico and .icns");
} finally {
  r.close();
}
