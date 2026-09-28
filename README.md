# openbim-bcf

Tolerant pure-Rust reader and strict, deterministic writer for **BCF-XML** —
the BIM Collaboration Format issue exchange container (buildingSMART S1005).

[![crates.io](https://img.shields.io/crates/v/openbim-bcf.svg)](https://crates.io/crates/openbim-bcf)
[![docs.rs](https://img.shields.io/docsrs/openbim-bcf)](https://docs.rs/openbim-bcf)

Canonical repository for OpenBIM.rs BCF support. `openbimrs/openbim` pins it at
`packages/bcf`.

```rust
let archive = openbim_bcf::read_path("issues.bcfzip")?;

for topic in archive.topics() {
    // Status is whatever the file says — see "Tolerance" below.
    println!("{} [{}]", topic.title(), topic.status().unwrap_or("<unset>"));
}

// Everything the reader had to tolerate, rather than silently absorb.
for d in archive.diagnostics() {
    eprintln!("{d}");
}
```

```toml
[dependencies]
openbim-bcf = "0.5"
```

## Tolerance, and why it is not laxness

BCF files in the field routinely violate the specification. Measured over
44 real third-party archives and buildingSMART's own 152-file test corpus:

| The spec says | The corpus says |
| --- | --- |
| `bcf.version` declares the version | **21 of 44** field archives have none |
| `project.bcfp` describes the project | **0 of 44** field archives have one |
| `TopicStatus` comes from an agreed set | `Open`, `OPEN`, `Offen`, `Active`, `ReOpened`, `In Progress` |
| `TopicType` comes from an agreed set | `Error`, `ERROR`, `formale Prüfung`, `Sichprüfung`, `Clash` |

A spec-strict reader rejects nearly all of them — files every other BIM tool
opens without complaint. The status vocabulary is not even fixed by the format:
BCF 2.x defines it in a per-project `extensions.xsd`, so the valid set is a
property of the *project*.

So this crate:

- rejects only what cannot be interpreted at all;
- keeps status, type, priority, stage, and dates **verbatim** — no enums, no
  normalisation, no round-trip corruption;
- reports every deviation it tolerated as a `Diagnostic`, so a caller that
  wants strictness can enforce its own policy.

Reproduce the table:

```bash
cargo run --example corpus-report -- references/test-cases
```

## Writing: strict where the reader is tolerant

A rule checker, clash detector, or review tool can emit `.bcfzip` files from
typed values instead of hand-rolling XML and ZIP packaging:

```rust
use openbim_bcf::write::{self, Document, TargetVersion, Topic};

let doc = Document {
    version: TargetVersion::V3_0,
    extensions: None, // 3.0: derived from the values the topics use
    topics: vec![Topic {
        guid: "3f2504e0-4f89-41d3-9a0c-0305e82c3301".into(),
        title: "Duct clashes with beam".into(),
        topic_type: Some("Clash".into()),
        topic_status: Some("Open".into()),
        creation_date: "2026-09-26T10:00:00Z".into(),
        creation_author: "checker@example.com".into(),
        ..Topic::default()
    }],
};
write::to_path(&doc, "issues.bcfzip")?;
```

The writer targets BCF 2.1 and 3.0 and refuses, before writing a byte,
anything the official schemas reject: malformed GUIDs, an `IfcGuid` that is not
22 characters of IFC base64, dates outside `xs:dateTime`, a 3.0 topic without
type or status, a 3.0 viewpoint without a camera, a type, status, priority or
label missing from supplied `Extensions`. Text with leading or trailing
whitespace is refused too — the reader trims, so it could not round-trip.

Output is **deterministic**. The caller supplies every GUID and timestamp;
entry order, element order, and ZIP metadata are fixed. By default entries are
*stored*, so the bytes do not even depend on a compression library's version:
identical input yields byte-identical archives, so reports can be diffed and
cached. When size matters more, opt into deflate at an explicit level —
still reproducible for a given dependency tree:

```rust
use openbim_bcf::write::{Compression, Options};

let options = Options::default().compression(Compression::Deflated { level: 9 });
write::to_path_with(&doc, "issues.bcfzip", options)?;
```

Written per topic: GUID, title, description, type, status, priority, labels,
creation author and date, comments, and viewpoints with a component selection,
visibility with exceptions, colouring, clipping planes, and an optional camera.
Header files, snapshots, lines, bitmaps, and document references are not
written.

## Version detection reports its evidence

`BCF-XML` 2.0, 2.1, and 3.0 relocate information rather than merely renaming it,
so guessing wrong yields a *different document*, not an error. Detection returns
`openbim_core::Detected`:

| Result | Meaning |
| --- | --- |
| `Declared(v)` | `bcf.version` said so and the markup agrees |
| `Inferred(v)` | no `bcf.version`; derived from document shape |
| `Conflict { declared, observed }` | the two disagree — **never resolved silently** |

`Conflict::resolved()` returns `None` on purpose. Which side is right is a
caller policy decision, and defaulting it here would reintroduce the exact
silent-wrong-parse failure the type exists to surface.

Detection uses only markers verified against the official `markup.xsd` of each
release. Notably, `TopicStatus`/`TopicType` are `Topic` **attributes in 2.0 and
2.1 alike** — a widely repeated claim that they moved in 2.1 is wrong, and
acting on it reports three of buildingSMART's own v2.0 test cases as conflicts.
All 71 official archives detect as `Declared`, with zero conflicts; this is
pinned as a test.

## Untrusted input

BCF archives arrive from third parties, so the reader is bounded rather than
trusting:

- entries that escape the archive root are a hard error, never tolerated;
- decompression is capped by `Limits` (total, per-entry, entry count) and read
  through a hard cap rather than trusting the attacker-controlled central
  directory;
- `#![forbid(unsafe_code)]`, and `zip` is built without optional C codecs.

## Status

| Capability | State |
| --- | --- |
| Archive scanning, entry normalisation, bounded extraction | implemented |
| Version detection with evidence and conflict reporting | implemented |
| Markup: topics, comments, viewpoint refs, header files, labels | implemented |
| Tolerance diagnostics | implemented |
| Reading viewpoint (`.bcfv`) component selection | implemented; all 65 official viewpoints read |
| Reading viewpoint camera, visibility, colouring, clipping | **not implemented** |
| Reading project extensions (`.bcfp`, `extensions.xml`/`.xsd`) | **not implemented** |
| Writing 2.1 and 3.0: topics, comments, component selections, cameras, 3.0 `extensions.xml` | implemented; XSD-validated, round-tripped, golden-pinned |
| Writing viewpoint visibility exceptions, colouring, clipping planes | implemented; XSD-validated, golden-pinned; not read back |
| Writing header files, snapshots, lines, bitmaps, document references, `project.bcfp` | **not implemented** |
| Writing BCF 2.0 | **not implemented** (read-only) |

Read and write support are tracked separately and must never be inferred from
one another. BCF-API (S1006) is a distinct standard and out of scope here.

## Verification

```bash
./scripts/gate.sh              # fmt, build, test, clippy, doc, XSD, package
./scripts/mutation-probes.py   # prove the gate can actually fail
./scripts/validate-written.py  # writer output against the official XSDs
```

The gate is authoritative and decides from exit codes. `mutation-probes.py`
injects 34 plausible defects — silent conflict resolution, dropped diagnostics,
tolerated path traversal, normalised status strings, a writer that accepts
malformed GUIDs or deflates its output — and requires the gate to catch every
one. All 34 are caught.

`validate-written.py` needs the fetched schemas and `lxml`; the gate runs it
whenever the schemas are present. It first proves it can fail by feeding the
validator nine known schema violations.

The official corpus is fetched, not vendored (CC BY-ND); see
`references/README.md`.

## Licence

Repository-authored code is AGPL-3.0-or-later. The buildingSMART reference
corpus is CC BY-ND 4.0 and is never committed here.
