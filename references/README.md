# Official BCF references

Unmodified buildingSMART BCF specifications, downloaded on demand.

## 🚨 Why these are not committed

The BCF specifications are licensed **CC BY-ND 4.0, (c) buildingSMART
International Ltd.** This repository is AGPL-3.0-or-later. Vendoring a differently-licensed
corpus into the tree would misrepresent the licence of the whole repository, so
the payload directories are gitignored and fetched locally instead.

`scripts/check-references-untracked.sh` enforces this as part of the gate — it
fails if any payload file is tracked. Only `SOURCE-MANIFEST.json` is committed.

Restore and verify locally:

```bash
./scripts/fetch-official-references.py            # fetch + verify
./scripts/fetch-official-references.py --verify   # verify only
```

## What is fetched

| Directory | Upstream | Contents |
| --- | --- | --- |
| `schemas/bcf-xml-2.0/` | `BCF-XML` tag `v2.0` | `Schemas/` XSDs + README |
| `schemas/bcf-xml-2.1/` | `BCF-XML` tag `v2.1` | `Schemas/` XSDs + README |
| `schemas/bcf-xml-3.0/` | `BCF-XML` tag `v3.0` | `Schemas/` XSDs + README |
| `schemas/bcf-api-2.1/` | `BCF-API` tag `v2.1` | `Schemas_draft-03/` JSON Schemas + README |
| `schemas/bcf-api-3.0/` | `BCF-API` tag `v3.0` | `Schemas_draft-03/` JSON Schemas + README |
| `test-cases/` | `BCF-XML` tag `v3.0` | the consolidated official test corpus |

BCF-API's `README.md` **is** the normative specification text, so it is
retained rather than treated as boilerplate.

Measured after a clean fetch on 2026-08-26:

- 658 files total;
- 15 XSDs across the three BCF-XML releases;
- 103 BCF-API JSON Schema documents;
- 152 official test files under `test-cases/`, of which 71 are readable ZIP
  archives and 81 are loose `markup.bcf` documents distributed unpacked.

Do not edit these files to make a parser accept them. Preserving the official
bytes is the entire purpose of the corpus.

## Running the corpus tests

The corpus tests are opt-in and skip cleanly when the variables are unset:

```bash
BCF_OFFICIAL_CORPUS="$PWD/references/test-cases" \
  cargo test -p openbim-bcf --test corpus -- --nocapture
```

`BCF_FIELD_CORPUS` points at a directory of real third-party `.bcf`/`.bcfzip`
files. That corpus is project data belonging to third parties and is never
part of this repository; see `docs/corpus-evidence.md` for what was measured.

## Two standards, one name

- **BCF-XML** (buildingSMART S1005) — the file container. What `openbim-bcf`
  reads.
- **BCF-API** (S1006) — a REST/JSON service specification sharing the data
  model. Out of scope for this crate; the schemas are fetched for reference so
  the domain model can be kept compatible.

## Provenance

`SOURCE-MANIFEST.json` records the source URL, byte size, and SHA-256 of each
release archive, plus a SHA-256 for every extracted file. `--verify` compares
the tree against it and reports missing, changed, or unexpected files.
