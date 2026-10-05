#!/usr/bin/env bash
# Bootstrap an Ubuntu/Debian cloud workspace (Codex, Claude, or a fresh VM).
# Run while networking is available. See docs/cloud-setup.md for the contract.
set -euo pipefail

case "${1:-}" in
  --help|-h)
    printf 'Usage: %s\nPrepares tools and dependencies; run scripts/gate.sh separately.\n' "$0"
    exit 0 ;;
  '') ;;
  *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
esac
[[ $# -le 1 ]] || { echo 'Expected no arguments or --help' >&2; exit 2; }

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
trap 'printf "cloud setup failed at line %s: %s\n" "$LINENO" "$BASH_COMMAND" >&2' ERR

as_root() {
  if [[ "$(id -u)" == 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null && sudo -n true 2>/dev/null; then
    sudo -n "$@"
  else
    echo 'Cloud setup requires root or passwordless sudo to install image dependencies.' >&2
    return 1
  fi
}

command -v apt-get >/dev/null || {
  echo 'Cloud setup supports Ubuntu/Debian images with apt-get.' >&2
  exit 1
}
as_root env DEBIAN_FRONTEND=noninteractive apt-get update
as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  ca-certificates curl git build-essential pkg-config python3 python3-dev \
  python3-pip python3-venv xz-utils python3-lxml

# Make installed commands visible to later shells too: exports from a setup
# subprocess do not survive `exec ./scripts/cloud-setup.sh`.
expose() {
  [[ "$1" != "/usr/local/bin/$2" ]] || return 0
  [[ -x "$1" ]] || { echo "Installed tool missing: $1" >&2; return 1; }
  as_root mkdir -p /usr/local/bin
  as_root ln -sfn "$1" "/usr/local/bin/$2"
}
export PATH="/usr/local/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:$PATH"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

toolchain="1.88.0" # Matches the repository's CI/MSRV.
if ! command -v rustup >/dev/null; then
  curl --fail --location --silent --show-error --retry 3 \
    https://sh.rustup.rs --output "$work/rustup-init.sh"
  bash "$work/rustup-init.sh" -y --profile minimal --default-toolchain none --no-modify-path
fi
rustup toolchain install "$toolchain" --profile minimal --component rustfmt --component clippy
# Repositories without rust-toolchain.toml still need the CI pin in future shells.
if [[ ! -f rust-toolchain.toml && ! -f rust-toolchain ]]; then
  rustup override set "$toolchain"
fi
for tool in rustup cargo rustc rustfmt cargo-fmt cargo-clippy clippy-driver; do
  if [[ -x "${CARGO_HOME:-$HOME/.cargo}/bin/$tool" ]]; then
    expose "${CARGO_HOME:-$HOME/.cargo}/bin/$tool" "$tool"
  fi
done

cargo fetch --locked

# These tools belong to the disposable cloud image, not the project lockfile.
# --user avoids overwriting distro packages; Debian's externally-managed
# interpreter requires the explicit override even for user-site installs.
pip_args=(--disable-pip-version-check)
python_bin="$(python3 -c 'import sys; print(sys.prefix + "/bin")')"
if python3 -c 'import sys; sys.exit(sys.prefix != sys.base_prefix)'; then
  pip_args+=(--user --break-system-packages)
  python_bin="$(python3 -m site --user-base)/bin"
fi
python3 -m pip install "${pip_args[@]}" lxml


# Like CI, a corpus outage is non-fatal and reported explicitly.
if ! scripts/fetch-official-references.py; then
  echo "warning: BCF corpus unavailable; corpus-backed tests will skip." >&2
fi

printf '\nCloud setup complete. Run: %s\n' 'scripts/gate.sh'
