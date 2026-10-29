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

### Added

- The web app draws picture subtitles (PGS, VobSub, DVB) over the video
  itself, so the file still plays as it is, or its stream keeps HDR,
  rather than having them burned in.
- A picture subtitle track still being copied out of a big file comes as
  far as it is (`X-Pley-Complete: 0`), and its copy starts as the playback
  does; one a Matroska file's Cues list comes whole in seconds.
- PGS tracks as `.sup` files (`/api/files/{id}/subtitles/{index}.sup`),
  for Kodi. Play responses say `pts_offset_ms`.

### Changed

- Picture subtitles a player shows itself are burned in on any stream it
  asks for (Remux, Transcode), not only when the file can't play as it is.
- AMD graphics on Windows transcode faster: the frames reach AMD's encoder
  from the graphics card, where it copied them from system memory itself,
  several times slower.
- HDR tone mapping does less where the graphics card would fall behind the
  player: Pley times it when it starts, and each transcode takes the best
  quality that keeps ahead, counting the other HDR transcodes running.
- Several HDR transcodes at once keep up: one already running moves to the
  quality the graphics card can share as soon as another starts, and where
  AMD or Intel graphics fall behind even so, the processor decodes the
  video while it has cores to spare. Two 4K HDR films tone mapped to 4K at
  once went from 0.9× to 1.2× real time each on a Ryzen 5 5600G.
- The fastest tone mapping quality is faster where the picture shrinks by
  half at most (4K to 4K or 1080p), with the same picture: two 4K films
  made 1080p at once went from 1.0× to 1.65× real time each.

### Fixed

- H.264 from AMD's encoder says its colours (BT.709), which it left out:
  players guessed, and some took 4K video for BT.2020.

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/server-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/server-v0.0.1
