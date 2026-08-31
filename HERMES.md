# OpenBIM.rs BCF

Canonical repository: <https://github.com/openbimrs/bcf>
Integration repository: <https://github.com/openbimrs/openbim>

Read `AGENTS.md` before changing the repository and `openbim-bcf/AGENTS.md`
before editing the crate. The crate stays independently buildable; the parent
OpenBIM.rs workspace pins this repository as a submodule but is not required
for standalone development.

## Verification

Run `./scripts/gate.sh`. It is the authoritative local and CI gate and decides
success from command exit codes.

Run `./scripts/mutation-probes.py` after changing reader semantics: a passing
suite means nothing unless the suite can fail.

## Project conventions

- Rust 2021, MSRV 1.85, AGPL-3.0-or-later, `#![forbid(unsafe_code)]`, pure Rust.
- BCF-specific models, version semantics, tolerance policy, diagnostics, and
  archive safety stay here. Shared openBIM domain vocabulary lives in
  `openbim-core`.
- Preserve file content verbatim. Never normalise a project-defined vocabulary.
- Do not claim schema validation, write support, or viewpoint geometry without
  executable evidence over the official corpus.
- Do not commit buildingSMART schemas or test cases: they are CC BY-ND and this repository is
  AGPL-3.0-or-later. Use `scripts/fetch-official-references.py` locally.
- Use Keep a Changelog and distinguish implemented capabilities from roadmap.
