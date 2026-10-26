// Times the app's list work in brs-node, the BrightScript engine: parsing a
// 300-title library page, mapping it to tiles, building their ContentNodes
// (with the engine's SceneGraph), Home's rows and the capability report.
// A computer runs the engine many times faster than a Roku runs
// BrightScript: these numbers compare versions of the code, they don't
// predict a device.
//
//   node scripts/bench.mjs

import { createRequire } from "node:module";
import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const brs = require("brs-node");
const sg = require("brs-node/bin/brs-sg.node.js");
brs.registerExtension(() => new sg.BrightScriptExtension());

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const state = (position_ms, watched) => ({ position_ms, watched, play_count: watched ? 1 : 0 });
const movie = (i) => ({
  id: 1000 + i, library_id: 1, kind: "movie", title: `Movie ${i + 1}`, sort_title: `Movie ${i + 1}`, year: 1990 + (i % 30),
  duration_ms: 6000000, added_at: 1700000000 + i, poster_url: `/api/items/${1000 + i}/images/poster?v=1729600000`,
  rating: 7.5, user_state: state(i % 5 === 0 ? 600000 : 0, i % 7 === 3),
});
const page = { total: 300, items: Array.from({ length: 300 }, (_, i) => movie(i)) };
const hubs = {
  hubs: Array.from({ length: 6 }, (_, h) => ({
    key: h === 0 ? "continue_watching" : `recently_added:${h}`, title: `Row ${h}`, library_id: h === 0 ? null : h,
    items: Array.from({ length: 20 }, (_, i) => movie(h * 20 + i)),
  })),
};

const main = `
sub Main()
    page = ReadAsciiFile("pkg:/data/page.json")
    hubs = ReadAsciiFile("pkg:/data/hubs.json")
    server = "http://192.168.1.20:7539"
    benchTime("parse 300 titles (ParseJson)", 5, function(input) : return ParseJson(input.page) : end function, { page: page })
    data = ParseJson(page)
    benchTime("map 300 titles to tiles", 5, function(input) : return contentTiles(input.items, input.server) : end function, { items: data.items, server: server })
    tiles = contentTiles(data.items, server)
    benchTime("build 300 ContentNodes", 5, function(input) : return nodesContent(input.tiles) : end function, { tiles: tiles })
    hubData = ParseJson(hubs)
    benchTime("Home rows (6 x 20) mapped and built", 5, function(input) : return nodesRows(contentHomeRows(input.hubs, invalid, input.server)) : end function, { hubs: hubData.hubs, server: server })
    benchTime("capability report", 5, function(input) : return capsBuild(fakeUltra()) : end function, {})
end sub

sub benchTime(name as string, runs as integer, work as function, input as object)
    times = []
    for i = 1 to runs
        clock = CreateObject("roTimespan")
        work(input)
        times.Push(clock.TotalMilliseconds())
    end for
    sorted = []
    for each t in times
        index = sorted.Count()
        for j = 0 to sorted.Count() - 1
            if t < sorted[j]
                index = j
                exit for
            end if
        end for
        before = []
        for j = 0 to sorted.Count() - 1
            if j = index then before.Push(t)
            before.Push(sorted[j])
        end for
        if index >= sorted.Count() then before.Push(t)
        sorted = before
    end for
    print "bench," + name + "," + sorted[Int(runs / 2)].ToStr() + "," + sorted[0].ToStr() + "," + sorted[runs - 1].ToStr()
end sub
`;

const files = new Map();
for (const name of readdirSync(join(root, "components/lib")).filter((n) => n.endsWith(".brs"))) {
  files.set(`source/lib/${name}`, new Blob([readFileSync(join(root, "components/lib", name))]));
}
files.set("source/fakedevice.brs", new Blob([readFileSync(join(root, "tests/fakedevice.brs"))]));
files.set("source/bench.brs", new Blob([main]));
files.set("data/page.json", new Blob([JSON.stringify(page)]));
files.set("data/hubs.json", new Blob([JSON.stringify(hubs)]));
files.set("manifest", new Blob([readFileSync(join(root, "manifest"))]));

const lines = [];
brs.registerCallback((message) => {
  if (typeof message === "string") lines.push(message);
});
const payload = await brs.createPayloadFromFileMap(files, {});
const result = await brs.executeFile(payload);

const rows = lines.filter((l) => l.startsWith("print,bench,")).map((l) => l.slice(12).trim().split(","));
if (rows.length === 0) {
  console.error(`the bench didn't run (${result?.exitReason}):`);
  for (const line of lines) console.error(line.trim());
  process.exit(1);
}
console.log(`brs-node ${require("brs-node/package.json").version}, Node ${process.version}; median of 5 runs (fastest-slowest), ms`);
for (const [name, median, fastest, slowest] of rows) console.log(`  ${name.padEnd(40)} ${median.padStart(5)}  (${fastest}-${slowest})`);
