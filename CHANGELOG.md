# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-10-25

### Added

- The artist's album count as Deezer states it is reported on the catalog
  artist, so a search result can show how many albums the artist has.

### Changed

- Build against the `hearthsound:plugin@0.4.0` API (plugin-sdk v0.4.0).
- The declared request rate is 4 per second with a burst of 4: at most 8
  requests in any second and 20 per 5 seconds, half of Deezer's published
  ceiling of 50 per 5 seconds. The server's limiter then queues a burst of
  cover fetches instead of reaching Deezer and collecting a 403 with its 5
  second penalty.

### Fixed

- An artist Deezer has no portrait for is described without images instead of
  with Deezer's placeholder square, so that square is no longer served as cover
  art when no artwork source has a real image.

## [0.3.1] - 2026-10-24

### Fixed

- Searches stay under Deezer's quota: the declared request rate is below 50
  requests per 5 seconds, a quota error carried by an HTTP status is reported as
  rate-limited instead of unavailable (so the server backs off for one quota
  window), and a forbidden status without a Deezer error body is treated as a
  refusal to retry after that window.

## [0.3.0] - 2026-10-23

### Changed

- The plugin builds against the 0.3.0 plugin API
  (`hearthsound:plugin@0.3.0`).

### Fixed

- An id Deezer cannot have issued is answered with not-found instead of a
  provider error, without a request; Deezer reports such a lookup as an
  "invalid query" error rather than as "no data".
- A search or listing asked for a limit of zero returns empty lists instead of
  everything, which is what Deezer reads `limit=0` as.

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

[Unreleased]: https://github.com/hearthsound/plugin-deezer/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.4.0
[0.3.1]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.3.1
[0.3.0]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.3.0
[0.2.0]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.2.0
[0.1.1]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.1.1
[0.1.0]: https://github.com/hearthsound/plugin-deezer/releases/tag/v0.1.0
