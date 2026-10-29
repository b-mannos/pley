#!/usr/bin/env bun
// Prepares a unit's release (docs/RELEASING.md): bumps its version in every
// manifest and moves its CHANGELOG's Unreleased entries under the new
// version. The Release workflow, run by hand on GitHub, then builds the unit,
// tags it <unit>-v<version> and publishes the release with those notes.
//
//   bun scripts/release.mjs                                  each unit's version, last tag and what waits
//   bun scripts/release.mjs <unit> <patch|minor|major|X.Y.Z>  bump the unit and date its Unreleased entries
//   bun scripts/release.mjs <unit> draft                      add its commits since its last tag to Unreleased
//   bun scripts/release.mjs <unit> notes [X.Y.Z]              print a version's notes (default: the current one)
//
//   --dry-run      show what would change and write nothing
//   --draft        with a bump: add the commits since the last tag to Unreleased first
//   --since <ref>  draft from this commit or tag rather than the unit's last tag
//   --date <date>  the release's date, YYYY-MM-DD (default: today)
//   --commit       commit the files changed, as "<unit>: release X.Y.Z"
//
// Units: server, desktop, android, samsung, lg, kodi, roku (scripts/version.mjs).

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { addEntries, entryOf, findSection, notes, plainText, release } from "./changelog.mjs";
import {
  REPOSITORY,
  ROOT,
  UNITS,
  UNIT_NAMES,
  UsageError,
  androidVersionCode,
  bump,
  compare,
  planSet,
  problems,
  tagOf,
  unitOf,
  versionOf,
} from "./version.mjs";

const color = process.stdout.isTTY && !process.env.NO_COLOR;
const paint = (code) => (s) => (color ? `\x1b[${code}m${s}\x1b[0m` : String(s));
const bold = paint(1);
const dim = paint(2);
const green = paint(32);
const yellow = paint(33);

function git(args, root = ROOT) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
}

/** The unit's last release tag reachable from HEAD, or null. */
export function lastTag(unit, root = ROOT) {
  try {
    return git(["describe", "--tags", "--abbrev=0", "--match", `${unit}-v*`, "HEAD"], root) || null;
  } catch {
    return null;
  }
}

/** Subjects of the commits since `since` that touch the unit's files, oldest first. */
export function commitsSince(unit, since, root = ROOT) {
  const out = git(["log", "--no-merges", "--reverse", "--format=%s", `${since}..HEAD`, "--", ...UNITS[unit].paths], root);
  return out ? out.split("\n") : [];
}

const tagExists = (tag, root) => {
  try {
    git(["rev-parse", "-q", "--verify", `refs/tags/${tag}`], root);
    return true;
  } catch {
    return false;
  }
};

const today = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};

const read = (root, path) => readFileSync(join(root, path), "utf8");

/** The commits to draft from: since `since`, else the unit's last tag. */
function draftSource(unit, since, root) {
  const from = since ?? lastTag(unit, root);
  if (!from) {
    throw new UsageError(
      `${unit} has no ${unit}-v* tag to draft from yet (git fetch --tags brings GitHub's): pass --since <commit or tag>`,
    );
  }
  try {
    git(["rev-parse", "--verify", "-q", `${from}^{commit}`], root);
  } catch {
    throw new UsageError(`--since ${from}: no such commit or tag`);
  }
  return from;
}

/** Unreleased with the commits since `from` added. */
export function draft(unit, text, from, root = ROOT) {
  const subjects = commitsSince(unit, from, root);
  const entries = subjects.map(entryOf).filter(Boolean);
  return { ...addEntries(text, entries), commits: subjects.length };
}

/** Kodi shows the add-on's <news> as its changelog: the new version's notes, as plain text. */
function kodiNews(addonXml, version, date, body) {
  const escape = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  let news = plainText(version, date, body);
  if (news.length > 1500) news = `${news.slice(0, 1497).replace(/\s+\S*$/, "")}...`;
  if (!/<news>[\s\S]*?<\/news>/.test(addonXml)) throw new Error("addon.xml has no <news> element");
  return addonXml.replace(/<news>[\s\S]*?<\/news>/, () => `<news>${escape(news)}</news>`);
}

/**
 * What releasing the unit changes, nothing written: { from, to, tag, date,
 * drafted, changes: [{ path, before, after, what }], notes }.
 */
