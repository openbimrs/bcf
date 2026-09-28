# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-09-28

### Added

- Viewpoint visibility, colouring, and clipping planes in the writer (#7):
  `write::Viewpoint` gained `visibility: Option<Visibility>` (default
  visibility plus exceptions), `coloring: Vec<Coloring>` (6- or 8-digit hex
  colour and its components), and `clipping_planes: Vec<ClippingPlane>`.
  Each version gets its own schema shape: 3.0 wraps coloured components in
  `Color/Components`, 2.1 lists them under `Color`.
- Refusals `Invalid::Color` (not 6 or 8 hex digits; uppercase only in 2.1, as
  its schema says) and `Invalid::NoComponents` (a colouring without
  components). Components in exceptions and colourings need an identifier,
  as in a selection; a clipping plane needs a non-zero direction.
- Two XSD-validated golden samples (`sample-2.1-styled`, `sample-3.0-styled`),
  four new negative controls in `scripts/validate-written.py`, and five
  mutation probes (34 in total, all caught).

### Changed

- **Breaking:** `write::Viewpoint` gained three public fields, so struct
  literals must set them or end in `..Viewpoint::default()`. With
  `visibility: None` and no colouring, output is byte-identical to 0.3.0.

## [0.3.0] - 2026-09-26

The first release with an implementation on crates.io: `0.2.0` was prepared
but never published, so from crates.io this follows the `0.1.0` name
reservation directly and includes everything listed under `0.2.0` below.

### Added

- `openbim_bcf::write`: a strict, deterministic BCF-XML writer for 2.1 and 3.0
  (#1). `Document` → `to_vec`, `to_writer`, or `to_path`. Writes topics
  (GUID, title, description, type, status, priority, labels, creation author
  and date), comments, and viewpoints with a component selection and an
  optional camera; for 3.0 also `extensions.xml`, supplied or derived from the
  values in use.
- The writer refuses, before writing anything, every value the official
  schemas reject, reporting it as `WriteError::Invalid { at, problem }` with a
  path such as `topics[0].comments[1].date`.
- Deterministic output: fixed entry and element order, a 1980-01-01 timestamp
  and `0644` permissions on every entry. Entries are stored by default, which
  keeps bytes identical across dependency upgrades; pinned by golden files in
  `openbim-bcf/tests/golden/`.
- `write::Options` / `write::Compression` with `to_vec_with`,
  `to_writer_with`, and `to_path_with`: opt into deflate at an explicit level
  `1..=9`, reproducible for a given dependency tree.
- The reader resolves each viewpoint reference and reads the `.bcfv`
  document's GUID and component selection into `ViewPointRef::visualization`
  (`Visualization`, `Component`). All 65 viewpoints in the official corpus
  read.
- `Tolerance::UnreadableViewpoint` for a referenced `.bcfv` that exists but
  cannot be read.
- `scripts/validate-written.py`, run by the gate when the schemas are fetched,
  validates the writer's output against the official XSDs.
- `examples/write-samples`, and 12 mutation probes covering the writer and
  viewpoint reading (29 in total, all caught).

### Changed

- **Breaking:** `ViewPointRef` gained a public `visualization` field, so code
  constructing it with a struct literal must set it.

- Relicensed repository-authored work from MIT to `AGPL-3.0-or-later`; historical releases remain under their published MIT terms, and third-party material retains its own terms.

## [0.2.0] - 2026-08-26 [NOT PUBLISHED]

Prepared and dated here, but never tagged or published to crates.io; its
contents first shipped in `0.3.0`. First version with an implementation.
`0.1.0` was a name reservation containing a `BcfVersion` enum and nothing
else.

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

[Unreleased]: https://github.com/openbimrs/bcf/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/openbimrs/bcf/releases/tag/v0.4.0
[0.3.0]: https://github.com/openbimrs/bcf/releases/tag/v0.3.0
[0.2.0]: https://github.com/openbimrs/bcf/tree/22a1b10
