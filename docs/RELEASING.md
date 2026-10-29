# Versions, changelogs and releases

Each app is released on its own, with its own version, changelog and tag.

## Units

| Unit | What | Version in | Release files |
|---|---|---|---|
| `server` | the server, its transcoder and the web app it embeds | `apps/server/Cargo.toml`, `apps/transcoder/Cargo.toml`, `packages/transcoder-api/Cargo.toml`, `apps/web/package.json` | `pley-server-<version>-<os>-<arch>.zip`/`.tar.gz`, the Windows installers `…-setup.exe`, the Docker image `ghcr.io/hhaidar/pley:<version>`, `:<major>.<minor>` and `:latest` |
| `desktop` | the desktop app (Tauri) | `apps/desktop/package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` | `pley-desktop-<version>-<os>-<arch>` `.msi`, `-setup.exe`, `.dmg`, `.deb`, `.rpm`, `.AppImage` |
| `android` | phones, tablets, Android TV, Fire TV | `apps/android/app/build.gradle.kts` (`appVersion`) | `pley-android-<version>.apk`, `pley-android-<version>-play.aab` |
| `samsung` | Samsung TVs (`apps/tizen`) | `apps/tizen/package.json`, `config.xml` | none: the package is signed with a Samsung certificate, by hand (`bun run --cwd apps/tizen package`) |
| `lg` | LG TVs (`apps/webos`) | `apps/webos/package.json`, `appinfo.json` | `pley-lg-<version>.ipk` |
| `kodi` | the Kodi add-on | `apps/kodi/plugin.video.pley/addon.xml` | `plugin.video.pley-<version>.zip` |
| `roku` | the Roku app | `apps/roku/manifest` (major, minor, build), `apps/roku/package.json` | `pley-roku-<version>.zip` |

The lock files (`Cargo.lock`, `bun.lock`, `apps/desktop/src-tauri/Cargo.lock`)
record the versions too. The transcoder and the web app carry the server's
version: they ship only inside the server's archives, installers and image,
the web app inside the server's binary. The server and the transcoder agree
through their protocol's own `api_version`, not their versions.
`packages/transcoder-api` follows the server for the same reason, and
`packages/tv` has no version: each TV app builds it in and versions itself.

Each unit's `CHANGELOG.md` is in its folder (the server's in `apps/server`,
Samsung's in `apps/tizen`, LG's in `apps/webos`). Releases are tagged
`<unit>-v<version>`, such as `kodi-v0.0.2`.

Builds show the commit they were made from beside the version, as
"0.0.1 (1a2b3c4d5)": `pley --version`, the web app's Account page and
dashboard, and each app's Settings (`scripts/commit.mjs`, `PLEY_COMMIT`).

### Android's version code

`10,000,000 + major × 1,000,000 + minor × 10,000 + patch × 100`, so 0.0.1 is
10000100 and 1.2.3 is 11020300. Google Play had 20003 (0.2.0, the one version
every app shared before), and every code since is above it. Minor and patch
stay below 100. A rebuild of the same version for the stores, which take each
code once, sets `PLEY_BUILD` (1 to 99), which is added.

## Commands

```sh
bun scripts/version.mjs                  # every unit's version
bun scripts/version.mjs check [unit]     # manifests and CHANGELOG agree (CI runs it)
bun scripts/version.mjs set kodi 0.1.0   # sets one unit's manifests, nothing else

bun scripts/release.mjs                  # each unit: version, last tag, commits since, entries waiting
bun scripts/release.mjs kodi draft       # adds the commits since kodi's last tag to Unreleased
bun scripts/release.mjs kodi patch --dry-run
bun scripts/release.mjs kodi patch --commit
bun scripts/release.mjs kodi notes 0.0.2 # a version's notes, as the release shows them
bun test ./scripts                       # the scripts' tests
```

`release.mjs <unit> <patch|minor|major|X.Y.Z>` sets the unit's version in its
manifests and moves its CHANGELOG's Unreleased entries under the new version,
dated today (`--date` to change it). `--draft` first adds the subjects of the
commits since the unit's last tag that touch its folders, without their
"area:" prefix, under "Changed", to sort and edit; `--since <ref>` drafts from
another commit (before a unit's first tag, `git fetch --tags` and `--since` are
needed). For Kodi it also writes the notes into `addon.xml`'s `<news>`, which
Kodi shows as the add-on's changelog. It commits only with `--commit` (as
"kodi: release 0.0.2"), and refuses an empty Unreleased, a version not above
the current one, a tag that exists, or manifests that disagree.

## Releasing

1. Write what changed under `## [Unreleased]` in the unit's `CHANGELOG.md`
   as it changes (Added, Changed, Fixed, Removed), or draft it with
   `bun scripts/release.mjs <unit> draft` and edit.
2. `bun scripts/release.mjs <unit> patch --dry-run`, then without
   `--dry-run` (or with `--commit`), and push the commit to master.
3. On GitHub: Actions → Release → Run workflow, choose the unit.

The Release workflow refuses to run off master, checks the unit's manifests
and CHANGELOG agree, and stops if `<unit>-v<version>` exists. It runs the
unit's own workflow (its tests and checks) with its release builds, then
tags the commit, and publishes the GitHub release, with the version's
CHANGELOG section as its notes, `SHA256SUMS`, and build provenance
(`gh attestation verify <file> --repo hhaidar/pley`) where GitHub keeps it
(public repositories, or GitHub Enterprise Cloud; else a warning says it is
missing). "Keep the GitHub
release a draft" leaves the release to publish by hand. A server release
also pushes the Docker image, and the repository's "Latest" release is
always the server's. If a run fails after tagging, "Re-run failed jobs"
picks up the tag it made.

Settings the workflow uses, all optional:

- `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`
  and `ANDROID_KEY_PASSWORD`, secrets: the Android release key
  ([ANDROID.md](ANDROID.md#releases)). Without them an Android release has
  its tag and notes, no APK or bundle, and says so.
- The `release` environment: required reviewers there hold the Docker image
  until someone approves.

## CI

Each unit has a workflow named after it, which runs on pushes to master and
pull requests that touch it, and by hand (Run workflow):

| Workflow | Runs for changes in |
|---|---|
| Server | `apps/server`, `apps/transcoder`, `packages/transcoder-api`, `apps/web`, `deploy/` (but its Markdown), `Cargo.*`, `rustfmt.toml`, `deny.toml`, the root `package.json`, `bun.lock`, `bunfig.toml`, the build, installer, licence and source scripts |
| Desktop | `apps/desktop`, the root `package.json`, `bun.lock`, `bunfig.toml`, `rustfmt.toml` |
| Android | `apps/android`, `scripts/check-android.ps1` |
| Samsung TV, LG TV | `apps/tizen` or `apps/webos`, `packages/tv`, the root `package.json`, `bun.lock`, `bunfig.toml`, `scripts/commit.mjs` |
| Kodi | `apps/kodi`, and the server's Rust code and `Cargo.*`: its end-to-end test runs against a server built from the same commit |
| Roku | `apps/roku`, the root `package.json` (Bun's version) |
| Repository | `.github/workflows/`, the version and release scripts, any `CHANGELOG.md` |

Each also runs when its own workflow file changes. A CHANGELOG alone runs
only Repository. Audit (known vulnerabilities in the lock files) runs only by
hand.