export function plan({ unit, how, date = today(), withDraft = false, since = null, root = ROOT }) {
  const { changelog } = unitOf(unit);
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new UsageError(`--date ${date}: not YYYY-MM-DD`);
  const wrong = problems(unit, root);
  if (wrong.length) throw new UsageError(`${unit}'s versions disagree; fix them first:\n  ${wrong.join("\n  ")}`);
  const from = versionOf(unit, root);
  const to = bump(from, how);
  if (compare(to, from) <= 0) throw new UsageError(`${unit} is at ${from}: ${to} isn't above it`);
  const tag = tagOf(unit, to);
  if (tagExists(tag, root)) throw new UsageError(`the tag ${tag} exists already`);

  const before = read(root, changelog);
  let text = before;
  let drafted = null;
  if (withDraft) {
    const from = draftSource(unit, since, root);
    drafted = { ...draft(unit, text, from, root), from };
    text = drafted.text;
  }
  if (!findSection(text, "Unreleased")?.body) {
    const why = withDraft ? `, and none of the ${drafted.commits} commits since ${drafted.from} gave an entry` : "";
    const hint = withDraft ? "" : ", or pass --draft to start from the commits";
    throw new UsageError(`Unreleased in ${changelog} is empty${why}: write what changed there${hint}`);
  }
  text = release(text, { version: to, date, tag, repository: REPOSITORY });
  const body = notes(text, to);

  const changes = planSet(unit, to, root).map((c) => ({ ...c, what: `version ${from} -> ${to}` }));
  changes.push({ path: changelog, before, after: text, what: `Unreleased -> ${to} (${date})` });
  if (unit === "kodi") {
    const c = changes.find((x) => x.path.endsWith("addon.xml"));
    c.after = kodiNews(c.after, to, date, body);
    c.what += ", <news>";
  }
  return { unit, from, to, tag, date, drafted, changes, notes: body };
}

function commit(unit, version, paths, root) {
  git(["add", "--", ...paths], root);
  git(["commit", "-q", "-m", `${unit}: release ${version}`, "--", ...paths], root);
  return git(["rev-parse", "--short", "HEAD"], root);
}

function printChanges(changes) {
  const width = Math.max(...changes.map((c) => c.path.length));
  for (const c of changes) console.log(`  ${c.path.padEnd(width)}  ${dim(c.what)}`);
}

function indent(text) {
  return text
    .split("\n")
    .map((l) => (l ? `    ${l}` : ""))
    .join("\n");
}

