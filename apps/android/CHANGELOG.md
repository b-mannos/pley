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
- Automatic quality no longer leaves the original for a wait the server
  caused (a slow disk, a remux catching up): a wait of 2 s or more counts
  only on a network shown below 1.5 times the file's bitrate in the last
  2 minutes (where nothing showed it, the second such wait within 5
  minutes), and the server converting slower than the video plays doesn't
  count at the original. A title that starts lower for what was
  remembered of the network goes back up after 20 s calm, as high as the
  network carries, the original included.

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/android-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/android-v0.0.1
