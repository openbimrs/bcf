# Repository instructions

This is the canonical standalone repository for OpenBIM.rs BCF support.
`openbimrs/openbim` pins it at `packages/bcf`.

## Map

- `openbim-bcf/` — the published crate; read its `AGENTS.md` before editing.
- `docs/` — architecture, corpus evidence, ADRs, changelog.
- `references/` — provenance plus the locally fetched official corpus.
- `scripts/gate.sh` — authoritative local and CI gate.
- `scripts/mutation-probes.py` — proves the gate can fail.

## Commands

```bash
./scripts/gate.sh
./scripts/mutation-probes.py
cargo run --example corpus-report -- references/test-cases
```

Trust command exit codes. Never summarise a Cargo pipeline through
`grep`/`awk`: the pipe hides the Cargo process status.

## Invariants

1. The crate builds from crates.io dependencies with no parent workspace.
2. Status, type, priority, stage, and dates are preserved **verbatim**. Never
   map them onto an enum — BCF 2.x defines the vocabulary per project in
   `extensions.xsd`, so the valid set is not a property of the format.
3. Every tolerated deviation produces a `Diagnostic`. Silently absorbing one is
   a defect even when the parse succeeds.
4. Version detection uses only markers verified against the official
   `markup.xsd` of the relevant release, and reports disagreement as
   `Detected::Conflict` rather than picking a side.
5. Untrusted-input failures (path traversal, limit breach) are hard errors and
   are never downgraded to diagnostics.
6. Official reference bytes stay untracked; they are CC BY-ND and this repo is
   MIT. `scripts/check-references-untracked.sh` enforces it.
7. Read and write support are tracked separately. Never describe writing,
   viewpoint geometry, or project extensions as implemented without executable
   evidence.

## Documentation discipline

Keep capability tables honest: distinguish reserved API, implemented algorithm,
and corpus-verified behaviour. Every measured claim in README or rustdoc must
be reproducible with `cargo run --example corpus-report`. Update README,
rustdoc, and `docs/CHANGELOG.md` together for user-visible changes.
