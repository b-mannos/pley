# Changelog

Notable changes to Pley Media Server: the server, its transcoder and the web app it
embeds, with the Docker image and the Windows installer, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases are tagged `server-v<version>`; [docs/RELEASING.md](../../docs/RELEASING.md)
describes how they are made.

## [Unreleased]

### Security

- A session signed out, or whose account was disabled or changed, could
  keep working for up to a minute when it was being checked at that
  moment.
- An administrator's own PIN changes only in Account, with the password,
  no longer through the users list with the session alone.
- The transcoder's folder for segments in memory (`/dev/shm`) is never one
  another account on the computer made first.
- A file's attached fonts are taken 1,000 at most.
- An account keeps at most 200 networks remembered for Automatic quality.
- Subtitles from OpenSubtitles download over https only.
- ctutils 0.4.7 (RUSTSEC-2026-0385).

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/server-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/server-v0.0.1
