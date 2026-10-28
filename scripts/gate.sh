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
# prove it stayed out of Git either way. With the official XSDs at hand, also
# prove every archive the writer produces is schema-valid.
if [[ -f references/SOURCE-MANIFEST.json && -d references/schemas ]]; then
  ./scripts/fetch-official-references.py --verify
  ./scripts/validate-written.py
fi
./scripts/check-references-untracked.sh

cargo package --locked -p openbim-bcf

# The crate is `AGPL-3.0-or-later OR LicenseRef-Commercial`: its archive ships
# byte-identical copies of the root licence texts and notice.
version="$(cargo metadata --format-version 1 --no-deps \
  | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"]=="openbim-bcf"))')"
package_root="${CARGO_TARGET_DIR:-target}/package/openbim-bcf-${version}"
for f in LICENSE NOTICE LICENSES/LicenseRef-Commercial.txt; do
  cmp -s "$f" "$package_root/$f" || {
    echo "packaged crate: $f is missing or differs from the root" >&2
    exit 1
  }
done
grep -qx 'license = "AGPL-3.0-or-later OR LicenseRef-Commercial"' "$package_root/Cargo.toml" || {
  echo "packaged crate: unexpected license expression" >&2
  exit 1
}
