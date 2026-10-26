#!/usr/bin/env node
// Renders the icons the web app, the desktop app and the TV apps (Samsung,
// LG, Kodi, Roku) ship from the logo (apps/web/public/favicon.svg), through
// a headless Chrome or Edge:
//
//   apps/web/public/icon-maskable-512.png   full bleed, the P as big as the native
//                                           Android app's once Chrome masks it
//   apps/web/public/favicon-32.png          for browsers without SVG favicons
//   apps/desktop/src-tauri/icons/icon.ico   32 px first: Windows takes the first
//                                           for the window, the taskbar and Alt-Tab
//   apps/desktop/src-tauri/icons/icon.icns  on Apple's grid (an 824 plate in 1024,
//                                           a soft shadow), as macOS draws its apps
//   apps/tizen/assets/icon.png              Samsung's 512×423, full bleed, as a
//                                           24-bit RGB PNG at 72 dpi (Samsung's spec)
//   apps/webos/assets/icon.png, largeIcon.png
//                                           LG's 80 and 130 px, full bleed on the
//                                           tile's colour, which appinfo.json's
//                                           iconColor gives the TV's own tile
//   apps/webos/assets/splash.png            LG's launch picture, 1920×1080
//   apps/kodi/plugin.video.pley/resources/icon.png
//                                           512 square, the tile's colour out to the
//                                           corners: Kodi's add-on checker refuses
//                                           transparency
//   apps/kodi/plugin.video.pley/resources/fanart.jpg
//                                           1920×1080, the TV banner's picture
//   apps/roku/images/icon_focus_{fhd,hd}.png
//                                           540×405 and 290×218, Roku's home screen
//                                           tiles, as the Android TV app's banner
//   apps/roku/images/splash_{fhd,hd,sd}.png the logo on the Roku app's background
//
// Chrome comes from $CHROME, else the usual place on Windows, macOS or Linux.
// The banner's wordmark is in the system's bold sans (Roboto, else Segoe UI).
//
//   node scripts/render-icons.mjs

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32, deflateSync, inflateSync } from "node:zlib";

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

/** The P on the tile's colour to the edges of a width × height picture, the 64-unit logo at [s] with its top left at (x, y). */
function onTile(width, height, s, x, y) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}">${defs}<rect width="${width}" height="${height}" fill="${TILE}"/><g transform="translate(${x} ${y}) scale(${s})">${glyph}</g></svg>`;
}

/**
 * A TV home screen's tile, as the Android TV app's banner (320 × 180): the P in its gradient and the
 * wordmark on the tile's colour, the brand's deep blue glowing from the lower right corner. Other sizes
 * get the same picture scaled to fit, the colours out to the edges.
 */
function banner(width, height) {
  const s = Math.min(width / 320, height / 180);
  const g = 81 / 41; // the P 81 high, from (68, 54)
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}">${defs}
  <radialGradient id="corner" cx="${width}" cy="${height}" r="${width * 0.52}" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#0369a1" stop-opacity="0.3"/><stop offset="1" stop-color="#0369a1" stop-opacity="0"/>
  </radialGradient>
  <rect width="${width}" height="${height}" fill="${TILE}"/><rect width="${width}" height="${height}" fill="url(#corner)"/>
  <g transform="translate(${(width - 320 * s) / 2} ${(height - 180 * s) / 2}) scale(${s})">
    <g transform="translate(${68 - 17 * g} ${54 - 14 * g}) scale(${g})">${glyph}</g>
    <text x="160" y="107" font-family="Roboto, 'Segoe UI', system-ui, sans-serif" font-size="63" font-weight="700" letter-spacing="-1" fill="#e9ebef">pley</text>
  </g></svg>`;
}

/** A splash screen: the logo, a fifth of the height, in the middle of the app's background [bg]. */
function splash(width, height, bg) {
  const size = height / 5;
  const placed = logo.replace("<svg", `<svg x="${(width - size) / 2}" y="${(height - size) / 2}" width="${size}" height="${size}"`);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}"><rect width="${width}" height="${height}" fill="${bg}"/>${placed}</svg>`;
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

