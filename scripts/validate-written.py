#!/usr/bin/env python3
"""Validate archives written by openbim-bcf against the official XSDs.

Runs `cargo run --example write-samples`, then validates every entry of every
written archive against the buildingSMART schema of the version its
`bcf.version` declares:

    bcf.version     -> version.xsd
    extensions.xml  -> extensions.xsd (3.0)
    */markup.bcf    -> markup.xsd
    */*.bcfv        -> visinfo.xsd

An entry of any other kind fails the run: the writer must not emit files this
script does not know how to check.

Before trusting a pass, the script proves it can fail: it applies known
schema violations to written entries and requires each to be rejected.

The schemas are CC BY-ND and fetched, not vendored; see references/README.md.
Requires lxml (Debian/Ubuntu: python3-lxml).

Usage:
    ./scripts/validate-written.py
"""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import zipfile

try:
    from lxml import etree
except ImportError:
    sys.exit("ERROR: lxml is required (apt install python3-lxml / pip install lxml)")

ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / "references" / "schemas"

_cache: dict[Path, etree.XMLSchema] = {}


def schema(version: str, name: str) -> etree.XMLSchema:
    path = SCHEMAS / f"bcf-xml-{version}" / "Schemas" / name
    if path not in _cache:
        if not path.is_file():
            sys.exit(f"ERROR: {path} missing; run ./scripts/fetch-official-references.py")
        _cache[path] = etree.XMLSchema(etree.parse(str(path)))
    return _cache[path]


def schema_for(entry: str, version: str) -> etree.XMLSchema | None:
    if entry == "bcf.version":
        return schema(version, "version.xsd")
    if entry == "extensions.xml" and version == "3.0":
        return schema(version, "extensions.xsd")
    if entry.endswith("/markup.bcf"):
        return schema(version, "markup.xsd")
    if entry.endswith(".bcfv"):
        return schema(version, "visinfo.xsd")
    return None


def errors(xsd: etree.XMLSchema, data: bytes) -> list[str]:
    doc = etree.fromstring(data)
    if xsd.validate(doc):
        return []
    return [f"line {e.line}: {e.message}" for e in xsd.error_log]


def declared_version(z: zipfile.ZipFile) -> str:
    return etree.fromstring(z.read("bcf.version")).get("VersionId")


def validate_archive(path: Path) -> list[str]:
    problems = []
    with zipfile.ZipFile(path) as z:
        version = declared_version(z)
        if version not in ("2.1", "3.0"):
            return [f"{path.name}: unexpected VersionId {version!r}"]
        for entry in z.namelist():
            xsd = schema_for(entry, version)
            if xsd is None:
                problems.append(f"{path.name}:{entry}: no schema for this entry kind")
                continue
            for err in errors(xsd, z.read(entry)):
                problems.append(f"{path.name}:{entry}: {err}")
    return problems


# (label, version, entry predicate, find, replace) — each must be rejected.
NEGATIVE_CONTROLS = [
    (
        "3.0 viewpoint without a camera",
        "3.0",
        lambda e: e.endswith(".bcfv"),
        (b"<PerspectiveCamera>", b"</PerspectiveCamera>"),
        None,
    ),
    (
        "3.0 uppercase topic GUID",
        "3.0",
        lambda e: e.endswith("/markup.bcf"),
        b'Topic Guid="3f2504e0',
        b'Topic Guid="3F2504E0',
    ),
    (
        "2.1 comment elements out of sequence order",
        "2.1",
        lambda e: e.endswith("/markup.bcf"),
        b"<Date>2026-09-26T10:00:00Z</Date>\n    <Author>checker@example.com</Author>",
        b"<Author>checker@example.com</Author>\n    <Date>2026-09-26T10:00:00Z</Date>",
    ),
    (
        "2.1 IfcGuid of 21 characters",
        "2.1",
        lambda e: e.endswith(".bcfv"),
        b'IfcGuid="0fXw$sQh19ixbI4tZgfkXu"',
        b'IfcGuid="0fXw$sQh19ixbI4tZgfkX"',
    ),
    (
        "2.1 malformed CreationDate",
        "2.1",
        lambda e: e.endswith("/markup.bcf"),
        b"<CreationDate>2026-09-26T10:00:00Z</CreationDate>",
        b"<CreationDate>2026-09-26 10:00</CreationDate>",
    ),
]


def mutate(data: bytes, find, replace) -> bytes | None:
    if replace is None:
        start, end = find
        i, j = data.find(start), data.find(end)
        if i < 0 or j < 0:
            return None
        return data[:i] + data[j + len(end):]
    if find not in data:
        return None
    return data.replace(find, replace, 1)


def run_negative_controls(archives: dict[str, Path]) -> list[str]:
    failures = []
    for label, version, pick, find, replace in NEGATIVE_CONTROLS:
        rejected = anchored = False
        with zipfile.ZipFile(archives[version]) as z:
            for entry in filter(pick, z.namelist()):
                mutated = mutate(z.read(entry), find, replace)
                if mutated is None:
                    continue
                anchored = True
                rejected = bool(errors(schema_for(entry, version), mutated))
                break
        if not anchored:
            failures.append(f"negative control anchor not found: {label}")
        elif not rejected:
            failures.append(f"validator accepted a known violation: {label}")
        else:
            print(f"rejected as expected: {label}")
    return failures


def main() -> int:
    with tempfile.TemporaryDirectory() as out:
        subprocess.run(
            ["cargo", "run", "--quiet", "--example", "write-samples", "--", out],
            cwd=ROOT,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        archives = sorted(Path(out).glob("*.bcfzip"))
        if not archives:
            print("FAIL: write-samples produced no archives")
            return 1

        by_version = {}
        problems = []
        for path in archives:
            with zipfile.ZipFile(path) as z:
                by_version.setdefault(declared_version(z), path)
            found = validate_archive(path)
            problems.extend(found)
            if not found:
                with zipfile.ZipFile(path) as z:
                    print(f"valid: {path.name} ({len(z.namelist())} entries)")

        for version in ("2.1", "3.0"):
            if version not in by_version:
                problems.append(f"no sample archive targets BCF {version}")
        if not problems:
            problems.extend(run_negative_controls(by_version))

    if problems:
        print("\nFAIL:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"\nall {len(archives)} written archives are schema-valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
