# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The plugin builds against the 0.3.0 plugin API
  (`hearthsound:plugin@0.3.0`).

## [0.2.0] - 2026-10-16

### Added

- The artwork interface: album covers and artist portraits, read from the
  subject's own id, from an album's UPC, or from an exact-match Deezer search.
  An artist without a portrait is answered with nothing rather than with
  Deezer's placeholder picture.

### Changed

- The plugin builds against the 0.2.0 plugin API (`hearthsound:plugin@0.2.0`).

## [0.1.1] - 2026-10-13

### Added

- The Deezer artist id on every artist credit, so the server resolves a
  credited artist against this instance instead of showing a name that points
  nowhere.

## [0.1.0] - 2026-10-13

### Added

- The repository scaffolding: the crate manifest, the toolchain pin, the dual
  MIT/Apache-2.0 license files and this changelog.
- Serde types for the Deezer API responses and the translation of Deezer
  tracks, albums and artists into catalog records, with the captured response
  fixtures its unit tests run on.
- The catalog interface over the Deezer public API: track, album and artist
  search, entity lookups, artist album and top-track listings, ISRC resolution,
  and a health probe, plus the manifest that declares them.
- A CI workflow that formats, lints, tests, builds, checks and packs the
  plugin, and uploads the archive.

[Unreleased]: https://github.com/hearthsound/plugin-deezer/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.2.0
[0.1.1]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.1.1
[0.1.0]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.1.0
