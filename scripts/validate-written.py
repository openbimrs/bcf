#!/usr/bin/env python3
"""Validate archives written by openbim-bcf against the official XSDs.

Runs `cargo run --example write-samples`, then validates every entry of every
written archive against the buildingSMART schema of the version its
`bcf.version` declares:

    bcf.version     -> version.xsd
    extensions.xml  -> extensions.xsd (3.0)
    */markup.bcf    -> markup.xsd
    */*.bcfv        -> visinfo.xsd
    */*.png         -> PNG signature, and referenced by the topic's markup

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


PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


def check_snapshot(z: zipfile.ZipFile, entry: str) -> list[str]:
    """A snapshot has no schema: it must be a PNG, and its topic's markup
    must reference it, or it is an orphan no viewer will show."""
    problems = []
    if not z.read(entry).startswith(PNG_SIGNATURE):
        problems.append("not a PNG (no signature)")
    topic, name = entry.rsplit("/", 1)
    markup = etree.fromstring(z.read(f"{topic}/markup.bcf"))
    if name not in {s.text for s in markup.iter("Snapshot")}:
        problems.append(f"not referenced by {topic}/markup.bcf")
    return problems


def validate_archive(path: Path) -> list[str]:
    problems = []
    with zipfile.ZipFile(path) as z:
        version = declared_version(z)
        if version not in ("2.1", "3.0"):
            return [f"{path.name}: unexpected VersionId {version!r}"]
        for entry in z.namelist():
            if entry.endswith(".png"):
                for err in check_snapshot(z, entry):
                    problems.append(f"{path.name}:{entry}: {err}")
                continue
            xsd = schema_for(entry, version)
            if xsd is None:
                problems.append(f"{path.name}:{entry}: no schema for this entry kind")
                continue
            for err in errors(xsd, z.read(entry)):
                problems.append(f"{path.name}:{entry}: {err}")
    return problems


# (label, sample archive stem, entry predicate, find, replace) — each must be
# rejected. Every one guards a schema rule the writer relies on.
NEGATIVE_CONTROLS = [
    (
        "3.0 viewpoint without a camera",
        "sample-3.0",
        lambda e: e.endswith(".bcfv"),
        (b"<PerspectiveCamera>", b"</PerspectiveCamera>"),
        None,
    ),
    (
        "3.0 uppercase topic GUID",
        "sample-3.0",
        lambda e: e.endswith("/markup.bcf"),
        b'Topic Guid="3f2504e0',
        b'Topic Guid="3F2504E0',
    ),
    (
        "2.1 comment elements out of sequence order",
        "sample-2.1",
        lambda e: e.endswith("/markup.bcf"),
        b"<Date>2026-09-26T10:00:00Z</Date>\n    <Author>checker@example.com</Author>",
        b"<Author>checker@example.com</Author>\n    <Date>2026-09-26T10:00:00Z</Date>",
    ),
    (
        "2.1 IfcGuid of 21 characters",
        "sample-2.1",
        lambda e: e.endswith(".bcfv"),
        b'IfcGuid="0fXw$sQh19ixbI4tZgfkXu"',
        b'IfcGuid="0fXw$sQh19ixbI4tZgfkX"',
    ),
    (
        "2.1 malformed CreationDate",
        "sample-2.1",
        lambda e: e.endswith("/markup.bcf"),
        b"<CreationDate>2026-09-26T10:00:00Z</CreationDate>",
        b"<CreationDate>2026-09-26 10:00</CreationDate>",
    ),
    (
        "2.1 lowercase Color",
        "sample-2.1-styled",
        lambda e: e.endswith(".bcfv"),
        b'Color="800000FF"',
        b'Color="800000ff"',
    ),
    (
        "3.0 Color without its Components wrapper",
        "sample-3.0-styled",
        lambda e: e.endswith(".bcfv"),
        (b'<Color Color="FF0000">\n        <Components>', b"</Components>\n      </Color>"),
        b'<Color Color="FF0000"><Component IfcGuid="0fXw$sQh19ixbI4tZgfkXu"/></Color>',
    ),
    (
        "2.1 Components without the required Visibility",
        "sample-2.1-styled",
        lambda e: e.endswith(".bcfv"),
        b'<Visibility DefaultVisibility="true"/>',
        b"",
    ),
    (
        "3.0 AssignedTo before DueDate",
        "sample-3.0-review",
        lambda e: e.endswith("/markup.bcf"),
        (b"<DueDate>2026-10-15T17:00:00+02:00</DueDate>\n    <AssignedTo>", b"</AssignedTo>"),
        b"<AssignedTo>reviewer@example.com</AssignedTo>\n    <DueDate>2026-10-15T17:00:00+02:00</DueDate>",
    ),
    (
        "3.0 Snapshot before Viewpoint",
        "sample-3.0-review",
        lambda e: e.endswith("/markup.bcf"),
        (b"<Viewpoint>Viewpoint_e4f5a6b7", b"</Snapshot>"),
        b"<Snapshot>Snapshot_e4f5a6b7-c8d9-4e0f-9a1b-2c3d4e5f6a7b.png</Snapshot>\n"
        b"        <Viewpoint>Viewpoint_e4f5a6b7-c8d9-4e0f-9a1b-2c3d4e5f6a7b.bcfv</Viewpoint>",
    ),
    (
        "2.1 Snapshot before Viewpoint",
        "sample-2.1-review",
        lambda e: e.endswith("/markup.bcf"),
        (b"<Viewpoint>Viewpoint_e4f5a6b7", b"</Snapshot>"),
        b"<Snapshot>Snapshot_e4f5a6b7-c8d9-4e0f-9a1b-2c3d4e5f6a7b.png</Snapshot>\n"
        b"    <Viewpoint>Viewpoint_e4f5a6b7-c8d9-4e0f-9a1b-2c3d4e5f6a7b.bcfv</Viewpoint>",
    ),
    (
        "2.1 empty Exceptions",
        "sample-2.1-styled",
        lambda e: e.endswith(".bcfv"),
        b'<Visibility DefaultVisibility="false"/>',
        b'<Visibility DefaultVisibility="false"><Exceptions/></Visibility>',
    ),
]


def mutate(data: bytes, find, replace) -> bytes | None:
    if isinstance(find, tuple):
        # Replace the span from the first `start` through the next `end`.
        start, end = find
        i = data.find(start)
        j = data.find(end, i) if i >= 0 else -1
        if i < 0 or j < 0:
            return None
        return data[:i] + (replace or b"") + data[j + len(end):]
    if find not in data:
        return None
    return data.replace(find, replace, 1)


def run_negative_controls(archives: dict[str, Path]) -> list[str]:
    failures = []
    for label, stem, pick, find, replace in NEGATIVE_CONTROLS:
        rejected = anchored = False
        if stem not in archives:
            failures.append(f"negative control targets missing sample {stem}: {label}")
            continue
        with zipfile.ZipFile(archives[stem]) as z:
            version = declared_version(z)
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
            problems.extend(run_negative_controls({p.stem: p for p in archives}))

    if problems:
        print("\nFAIL:")
        for p in problems:
            print(f"  {p}")
        return 1
    print(f"\nall {len(archives)} written archives are schema-valid.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
