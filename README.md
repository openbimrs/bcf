# openbim-bcf

Tolerant pure-Rust reader for **BCF-XML** — the BIM Collaboration Format issue
exchange container (buildingSMART S1005).

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
openbim-bcf = "0.2"
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
| Viewpoint (`.bcfv`) camera and component geometry | **not implemented** |
| Project extensions (`.bcfp`, `extensions.xsd`) | **not implemented** |
| **Writing** | **not implemented** |

Read and write support are tracked separately and must never be inferred from
one another. BCF-API (S1006) is a distinct standard and out of scope here.

## Verification

```bash
./scripts/gate.sh              # fmt, build, test, clippy, doc, package
./scripts/mutation-probes.py   # prove the gate can actually fail
```

The gate is authoritative and decides from exit codes. `mutation-probes.py`
injects 17 plausible defects — silent conflict resolution, dropped diagnostics,
tolerated path traversal, normalised status strings — and requires the gate to
catch every one. All 17 are caught.

The official corpus is fetched, not vendored (CC BY-ND); see
`references/README.md`.

## Licence

MIT. The buildingSMART reference corpus this crate is tested against is *not*
MIT and is never committed here.
