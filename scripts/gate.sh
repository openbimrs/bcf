#!/usr/bin/env bash
# Complete standalone verification gate for openbimrs/bcf.
#
# Decides success from command exit codes only. Never pipe a cargo invocation
# through grep/awk here: the pipe hides the cargo process status.
set -euo pipefail

cd "$(dirname "$0")/.."

# bbv-dev hosts concurrent Rust agents. Respect an explicit target; otherwise
# allocate a unique cache so simultaneous gates cannot share mutable artifacts.
if [[ -z "${CARGO_TARGET_DIR:-}" && -d /mnt/backup/build-cache ]]; then
  CARGO_TARGET_DIR="$(mktemp -d /mnt/backup/build-cache/openbim-bcf-target.XXXXXX)"
  export CARGO_TARGET_DIR
  trap 'rm -rf "$CARGO_TARGET_DIR"' EXIT
fi

# The corpus tests skip cleanly when these are unset. Point them at a local
# corpus if one is present, so the gate is as strong as the machine allows.
if [[ -z "${BCF_OFFICIAL_CORPUS:-}" && -d references/test-cases ]]; then
  export BCF_OFFICIAL_CORPUS="$PWD/references/test-cases"
fi

cargo fmt --all -- --check
cargo build --workspace --all-targets
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# The reference corpus is fetched, not vendored. Verify it when present, and
# prove it stayed out of Git either way.
if [[ -f references/SOURCE-MANIFEST.json && -d references/schemas ]]; then
  ./scripts/fetch-official-references.py --verify
fi
./scripts/check-references-untracked.sh

cargo package --locked -p openbim-bcf
