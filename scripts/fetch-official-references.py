#!/usr/bin/env python3
"""Fetch and verify official BCF references for local development.

The buildingSMART BCF specifications are licensed CC BY-ND 4.0. That permits
redistribution of verbatim copies, but this repository is AGPL-3.0-or-later and vendoring a
differently-licensed corpus into it would misrepresent the licence of the tree.
So the payload is downloaded on demand, hash-pinned in SOURCE-MANIFEST.json,
and gitignored.

    ./scripts/fetch-official-references.py            # fetch + verify
    ./scripts/fetch-official-references.py --verify   # verify only
    ./scripts/fetch-official-references.py --update-manifest

Afterwards the corpus tests can run:

    BCF_OFFICIAL_CORPUS="$PWD/references/test-cases" \\
      cargo test -p openbim-bcf --test corpus -- --nocapture
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
from urllib.request import Request, urlopen
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]
REFERENCES = ROOT / "references"
MANIFEST = REFERENCES / "SOURCE-MANIFEST.json"

SCHEMAS_DIR = REFERENCES / "schemas"
TEST_CASES_DIR = REFERENCES / "test-cases"

# buildingSMART publishes each BCF release as a Git tag. The tarball URL is
# stable and does not require an API token.
XML_RELEASES = {
    "bcf-xml-2.0": "v2.0",
    "bcf-xml-2.1": "v2.1",
    "bcf-xml-3.0": "v3.0",
}
API_RELEASES = {
    "bcf-api-2.1": "v2.1",
    "bcf-api-3.0": "v3.0",
}

XML_URL = "https://codeload.github.com/buildingSMART/BCF-XML/zip/refs/tags/{tag}"
API_URL = "https://codeload.github.com/buildingSMART/BCF-API/zip/refs/tags/{tag}"

UA = "openbim-bcf reference fetcher (https://github.com/openbimrs/bcf)"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url: str) -> bytes:
    req = Request(url, headers={"User-Agent": UA})
    with urlopen(req, timeout=180) as resp:
        return resp.read()


def extract_release(payload: bytes, dest_schemas: Path, dest_cases: Path | None) -> None:
    """Pull the schema tree and (optionally) Test Cases out of a release archive.

    BCF-XML puts XSDs in `Schemas/`; BCF-API puts JSON Schemas in
    `Schemas_draft-03/`. Both are copied to the same destination so callers do
    not have to care which upstream repo a release came from. `README.md` comes
    along because for BCF-API it *is* the normative specification text.
    """
    with tempfile.TemporaryDirectory() as tmp:
        tmp_path = Path(tmp)
        archive = tmp_path / "release.zip"
        archive.write_bytes(payload)
        with ZipFile(archive) as zf:
            bad = zf.testzip()
            if bad is not None:
                raise SystemExit(f"corrupt archive entry: {bad}")
            zf.extractall(tmp_path / "x")
        roots = list((tmp_path / "x").iterdir())
        if len(roots) != 1:
            raise SystemExit(f"unexpected archive layout: {roots}")
        src = roots[0]

        if dest_schemas.exists():
            shutil.rmtree(dest_schemas)
        dest_schemas.mkdir(parents=True)

        copied = False
        for name in ("Schemas", "Schemas_draft-03"):
            origin = src / name
            if origin.is_dir():
                shutil.copytree(origin, dest_schemas / name)
                copied = True
        readme = src / "README.md"
        if readme.is_file():
            shutil.copy2(readme, dest_schemas / "README.md")
            copied = True
        if not copied:
            raise SystemExit(f"no schema tree found in {src.name}")

        if dest_cases is not None:
            origin = src / "Test Cases"
            if origin.is_dir():
                if dest_cases.exists():
                    shutil.rmtree(dest_cases)
                shutil.copytree(origin, dest_cases)


def fetch_all() -> None:
    SCHEMAS_DIR.mkdir(parents=True, exist_ok=True)
    TEST_CASES_DIR.mkdir(parents=True, exist_ok=True)

    archives: dict[str, dict[str, object]] = {}

    for name, tag in XML_RELEASES.items():
        url = XML_URL.format(tag=tag)
        print(f"fetching {name} ({url}) ...")
        payload = download(url)
        archives[name] = {"url": url, "bytes": len(payload), "sha256": sha256_bytes(payload)}
        # Only the newest release ships the consolidated Test Cases tree.
        cases = TEST_CASES_DIR if name == "bcf-xml-3.0" else None
        extract_release(payload, SCHEMAS_DIR / name, cases)

    for name, tag in API_RELEASES.items():
        url = API_URL.format(tag=tag)
        print(f"fetching {name} ({url}) ...")
        payload = download(url)
        archives[name] = {"url": url, "bytes": len(payload), "sha256": sha256_bytes(payload)}
        extract_release(payload, SCHEMAS_DIR / name, None)

    write_manifest(archives)
    print(f"\nwrote {MANIFEST.relative_to(ROOT)}")


def local_files() -> dict[str, str]:
    files: dict[str, str] = {}
    for base in (SCHEMAS_DIR, TEST_CASES_DIR):
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if path.is_file():
                files[str(path.relative_to(REFERENCES))] = sha256_file(path)
    return files


def write_manifest(archives: dict[str, dict[str, object]]) -> None:
    existing = {}
    if MANIFEST.is_file():
        existing = json.loads(MANIFEST.read_text())
    manifest = {
        "source": {
            "bcf-xml": "https://github.com/buildingSMART/BCF-XML",
            "bcf-api": "https://github.com/buildingSMART/BCF-API",
            "license": "CC BY-ND 4.0 (c) buildingSMART International Ltd.",
            "note": (
                "Not vendored: this repository is AGPL-3.0-or-later and the reference corpus is not. "
                "Fetch locally; the payload directories are gitignored."
            ),
        },
        "archives": archives or existing.get("archives", {}),
        "files": local_files(),
    }
    MANIFEST.parent.mkdir(parents=True, exist_ok=True)
    MANIFEST.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")


def verify() -> int:
    if not MANIFEST.is_file():
        print(f"no manifest at {MANIFEST}; run without --verify first.", file=sys.stderr)
        return 1
    manifest = json.loads(MANIFEST.read_text())
    expected: dict[str, str] = manifest.get("files", {})
    actual = local_files()

    missing = sorted(set(expected) - set(actual))
    extra = sorted(set(actual) - set(expected))
    changed = sorted(k for k in set(expected) & set(actual) if expected[k] != actual[k])

    for label, items in (("missing", missing), ("unexpected", extra), ("changed", changed)):
        if items:
            print(f"{len(items)} {label}:")
            for item in items[:20]:
                print(f"  {item}")
            if len(items) > 20:
                print(f"  ... and {len(items) - 20} more")

    if missing or changed:
        return 1
    print(f"verified {len(expected)} reference files against the manifest.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true", help="verify local files only")
    parser.add_argument(
        "--update-manifest",
        action="store_true",
        help="rewrite the manifest from what is on disk",
    )
    args = parser.parse_args()

    if args.verify:
        return verify()
    if args.update_manifest:
        write_manifest({})
        print(f"wrote {MANIFEST.relative_to(ROOT)}")
        return 0

    fetch_all()
    return verify()


if __name__ == "__main__":
    sys.exit(main())
