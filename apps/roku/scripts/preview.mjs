// Runs the app in brs-node and shows it in a browser: the TV's picture as
// brs-node draws it, and the keyboard or the buttons under it as the
// remote. Against the fake pley server of the smoke run (it allows the TV
// code by itself), or a real server with --server, whose /link page allows
// it. brs-node is a simulator: it plays no video, draws with its own fonts
// and isn't a Roku's speed.
//
//   bun scripts/preview.mjs [--port 8086] [--server http://192.168.1.5:7539]

import { createServer } from "node:http";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { startFixture } from "./fixture-server.mjs";

const require = createRequire(import.meta.url);
const brs = require("brs-node");
const { createCanvas } = createRequire(require.resolve("brs-node"))("canvas");

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const option = (name, fallback) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const port = Number(option("--port", "8086"));

// The picture: brs-node's last frame, made into a JPEG when the page asks
// for a newer one than it has.
let frame = null;
let frameNumber = 0;
const waiting = new Set();
brs.subscribeHost("preview", (event, data) => {
  if (event === "frame") {
    frame = data;
    frameNumber += 1;
    for (const wake of waiting) wake();
    waiting.clear();
  } else if (event === "message" && typeof data === "string") {
    const [kind, ...rest] = data.split(",");
    if (kind === "print" || kind === "error" || kind === "warning") process.stdout.write(rest.join(","));
  }
});
const WIDTH = 1280;
const HEIGHT = 720;
const full = { canvas: null };
function jpeg() {
  if (!full.canvas || full.canvas.width !== frame.width || full.canvas.height !== frame.height) {
    full.canvas = createCanvas(frame.width, frame.height);
  }
  full.canvas.getContext("2d").putImageData(frame, 0, 0);
  const scaled = createCanvas(WIDTH, HEIGHT);
  scaled.getContext("2d").drawImage(full.canvas, 0, 0, WIDTH, HEIGHT);
  return scaled.toBuffer("image/jpeg", { quality: 0.85 });
}

// The remote, as the smoke run presses it: brs-node's key codes in a shared
// buffer, a key's release being its code plus 100.
const KEYS = {
  back: 0, up: 2, down: 3, left: 4, right: 5, select: 6, instantreplay: 7, rev: 8, fwd: 9,
  info: 10, backspace: 11, play: 13, enter: 15,
};
const shared = new SharedArrayBuffer(Int32Array.BYTES_PER_ELEMENT * (brs.DataBufferIndex + brs.DataBufferSize));
const control = new Int32Array(shared).fill(-1);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function send(code, mod) {
  const until = Date.now() + 10000;
  while (Date.now() < until) {
    for (let slot = 0; slot < brs.KeyArraySpots; slot++) {
      const at = slot * brs.KeyBufferSize;
      if (Atomics.load(control, brs.DataType.KEY + at) < 0) {
        Atomics.store(control, brs.DataType.RID + at, brs.RemoteType.SIM);
        Atomics.store(control, brs.DataType.MOD + at, mod);
        Atomics.store(control, brs.DataType.KEY + at, code + mod);
        return;
      }
    }
    await sleep(10);
  }
}
// Presses go one after another, in the order they came.
let pressing = Promise.resolve();
function press(name) {
  const code = name.startsWith("lit_") ? name.codePointAt(4) : KEYS[name];
  if (code === undefined) return false;
  pressing = pressing.then(async () => {
    await send(code, 0);
    await sleep(60);
    await send(code, 100);
  });
  return true;
}

// The server the app opens on, already chosen, so it starts at sign-in.
let serverUrl = option("--server", "");
let serverName = "pley";
let fixture = null;
if (serverUrl) {
  serverUrl = serverUrl.replace(/\/+$/, "");
  try {
    const answer = await fetch(`${serverUrl}/api/server`);
    serverName = (await answer.json()).name || serverName;
  } catch {
    console.log(`${serverUrl} didn't answer yet; the app will say so.`);
  }
} else {
  fixture = await startFixture();
  serverUrl = fixture.issuer;
  serverName = "Preview";
}
const registry = new Map([
  ["preview.servers.list", JSON.stringify([{ url: serverUrl, name: serverName, id: "preview", last_used: 1 }])],
  ["preview.servers.current", serverUrl],
]);
const payload = await brs.createPayloadFromFiles([], { developerId: "preview", displayMode: "1080p", registry }, undefined, root);
payload.extensions = [brs.SupportedExtension.SceneGraph];
payload.device.extensions = new Map([[brs.SupportedExtension.SceneGraph, "brs-sg.node.js"]]);
brs.executeApp(payload, { sharedBuffer: shared }).then(() => {
  console.log("The app ended (Home on the remote, or an error); start the preview again.");
});

