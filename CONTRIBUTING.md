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
   this repository is `AGPL-3.0-or-later OR LicenseRef-Commercial`; the
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
mutation probes on the tagged commit; publishes from the `crates.io`
environment; and creates the GitHub release from the changelog section.
Rehearse with *Actions → Release → Run workflow* and a tag name: everything
but publishing runs.

## Licensing contributions

OpenBIM.rs is dual-licensed: `AGPL-3.0-or-later OR LicenseRef-Commercial`
(see [`LICENSING.md`](LICENSING.md)). To keep the commercial option
possible, every contribution is accepted on the following terms, which you
agree to by submitting it (a pull request, patch, issue attachment or any
other material intended for inclusion):

1. **Licence to the project.** Your contribution is licensed under
   `AGPL-3.0-or-later`.
2. **Licence to the copyright holder.** You also grant the copyright holder
   named in [`NOTICE`](NOTICE) a perpetual, worldwide, non-exclusive,
   royalty-free, irrevocable licence to use, reproduce, modify, distribute,
   sublicense and relicense your contribution, including under proprietary
   or commercial terms, as part of OpenBIM.rs or works derived from it.
3. **You keep your copyright.** These terms grant a licence; they do not
   transfer ownership.
4. **You have the right to grant this.** The contribution is your own work,
   or you have permission to submit it on these terms (for example from
   your employer). Identify third-party material, keep its licence,
   attribution and provenance, and do not submit third-party material whose
   licence does not allow these terms.
5. **Patents.** You grant the same parties a licence under any patent claims
   you can license that your contribution necessarily infringes, on the terms
   of section 11 of the AGPL.

If you cannot agree to these terms, say so in the issue or pull request
before submitting; the contribution can then not be merged.
