# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The repository scaffolding: the crate manifest, the toolchain pin, the dual
  MIT/Apache-2.0 license files and this changelog.
- Serde types for the Deezer API responses and the translation of Deezer
  tracks, albums and artists into catalog records, with the captured response
  fixtures its unit tests run on.
- The catalog interface over the Deezer public API: track, album and artist
  search, entity lookups, artist album and top-track listings, ISRC resolution,
  and a health probe, plus the manifest that declares them.