const page = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>pley for Roku</title>
<style>
  :root { color-scheme: dark; --bg: #0e1013; --panel: #1b1e24; --line: #2c313a; --text: #e8eaee; --muted: #9aa1ad; --accent: #38bdf8; }
  * { box-sizing: border-box; }
  body { margin: 0; background: var(--bg); color: var(--text); font: 14px/1.4 system-ui, sans-serif; }
  main { max-width: 1280px; margin: 0 auto; padding: 16px; }
  .tv { position: relative; aspect-ratio: 16 / 9; background: #000; border: 1px solid var(--line); border-radius: 6px; overflow: hidden; outline: none; }
  .tv:focus-visible { border-color: var(--accent); }
  .tv img { display: block; width: 100%; height: 100%; }
  .tv p { position: absolute; inset: 0; display: grid; place-items: center; margin: 0; color: var(--muted); }
  .tv p[hidden] { display: none; }
  .remote { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; align-items: center; }
  button { background: var(--panel); color: var(--text); border: 1px solid var(--line); border-radius: 6px; padding: 8px 12px; font: inherit; cursor: pointer; min-width: 44px; }
  button:hover { border-color: var(--accent); }
  button.pressed { background: var(--accent); color: #000; }
  .gap { width: 12px; }
  .hint { margin-top: 10px; color: var(--muted); }
  kbd { border: 1px solid var(--line); border-radius: 4px; padding: 0 4px; font: 12px ui-monospace, monospace; }
</style>
</head>
<body>
<main>
  <div class="tv" id="tv" tabindex="0" aria-label="The Roku's screen: use the keyboard as the remote">
    <img id="screen" alt="">
    <p id="wait">Starting the app in brs-node…</p>
  </div>
  <div class="remote" id="remote">
    <button data-key="back">Back</button>
    <span class="gap"></span>
    <button data-key="left">◀</button>
    <button data-key="up">▲</button>
    <button data-key="select">OK</button>
    <button data-key="down">▼</button>
    <button data-key="right">▶</button>
    <span class="gap"></span>
    <button data-key="instantreplay">Replay</button>
    <button data-key="info">Options</button>
    <span class="gap"></span>
    <button data-key="rev">Rewind</button>
    <button data-key="play">Play/Pause</button>
    <button data-key="fwd">Fast forward</button>
  </div>
  <p class="hint">Click the screen, then: arrows to move, <kbd>Enter</kbd> OK, <kbd>Esc</kbd> or <kbd>Backspace</kbd> Back,
  <kbd>Space</kbd> play/pause, <kbd>[</kbd> <kbd>]</kbd> rewind and fast forward, <kbd>*</kbd> options; letters and digits type on a keyboard.
  ${fixture ? "Server: the smoke run's fake pley server, which allows the TV code by itself." : "Server: " + serverUrl.replace(/[<&>]/g, "") + ": allow the code on its /link page."}</p>
</main>
<script>
  const screen = document.getElementById("screen");
  const wait = document.getElementById("wait");
  const tv = document.getElementById("tv");
  let shown = 0;
  async function frames() {
    for (;;) {
      try {
        const answer = await fetch("/frame?after=" + shown, { cache: "no-store" });
        if (answer.status === 200) {
          shown = Number(answer.headers.get("X-Frame")) || shown;
          const url = URL.createObjectURL(await answer.blob());
          const old = screen.src;
          screen.src = url;
          if (old) URL.revokeObjectURL(old);
          wait.hidden = true;
        }
      } catch {
        wait.hidden = false;
        wait.textContent = "The preview stopped: start it again.";
        await new Promise((r) => setTimeout(r, 2000));
      }
    }
  }
  function press(key) {
    fetch("/key/" + encodeURIComponent(key), { method: "POST" });
    const button = document.querySelector('[data-key="' + key + '"]');
    if (button) {
      button.classList.add("pressed");
      setTimeout(() => button.classList.remove("pressed"), 150);
    }
  }
  const KEYBOARD = {
    ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right", Enter: "select",
    Escape: "back", Backspace: "back", " ": "play", "[": "rev", "]": "fwd", "*": "info",
  };
  document.addEventListener("keydown", (event) => {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const key = KEYBOARD[event.key] || (event.key.length === 1 ? "lit_" + event.key : null);
    if (!key) return;
    event.preventDefault();
    press(key);
  });
  document.getElementById("remote").addEventListener("click", (event) => {
    const key = event.target.closest("button")?.dataset.key;
    if (key) {
      press(key);
      tv.focus();
    }
  });
  tv.focus();
  frames();
</script>
</body>
</html>`;

createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  if (req.method === "GET" && url.pathname === "/") {
    res.writeHead(200, { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" });
    return res.end(page);
  }
  if (req.method === "GET" && url.pathname === "/frame") {
    const after = Number(url.searchParams.get("after")) || 0;
    const answer = () => {
      if (!frame || frameNumber <= after) {
        res.writeHead(204, { "Cache-Control": "no-store" });
        return res.end();
      }
      res.writeHead(200, { "Content-Type": "image/jpeg", "Cache-Control": "no-store", "X-Frame": String(frameNumber) });
      return res.end(jpeg());
    };
    if (frame && frameNumber > after) return answer();
    // A new frame, or two seconds: whichever comes first.
    const timer = setTimeout(() => {
      waiting.delete(wake);
      answer();
    }, 2000);
    const wake = () => {
      clearTimeout(timer);
      answer();
    };
    waiting.add(wake);
    return;
  }
  if (req.method === "POST" && url.pathname.startsWith("/key/")) {
    const ok = press(decodeURIComponent(url.pathname.slice(5)));
    res.writeHead(ok ? 204 : 400);
    return res.end();
  }
  res.writeHead(404);
  res.end();
}).listen(port, "127.0.0.1", () => {
  console.log(`pley for Roku, in brs-node: http://127.0.0.1:${port}/ (server: ${serverUrl})`);
});
