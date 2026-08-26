#!/usr/bin/env python3
"""Prove the gate can fail: inject known defects and require detection.

A passing test suite is worthless if it passes for the wrong reasons. Each
mutation below is a plausible implementation mistake; the gate must reject
every one. Any mutation that survives marks an untested claim.

Usage:
    ./scripts/mutation-probes.py           # unit + integration probes
    BCF_OFFICIAL_CORPUS=... BCF_FIELD_CORPUS=... ./scripts/mutation-probes.py
"""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

# (label, file, find, replace) — each must break at least one test.
MUTATIONS: list[tuple[str, str, str, str]] = [
    (
        "version conflict resolved silently instead of surfaced",
        "openbim-bcf/src/version.rs",
        "(Some(declared), Some(observed)) => Detected::Conflict { declared, observed },",
        "(Some(declared), Some(_observed)) => Detected::Declared(declared),",
    ),
    (
        "inferred version reported as declared",
        "openbim-bcf/src/version.rs",
        "(None, Some(o)) => Detected::Inferred(o),",
        "(None, Some(o)) => Detected::Declared(o),",
    ),
    (
        "unknown VersionId rounded to the nearest known version",
        "openbim-bcf/src/version.rs",
        '            _ => None,\n        }\n    }',
        '            _ => Some(BcfVersion::V2_1),\n        }\n    }',
    ),
    (
        "2.0 comment back-reference treated as a second topic",
        "openbim-bcf/src/markup.rs",
        'c.child("Topic").is_some()\n            || c.child("Status").is_some()',
        'false\n            || c.child("Status").is_some()',
    ),
    (
        "3.0 collections nested in Topic go unnoticed",
        "openbim-bcf/src/markup.rs",
        'let nests_collections = topic.child("Comments").is_some()',
        'let nests_collections = false && topic.child("Comments").is_some()',
    ),
    (
        "2.x Viewpoints element misread as a 3.0 wrapper",
        "openbim-bcf/src/markup.rs",
        '.any(|v| v.child("ViewPoint").is_some())\n    {\n        return Some(BcfVersion::V3_0);',
        '.next().is_some()\n    {\n        return Some(BcfVersion::V3_0);',
    ),
    (
        "attribute TopicStatus treated as 2.1-only evidence",
        "openbim-bcf/src/markup.rs",
        "    // --- 2.x, version indeterminate ---------------------------------------",
        '    if topic.attr("TopicStatus").is_some() {\n        return Some(BcfVersion::V2_1);\n    }\n    // --- 2.x, version indeterminate ---------------------------------------',
    ),
    (
        "TopicStatus normalised instead of kept verbatim",
        "openbim-bcf/src/markup.rs",
        'topic_status: owned(node.attr_or_child_text("TopicStatus")),',
        'topic_status: owned(node.attr_or_child_text("TopicStatus")).map(|s| s.to_lowercase()),',
    ),
    (
        "missing Topic Guid silently accepted without a diagnostic",
        "openbim-bcf/src/markup.rs",
        "        diagnostics.push(Diagnostic::in_entry(entry, Tolerance::TopicWithoutGuid));",
        "        let _ = &diagnostics;",
    ),
    (
        "missing bcf.version not reported",
        "openbim-bcf/src/archive.rs",
        "                diagnostics.push(Diagnostic::in_archive(Tolerance::MissingVersionEntry));\n                None\n            }\n            Some((index, name)) => {",
        "                None\n            }\n            Some((index, name)) => {",
    ),
    (
        "path traversal entry tolerated instead of refused",
        "openbim-bcf/src/archive.rs",
        "            if file.enclosed_name().is_none() {\n                return Err(BcfError::UnsafeEntry { name: raw });\n            }",
        "",
    ),
    (
        "total uncompressed size limit not enforced",
        "openbim-bcf/src/archive.rs",
        "            if declared_total > limits.max_total_uncompressed {",
        "            if false && declared_total > limits.max_total_uncompressed {",
    ),
    (
        "empty archive returns an empty result instead of an error",
        "openbim-bcf/src/archive.rs",
        "        if markups.is_empty() {\n            return Err(BcfError::NoTopics);\n        }",
        "",
    ),
    (
        "backslash entry names not normalised",
        "openbim-bcf/src/archive.rs",
        "            let name = raw.replace('\\\\', \"/\");",
        "            let name = raw.clone();",
    ),
    (
        "dangling viewpoint references not reported",
        "openbim-bcf/src/archive.rs",
        "        diagnostics.extend(dangling);",
        "        let _ = dangling;",
    ),
    (
        "blank XML values treated as present",
        "openbim-bcf/src/xml.rs",
        "        self.child(name)\n            .map(|c| c.text.trim())\n            .filter(|t| !t.is_empty())",
        "        self.child(name).map(|c| c.text.trim())",
    ),
    (
        "namespace prefixes not stripped from element names",
        "openbim-bcf/src/xml.rs",
        "    match s.rsplit_once(':') {\n        Some((_, local)) => local.to_string(),\n        None => s.into_owned(),\n    }",
        "    s.into_owned()",
    ),
]


def run_gate() -> bool:
    """True when the gate passes."""
    env = dict(os.environ)
    env.setdefault("CARGO_TERM_COLOR", "never")
    proc = subprocess.run(
        ["cargo", "test", "--workspace", "--all-features"],
        cwd=ROOT,
        env=env,
        capture_output=True,
    )
    return proc.returncode == 0


def main() -> int:
    print("baseline: running the gate unmutated ...")
    if not run_gate():
        print("FAIL: the gate does not pass before mutation; fix that first.")
        return 1
    print("baseline: pass\n")

    survivors = []
    for i, (label, rel, find, replace) in enumerate(MUTATIONS, start=1):
        path = ROOT / rel
        original = path.read_text()
        if find not in original:
            print(f"[{i:2}/{len(MUTATIONS)}] ERROR anchor not found in {rel}: {label}")
            survivors.append((label, "anchor missing"))
            continue
        path.write_text(original.replace(find, replace, 1))
        try:
            caught = not run_gate()
        finally:
            path.write_text(original)
        status = "caught" if caught else "SURVIVED"
        print(f"[{i:2}/{len(MUTATIONS)}] {status}: {label}")
        if not caught:
            survivors.append((label, "gate still passed"))

    print()
    if survivors:
        print(f"{len(survivors)}/{len(MUTATIONS)} mutations survived:")
        for label, why in survivors:
            print(f"  - {label} ({why})")
        return 1
    print(f"all {len(MUTATIONS)} mutations caught; the gate can fail.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
