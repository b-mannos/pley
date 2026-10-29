# Changelog

Notable changes to Pley for Android: phones, tablets, Android TV and Fire TV, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases are tagged `android-v<version>`; [docs/RELEASING.md](../../docs/RELEASING.md)
describes how they are made.

## [Unreleased]

### Security

- No other app on a TV can make Pley play a title through a Watch Next
  link: links carry this installation's signature.
- A stream's subtitles are never read from a file on the device that the
  server names.

### Fixed

- Picture subtitles in a file next to the media show over the video, from
  the server's copy of them.
- Automatic quality keeps the original (direct play or remux) unless the
  picture stops for want of data for 2 s or more. Short waits and slow
  delivery moved it to a transcode though the network carried the file:
  a big file's buffer holds only seconds of it. From a stream in several
  sizes it now steps back up after calm minutes too, to the original when
  the network carries 1.6 times it.

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/android-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/android-v0.0.1
