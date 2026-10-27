#!/usr/bin/env python3
"""Check that tests observing process-wide trackers are serialised.

`SYS-003` says the trackers are *resettable*, and they are — but resettability buys nothing under
parallel execution: two tests that each call `clear()` and then record will see each other's records
whatever the reset does. The eleven coverage unit tests proved it in 9.0.47, failing two assertions on a
day when the timing shifted, after passing for months on luck.

So the rule is not "call `clear()`", it is "call `clear()` *and* `#[serial]`". This script enforces the
second half, which nothing did: it walks every test function in the workspace and fails when a test
touches a tracker without serialising itself.

Usage:
  scripts/check-singleton-tests.py

Exit codes: 0 every observing test is serialised, 1 one is not.
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# The process-wide trackers named in `specs/requirements.md` (SYS-003).
TRACKERS = (
    "CoverageTracking",
    "RuleExecutionTracker",
    "DMN_DECISION_TRACKER",
    "DmnDecisionTracker",
)

TEST_ATTRS = ("#[test]", "#[tokio::test]", "#[actix_web::test]")
FN_START = re.compile(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)")
SKIP_DIRS = {".cargo-home", "target", "node_modules", ".codewitness", "fuzz"}


def rust_files() -> list[pathlib.Path]:
    files = []
    for root in ("engine", "api", "engine_state"):
        for path in (REPO / root).rglob("*.rs"):
            if any(part in SKIP_DIRS for part in path.parts):
                continue
            files.append(path)
    return files


def function_body(lines: list[str], start: int) -> list[str]:
    """The lines of the function whose `fn` is at `start`, by brace counting."""
    depth = 0
    seen_open = False
    body = []
    for line in lines[start:]:
        body.append(line)
        for character in line:
            if character == "{":
                depth += 1
                seen_open = True
            elif character == "}":
                depth -= 1
        if seen_open and depth <= 0:
            break
    return body


def offenders() -> list[str]:
    problems = []
    for path in rust_files():
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for index, line in enumerate(lines):
            if line.strip() not in TEST_ATTRS:
                continue

            # Attributes sit on both sides of the test attribute — `#[serial]` is often written
            # above it — so collect upwards to the blank line and downwards to the `fn`.
            attributes = []
            above = index - 1
            while above >= 0 and lines[above].strip().startswith("#["):
                attributes.append(lines[above].strip())
                above -= 1

            cursor = index + 1
            while cursor < len(lines) and not FN_START.match(lines[cursor]):
                if lines[cursor].strip().startswith("#["):
                    attributes.append(lines[cursor].strip())
                cursor += 1
            if cursor >= len(lines):
                continue

            name = FN_START.match(lines[cursor]).group(1)
            body = function_body(lines, cursor)
            if not any(tracker in body_line for body_line in body for tracker in TRACKERS):
                continue
            if any("serial" in attribute for attribute in attributes):
                continue

            relative = path.relative_to(REPO)
            problems.append(f"{relative}:{cursor + 1}  {name} observes a process-wide tracker without #[serial]")
    return problems


def main() -> int:
    problems = offenders()
    print(f"singleton tests: {len(problems)} observing test(s) not serialised")
    for problem in problems:
        print(f"  {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
