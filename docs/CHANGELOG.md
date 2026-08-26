# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-08-26

First release with an implementation. `0.1.0` was a name reservation containing
a `BcfVersion` enum and nothing else.

### Added

- Tolerant BCF-XML reader for versions 2.0, 2.1, and 3.0: archive scanning,
  bounded extraction, markup interpretation, and entry classification.
- `BcfArchive` with `topics()`, `entries()`, `version()`, and `diagnostics()`.
- Markup model: `Topic`, `Comment`, `ViewPointRef`, `HeaderFile`, `Markup`.
  Status, type, priority, stage, and dates are preserved verbatim as strings.
- `Diagnostic` / `Tolerance`: every deviation the reader accepts is reported
  rather than silently absorbed, so callers can enforce their own strictness.
- Version detection via `openbim_core::Detected`, distinguishing a declared
  version from an inferred one and surfacing disagreement as `Conflict` instead
  of picking a side.
- `Limits` bounding total size, per-entry size, and entry count; entries
  escaping the archive root are refused outright.
- Entry points `read_path`, `read_slice`, `read_reader` and their `_with`
  variants taking explicit limits.
- `examples/corpus-report` reproducing every measured claim in the docs.
- `scripts/fetch-official-references.py` fetching and hash-verifying 658
  official BCF-XML and BCF-API files that are not vendored.
- `scripts/mutation-probes.py` injecting 17 plausible defects and requiring the
  gate to catch each one. All 17 are caught.
- `scripts/check-references-untracked.sh` enforcing the CC BY-ND / MIT licence
  boundary as an executable gate step.

### Changed

- `BcfVersion::status_is_attribute` and `wraps_viewpoints` were **removed** and
  replaced by `comments_carry_status` and `nests_collections_in_topic`. The
  originals encoded a false premise; see Fixed.

### Fixed

- Version detection no longer treats `Markup/Viewpoints` as a BCF 3.0 marker.
  In 2.x that element *is* the viewpoint; in 3.0 it wraps `ViewPoint`. The old
  behaviour misread every 2.1 file carrying a viewpoint as 3.0.
- Version detection no longer treats attribute-form `TopicStatus`/`TopicType`
  as 2.1-only evidence. Both are `Topic` attributes in the 2.0 XSD as well, and
  the old behaviour reported three official buildingSMART v2.0 test cases as
  conflicting with their own `bcf.version`.
- Comments and viewpoints nested inside a 3.0 `Topic` (`Topic/Comments/Comment`,
  `Topic/Viewpoints/ViewPoint`) are now read. Previously only the 2.x sibling
  placement was consulted, losing 9 comments across the official corpus.
- `Topic/Labels/Label` (3.0) is read in addition to repeated `Labels` (2.x).

[Unreleased]: https://github.com/openbimrs/bcf/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/openbimrs/bcf/releases/tag/v0.2.0