/** A headless browser page that turns an SVG into a PNG of exactly width × height, transparent around it. */
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

  /** [svg] drawn width × height, as a PNG, or a JPEG of [quality] (0 to 100). */
  async function capture(svg, width, height, quality) {
    await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false }, sessionId);
    const sized = svg.replace("<svg", `<svg width="${width}" height="${height}" style="display:block"`);
    await send("Page.setDocumentContent", { frameId: frameTree.frame.id, html: `<!doctype html><body style="margin:0">${sized}</body>` }, sessionId);
    const format = quality === undefined ? { format: "png" } : { format: "jpeg", quality };
    const { data } = await send("Page.captureScreenshot", { ...format, clip: { x: 0, y: 0, width, height, scale: 1 } }, sessionId);
    return Buffer.from(data, "base64");
  }

  return {
    png: (svg, width, height = width) => capture(svg, width, height),
    jpeg: (svg, width, height, quality) => capture(svg, width, height, quality),
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

/**
 * An opaque PNG from the browser (8-bit RGB or RGBA) again as a 24-bit RGB
 * PNG marked 72 dpi, as Samsung asks for its icons. Refuses a picture with
 * any pixel that isn't opaque, rather than dropping it.
 */
function rgb72(png) {
  let at = 8;
  let width = 0;
  let height = 0;
  let channels = 0;
  const data = [];
  while (at < png.length) {
    const length = png.readUInt32BE(at);
    const type = png.toString("ascii", at + 4, at + 8);
    const body = png.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      if (body[8] !== 8 || body[12] !== 0 || (body[9] !== 2 && body[9] !== 6)) throw new Error("expected an 8-bit RGB or RGBA PNG, not interlaced");
      channels = body[9] === 6 ? 4 : 3;
    }
    if (type === "IDAT") data.push(body);
    at += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(data));
  const stride = width * channels;
  const pixels = Buffer.alloc(stride * height);
  // Each row undoes its filter (PNG has five), from the rows above it.
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const left = x >= channels ? pixels[y * stride + x - channels] : 0;
      const up = y > 0 ? pixels[(y - 1) * stride + x] : 0;
      const corner = x >= channels && y > 0 ? pixels[(y - 1) * stride + x - channels] : 0;
      const p = left + up - corner;
      const [dl, du, dc] = [Math.abs(p - left), Math.abs(p - up), Math.abs(p - corner)];
      const paeth = dl <= du && dl <= dc ? left : du <= dc ? up : corner;
      const predicted = [0, left, up, (left + up) >> 1, paeth][filter];
      pixels[y * stride + x] = (raw[y * (stride + 1) + 1 + x] + predicted) & 255;
    }
  }
  if (channels === 4) {
    for (let i = 3; i < pixels.length; i += 4) if (pixels[i] !== 255) throw new Error("a picture meant to be opaque has see-through pixels");
  }
  // RGB rows, each as differences from the pixel on its left (filter 1), which suits flat colour.
  const rowBytes = width * 3 + 1;
  const rows = Buffer.alloc(rowBytes * height);
  for (let y = 0; y < height; y++) {
    rows[y * rowBytes] = 1;
    for (let x = 0; x < width; x++) {
      for (let c = 0; c < 3; c++) {
        const v = pixels[y * stride + x * channels + c];
        const left = x > 0 ? pixels[y * stride + (x - 1) * channels + c] : 0;
        rows[y * rowBytes + 1 + x * 3 + c] = (v - left) & 255;
      }
    }
  }
  const chunk = (type, body) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(body.length, 0);
    head.write(type, 4, "ascii");
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), body])), 0);
    return Buffer.concat([head, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  // 72 dpi: 2835 pixels a metre.
  const phys = Buffer.alloc(9);
  phys.writeUInt32BE(2835, 0);
  phys.writeUInt32BE(2835, 4);
  phys[8] = 1;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("pHYs", phys),
    chunk("IDAT", deflateSync(rows, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
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

  // Samsung's 512×423: the P where a 360 px logo centred in it puts it, the tile's colour to the edges.
  const tizen = join(root, "apps/tizen/assets");
  writeFileSync(join(tizen, "icon.png"), rgb72(await r.png(onTile(512, 423, 360 / 64, 76, 31.5), 512, 423)));

  // LG's: the logo at full size without its rounded corners, on the colour appinfo.json's iconColor
  // gives the TV's tile; and the launch picture with the P's own middle (33, 34.5 of the logo's 64)
  // in the middle, 205 px tall.
  const webos = join(root, "apps/webos/assets");
  writeFileSync(join(webos, "icon.png"), rgb72(await r.png(maskable(80, 80 / 64), 80)));
  writeFileSync(join(webos, "largeIcon.png"), rgb72(await r.png(maskable(130, 130 / 64), 130)));
  writeFileSync(join(webos, "splash.png"), rgb72(await r.png(onTile(1920, 1080, 5, 960 - 33 * 5, 540 - 34.5 * 5), 1920, 1080)));

  // Kodi's: the favicon's P where it is, without the rounded corners; the fanart a JPEG (Kodi's
  // checker takes up to 1000 KB).
  const kodi = join(root, "apps/kodi/plugin.video.pley/resources");
  writeFileSync(join(kodi, "icon.png"), rgb72(await r.png(maskable(512, 8), 512)));
  writeFileSync(join(kodi, "fanart.jpg"), await r.jpeg(banner(1920, 1080), 1920, 1080, 90));

  // Roku's home screen tiles, and splash screens on the app's background (components/shared/Ui.brs),
  // as the manifest's splash_color.
  const roku = join(root, "apps/roku/images");
  writeFileSync(join(roku, "icon_focus_fhd.png"), rgb72(await r.png(banner(540, 405), 540, 405)));
  writeFileSync(join(roku, "icon_focus_hd.png"), rgb72(await r.png(banner(290, 218), 290, 218)));
  for (const [name, width, height] of [["splash_fhd.png", 1920, 1080], ["splash_hd.png", 1280, 720], ["splash_sd.png", 720, 480]]) {
    writeFileSync(join(roku, name), rgb72(await r.png(splash(width, height, "#16191e"), width, height)));
  }
  console.log("rendered the web's maskable icon and PNG favicon, the desktop's .ico and .icns, the Samsung and LG apps' icons and LG's splash, Kodi's icon and fanart, and Roku's icons and splash screens");
} finally {
  r.close();
}
