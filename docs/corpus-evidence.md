# Corpus evidence

Every measured claim in the README and rustdoc comes from here. Reproduce with:

```bash
cargo run --example corpus-report -- <dir>...
```

Measured 2026-08-26 with `openbim-bcf` 0.2.0.

## Corpora

| Corpus | What | Redistributable |
| --- | --- | --- |
| official | buildingSMART BCF-XML v3.0 `Test Cases` (152 files) | CC BY-ND — fetched, not vendored |
| field | 44 real `.bcf`/`.bcfzip` from 5 construction projects | no — third-party project data |

The field corpus is the Schependomlaan open dataset plus four German
residential projects (`EFH`, `MFH`, `SFW`) whose audit reports were produced by
different tools in different languages. That linguistic and vendor diversity is
the point: it is what makes verbatim preservation testable.

## Official corpus

```
archives read : 71
loose xml     : 81      # markup.bcf shipped unpacked, not a ZIP
failed        : 0
topics        : 81
comments      : 64

-- version detection --
Declared(2.0)                33
Declared(2.1)                19
Declared(3.0)                19

-- tolerances --
(none)
```

**Zero conflicts and zero inferences.** Every official archive detects as
exactly the version it declares. This is the strongest oracle available: the
corpus is valid by construction, so any conflict is a defect in this reader.
Pinned as `official_archives_never_contradict_their_declared_version`.

It earned its place — it caught two real defects:

1. `Markup/Viewpoints` was treated as a BCF 3.0 marker. In 2.x that element
   *is* the viewpoint, so every 2.1 file with a viewpoint was misread as 3.0.
2. Attribute-form `TopicStatus` was treated as 2.1-only evidence. The 2.0 XSD
   declares it as an attribute too, so three v2.0 test cases were reported as
   conflicting with their own `bcf.version`.

Both were invisible to hand-written fixtures, because the fixtures encoded the
same wrong belief as the code.

### TopicStatus values observed

`Open` (48), `<none>` (17), `OPEN` (12), `Active` (2), `In Progress` (1),
`ReOpened` (1).

Six spellings of three concepts, in buildingSMART's *own* corpus. This is why
status is a `String`.

## Field corpus

```
archives read : 44
loose xml     : 0
failed        : 0
topics        : 180
comments      : 121

-- version detection --
Declared(2.0)                 4
Declared(2.1)                19
Inferred(2.0)                21

-- tolerances --
MissingVersionEntry          21
TopicWithoutTitle             1
```

- **21 of 44 (48%) carry no `bcf.version`.** Rejecting them, as the spec
  arguably permits, would reject nearly half of real-world BCF.
- **0 of 44 carry `project.bcfp`.** The project extension is effectively
  unused in practice.
- 106 of 121 comments carry a 2.0 `Topic` back-reference, which is what lets
  the 21 undeclared archives be inferred as 2.0 rather than guessed.

### TopicStatus / TopicType values observed

Status: `<none>` (102), `Offen` (62), `Open` (16).
Type: `<none>` (102), `formale Prüfung` (48), `Error` (21), `Sichprüfung` (8),
`Clash` (1).

`Offen` is German for `Open`. It is not a typo and not a parse failure — BCF
2.x defines the status vocabulary in a per-project `extensions.xsd`, so the
valid set is a property of the *project*, not the format. Normalising `Offen`
to `Open` would corrupt a round-trip and silently rewrite someone's data.

## What is deliberately not claimed

- No XSD validation is performed. Reading a file is not validating it.
- Writing is not implemented, so no round-trip fidelity claim is made.
- Viewpoint (`.bcfv`) geometry is not parsed; only the reference is read.
- The 81 loose `markup.bcf` documents in the official corpus are counted as
  "not an archive", not as failures — they are XML documents shipped unpacked,
  and this crate's entry point is the container.
