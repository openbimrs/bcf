# openbim-bcf crate instructions

Follow `../AGENTS.md`. This file covers only what is specific to the crate.

## Module map

| Module | Owns |
| --- | --- |
| `xml` | the minimal pull-based XML tree; no BCF semantics |
| `version` | `BcfVersion` and reconciliation into `Detected` |
| `markup` | the topic/comment/viewpoint model and shape-based detection |
| `archive` | ZIP access, `Limits`, entry classification, `BcfArchive` |
| `diagnostic` | `Tolerance` variants and their `Display` |
| `error` | `BcfError` — failures that yield *nothing* |
| `read` | the public entry points |

## The error/diagnostic boundary

This is the crate's central design decision and the easiest thing to get wrong.

- **`BcfError`** — the caller gets nothing back. Reserved for: not a ZIP, no
  markup at all, a limit breach, an entry escaping the archive root, I/O.
- **`Diagnostic`** — the document is still usable. Everything else: missing
  `bcf.version`, unknown version id, absent GUIDs or titles, dangling
  references, backslash separators, version conflicts.

Given that 21 of 44 real archives lack `bcf.version` and none carry
`project.bcfp`, putting spec deviations in `BcfError` would make the crate
useless. Adding a new tolerance means adding a `Tolerance` variant — never a
silent `unwrap_or_default()`.

## Why hand-rolled XML rather than serde

The same element name means different things across versions (`Comments` is a
3.0 wrapper inside `Topic`; `Comment` is a 2.x sibling of `Topic`), and real
files omit fields the schema requires. A derive mapping needs one struct set
per version and still cannot distinguish "absent" from "present but empty" —
the distinction the whole tolerance policy rests on.

## Version detection pitfalls

Verified against the official `markup.xsd` files; each of these was a real bug:

- `TopicStatus`/`TopicType` are `Topic` **attributes in 2.0 as well as 2.1**.
  The common claim that they moved is false and produces false conflicts on
  buildingSMART's own v2.0 test cases.
- `Markup/Viewpoints` in 2.x **is** the viewpoint. In 3.0 it is a wrapper
  around `ViewPoint`. The element name alone proves nothing.
- `DocumentReferences` exists in 2.0 too, holding `ReferencedDocument`
  directly. Only the 3.0 `DocumentReference` child is evidence.
- 2.0 vs 2.1 is decided **only** by `Comment` members (`Topic` back-reference,
  `Status`, `VerbalStatus`, `ReplyToComment`). A 2.x file with no comments is
  genuinely indeterminate — return `None` rather than guessing.

## Testing

- `tests/archive.rs` — synthesised containers, built in-memory. Fast, hermetic,
  and license-clean.
- `tests/corpus.rs` — opt-in sweeps over the official and field corpora, gated
  on `BCF_OFFICIAL_CORPUS` / `BCF_FIELD_CORPUS`. They skip cleanly when unset;
  do not delete them for CI's convenience.
- `official_archives_never_contradict_their_declared_version` is the sharpest
  oracle available: every official archive is valid by construction, so a
  conflict there is always this crate's defect. It caught two real bugs.