function releaseCommand(unit, how, opts) {
  const p = plan({ unit, how, date: opts.date, withDraft: opts.draft, since: opts.since, root: ROOT });
  // Only the CHANGELOG may hold uncommitted edits (the entries just written).
  if (opts.commit && !opts.dryRun) {
    const dirty = git(["status", "--porcelain", "--", ...p.changes.map((c) => c.path).filter((x) => x !== UNITS[unit].changelog)]);
    if (dirty) throw new UsageError(`--commit: these have uncommitted changes; commit or set them aside first:\n${dirty}`);
  }
  console.log(`${bold(UNITS[unit].title)} ${p.from} -> ${bold(green(p.to))}  ${dim(`(tag ${p.tag} once released)`)}`);
  if (unit === "android") console.log(dim(`  Android version code ${androidVersionCode(p.from)} -> ${androidVersionCode(p.to)}`));
  if (p.drafted) {
    console.log(
      p.drafted.added.length
        ? `  drafted ${p.drafted.added.length} entr${p.drafted.added.length === 1 ? "y" : "ies"} from ${p.drafted.commits} commit${p.drafted.commits === 1 ? "" : "s"} under "Changed"`
        : dim(`  nothing new to draft (${p.drafted.commits} commit${p.drafted.commits === 1 ? "" : "s"} since the last tag)`),
    );
  }
  console.log("");
  printChanges(p.changes);
  console.log(`\n  ${bold(`## [${p.to}] - ${p.date}`)}\n\n${indent(p.notes)}\n`);
  if (opts.dryRun) {
    console.log(yellow("Dry run: nothing was written."));
    return;
  }
  for (const c of p.changes) writeFileSync(join(ROOT, c.path), c.after);
  const paths = p.changes.map((c) => c.path);
  if (opts.commit) {
    const hash = commit(unit, p.to, paths, ROOT);
    console.log(`${green("Committed")} ${hash} "${unit}: release ${p.to}".`);
    console.log(`Next: push it, then on GitHub: Actions -> Release -> Run workflow -> ${unit}.`);
  } else {
    console.log(`${green("Written.")} Review the entries${p.drafted?.added.length ? " (the drafted ones especially)" : ""}, then commit:`);
    console.log(`  git commit -m "${unit}: release ${p.to}" -- ${paths.join(" ")}`);
    console.log(`Then push, and on GitHub: Actions -> Release -> Run workflow -> ${unit}.`);
  }
}

function draftCommand(unit, opts) {
  const { changelog } = unitOf(unit);
  const from = draftSource(unit, opts.since, ROOT);
  const before = read(ROOT, changelog);
  const d = draft(unit, before, from, ROOT);
  console.log(`${bold(UNITS[unit].title)}: ${d.commits} commit${d.commits === 1 ? "" : "s"} since ${from}`);
  if (!d.added.length) {
    console.log(dim("  nothing new for Unreleased"));
    return;
  }
  console.log(`\n${d.added.map((e) => `  - ${e}`).join("\n")}\n`);
  if (opts.dryRun) {
    console.log(yellow(`Dry run: ${changelog} wasn't written.`));
    return;
  }
  writeFileSync(join(ROOT, changelog), d.text);
  console.log(`${green("Added")} to Unreleased in ${changelog}, under "Changed": sort them into Added, Fixed and the rest, and edit them.`);
}

function notesCommand(unit, version) {
  const { changelog } = unitOf(unit);
  const v = version ?? versionOf(unit);
  const path = join(ROOT, changelog);
  if (!existsSync(path)) throw new UsageError(`${changelog} is missing`);
  try {
    process.stdout.write(`${notes(readFileSync(path, "utf8"), v)}\n`);
  } catch (e) {
    throw new UsageError(`${changelog}: ${e.message}`);
  }
}

function statusCommand() {
  const rows = UNIT_NAMES.map((unit) => {
    const version = versionOf(unit);
    const tag = lastTag(unit);
    const since = tag ? commitsSince(unit, tag).length : null;
    const body = findSection(read(ROOT, UNITS[unit].changelog), "Unreleased")?.body ?? "";
    const entries = body.split("\n").filter((l) => /^[-*] /.test(l)).length;
    return [unit, version, tag ?? "none yet", since === null ? "-" : String(since), String(entries)];
  });
  const head = ["unit", "version", "last tag", "commits since", "unreleased entries"];
  const widths = head.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
  const line = (cells) => cells.map((c, i) => c.padEnd(widths[i])).join("  ").trimEnd();
  console.log(bold(line(head)));
  for (const r of rows) console.log(line(r));
  console.log(dim("\nbun scripts/release.mjs <unit> <patch|minor|major|X.Y.Z> [--draft] [--dry-run] [--commit]"));
}

export function parseArgs(argv) {
  const opts = { dryRun: false, draft: false, commit: false, since: null, date: undefined };
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const value = () => {
      const v = argv[++i];
      if (v === undefined || v.startsWith("--")) throw new UsageError(`${a} needs a value`);
      return v;
    };
    if (a === "--dry-run" || a === "-n") opts.dryRun = true;
    else if (a === "--draft") opts.draft = true;
    else if (a === "--commit") opts.commit = true;
    else if (a === "--since") opts.since = value();
    else if (a === "--date") opts.date = value();
    else if (a === "--help" || a === "-h") opts.help = true;
    else if (a.startsWith("-")) throw new UsageError(`unknown option ${a}`);
    else positional.push(a);
  }
  return { positional, opts };
}

const USAGE = `usage:
  bun scripts/release.mjs                                  each unit's version, last tag and what waits
  bun scripts/release.mjs <unit> <patch|minor|major|X.Y.Z>  bump the unit and date its Unreleased entries
      [--draft] [--since <ref>] [--date YYYY-MM-DD] [--dry-run] [--commit]
  bun scripts/release.mjs <unit> draft [--since <ref>] [--dry-run]
  bun scripts/release.mjs <unit> notes [X.Y.Z]
units: ${UNIT_NAMES.join(", ")}`;

function main(argv) {
  const { positional, opts } = parseArgs(argv);
  const [unit, action, extra] = positional;
  if (opts.help) return console.log(USAGE);
  if (!unit) return statusCommand();
  unitOf(unit);
  if (!action) throw new UsageError(`${USAGE}\n\nWhat for ${unit}? A bump (patch, minor, major or X.Y.Z), draft or notes.`);
  if (action === "notes") return notesCommand(unit, extra);
  if (extra) throw new UsageError(`unexpected ${extra}\n${USAGE}`);
  if (action === "draft") return draftCommand(unit, opts);
  return releaseCommand(unit, action, opts);
}

if (import.meta.main) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(e instanceof UsageError ? e.message : `error: ${e.message}`);
    process.exit(e instanceof UsageError ? 2 : 1);
  }
}
