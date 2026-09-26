# Contributing

Contributions are welcome, especially those that widen corpus-verified reader
coverage or move a "not implemented" row in the README to "implemented".

## Before opening a pull request

1. Read `AGENTS.md` and `openbim-bcf/AGENTS.md`.
2. Respect the error/diagnostic boundary: a spec deviation that still leaves a
   usable document is a `Diagnostic`, never a `BcfError`. Adding tolerance
   means adding a `Tolerance` variant, not a silent fallback.
3. Never normalise a project-defined vocabulary. `TopicStatus`, `TopicType`,
   `Priority`, and `Stage` are `String` on purpose; BCF 2.x defines their valid
   values per project in `extensions.xsd`.
4. Add tests before claiming behaviour. Prefer synthesised fixtures in
   `tests/archive.rs` — they are hermetic and licence-clean.
5. Do not commit buildingSMART schemas or test cases. They are CC BY-ND and
   this repository is AGPL-3.0-or-later; the
   `scripts/check-references-untracked.sh` gate rejects them. Fetch them with `scripts/fetch-official-references.py`.
6. Run:

```bash
./scripts/gate.sh
./scripts/mutation-probes.py
```

7. Update the README capability table, rustdoc, and `docs/CHANGELOG.md`
   together when behaviour is user-visible.

## Evidence standards

A measured claim in the README or rustdoc must be reproducible with:

```bash
cargo run --example corpus-report -- <dir>...
```

If you add a claim, add its number to `docs/corpus-evidence.md` and say which
corpus produced it. "Works with real files" is not a claim; "44 of 44 field
archives read, 21 without `bcf.version`" is.

If you change reader semantics, add a mutation probe covering the behaviour you
are relying on. A test that cannot fail is not evidence.

## Version detection

Detection heuristics must be justified against the official `markup.xsd` of the
relevant release, not against folklore. Several widely repeated claims about
BCF version differences are wrong; see the pitfalls list in
`openbim-bcf/AGENTS.md`. When evidence is genuinely absent, return `None` and
let `Detected::Inferred` or a declared version stand — do not guess.

## Commits

Use focused commits with imperative subjects. Cross-repository changes publish
this crate first and update the `openbimrs/openbim` submodule pin last.

## Releasing

Releases are published by `.github/workflows/release.yml` through crates.io
trusted publishing; nobody publishes from a laptop and there is no token
secret.

1. Bump `version` in `openbim-bcf/Cargo.toml` and run `cargo update -p
   openbim-bcf`.
2. Move the `Unreleased` notes in `docs/CHANGELOG.md` under a dated
   `## [<version>]` section.
3. Commit on `main`, then push an annotated tag `v<version>` on that commit.

The workflow refuses a tag that is not on `main`, does not match the
`Cargo.toml` version, or has no changelog section; runs the full gate and the
mutation probes on the tagged commit; publishes from the `release`
environment; and creates the GitHub release from the changelog section.
Rehearse with *Actions → Release → Run workflow* and a tag name: everything
but publishing runs.

## Licensing contributions

Unless an explicitly signed agreement says otherwise, every contribution
submitted to this repository is licensed under `AGPL-3.0-or-later`. Submit only
work that you have the right to license. Identify third-party material and
preserve its license, attribution, and provenance.
