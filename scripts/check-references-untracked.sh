#!/usr/bin/env bash
# Prove the CC BY-ND reference corpus never enters this AGPL-3.0-or-later repository.
#
# This is an executable licence boundary, not a style check: committing
# buildingSMART payload would misrepresent the licence of the whole tree, and
# a .gitignore alone does not stop `git add -f` or a stray `git add -A` made
# before the ignore rule existed.
set -euo pipefail

cd "$(dirname "$0")/.."

tracked="$(git ls-files -- references/schemas references/test-cases || true)"
if [[ -n "$tracked" ]]; then
  echo "ERROR: official BCF reference payload is tracked by Git." >&2
  echo "These files are CC BY-ND 4.0 (c) buildingSMART and must stay untracked:" >&2
  echo "$tracked" | head -20 >&2
  echo "Remove with: git rm -r --cached references/schemas references/test-cases" >&2
  exit 1
fi

# The manifest itself is ours and must be committed, so the corpus a
# contributor fetches can be verified against a pinned, reviewed hash set.
if ! git ls-files --error-unmatch references/SOURCE-MANIFEST.json >/dev/null 2>&1; then
  echo "ERROR: references/SOURCE-MANIFEST.json must be tracked." >&2
  exit 1
fi

echo "reference payload is untracked; manifest is tracked."
