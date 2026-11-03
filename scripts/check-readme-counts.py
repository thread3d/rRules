#!/usr/bin/env python3
"""Check that the test totals the README quotes are the totals this tree produces.

The README's "Measured" table states how many tests the tree has. That number has been wrong three
times — "1005 across 38", then "996 across 40", then 996 again after the suite had grown to 1063 —
because a hand-written count goes stale the moment someone adds a test, and nothing executes it. This
gate executes it: it lists the tests instead of running them, which is cheap once `make test` has
built the binaries (that is why it sits after `test` in `make ci`).

What is measured, and what "test target" means:

  - **Rust tests** — `cargo test --workspace -- --list`, counting the lines libtest prints as
    `<name>: test`. It includes `#[ignore]`d tests and doc tests, so the total is the whole suite.
  - **ignored** — the same listing with `--list --ignored`.
  - **test targets** — the sections cargo runs a harness in: one per unit-test and integration-test
    binary (`Running …`) plus rustdoc's doc-test pass (`Doc-tests …`).
  - **frontend tests** — `npx vitest list --json` in `frontend/`, which lists without running.

Usage:
  scripts/check-readme-counts.py [--readme README.md]

Exit codes: 0 the README agrees with the tree, 1 it does not, 2 the measurement itself failed.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# `| Tests | **1069 Rust tests across 48 test targets**, 6 of them ignored, plus **43 frontend tests** — … |`
CLAIM = re.compile(
    r"\*\*(?P<total>\d+) Rust tests across (?P<targets>\d+) test targets\*\*,\s*"
    r"(?P<ignored>\d+) of them ignored, plus \*\*(?P<frontend>\d+) frontend tests\*\*"
)
TEST_LINE = re.compile(r": test$")
TARGET_LINE = re.compile(r"^\s+(?:Running|Doc-tests)\s")
# Cargo colourises the `Running …` banner when `CARGO_TERM_COLOR` is set — which `.github/workflows/ci.yml`
# does (`CARGO_TERM_COLOR: always`), so the line starts with an escape sequence rather than whitespace and
# an anchored match counts zero targets. That is exactly how this gate first ran in CI: 1069 tests in
# "0 test targets". The escape bytes are stripped before matching, and the child is asked for no colour.
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def cargo_list(extra: tuple[str, ...] = ()) -> tuple[int, int, str]:
    """Return (tests, targets, stderr) for `cargo test --workspace -- --list [--ignored]`."""
    command = ["cargo", "test", "--workspace", "--", "--list", *extra]
    env = {**os.environ, "CARGO_TERM_COLOR": "never"}
    result = subprocess.run(
        command, cwd=REPO, capture_output=True, text=True, check=False, env=env
    )
    if result.returncode != 0:
        raise SystemExit(
            f"readme counts: `{' '.join(command)}` failed ({result.returncode}):\n"
            f"{ANSI.sub('', result.stderr).strip()[-2000:]}"
        )
    stdout = ANSI.sub("", result.stdout)
    stderr = ANSI.sub("", result.stderr)
    tests = sum(1 for line in stdout.splitlines() if TEST_LINE.search(line))
    targets = sum(1 for line in stderr.splitlines() if TARGET_LINE.search(line))
    return tests, targets, stderr


def frontend_tests() -> int:
    result = subprocess.run(
        ["npx", "vitest", "list", "--json"],
        cwd=REPO / "frontend",
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"readme counts: `npx vitest list --json` failed ({result.returncode}):\n"
            f"{result.stderr.strip()[-2000:]}"
        )
    listed = json.loads(result.stdout)
    return len(listed)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--readme", type=pathlib.Path, default=REPO / "README.md")
    args = parser.parse_args()

    text = args.readme.read_text(encoding="utf-8")
    claim = CLAIM.search(text)
    if claim is None:
        print(
            f"readme counts: no measured-tests row found in {args.readme.name}. Expected a line of the\n"
            "  form: | Tests | **<n> Rust tests across <n> test targets**, <n> of them ignored, plus\n"
            "  **<n> frontend tests** — all green |",
            file=sys.stderr,
        )
        return 2

    tests, targets, _ = cargo_list()
    ignored, _, _ = cargo_list(("--ignored",))
    frontend = frontend_tests()

    measured = {
        "total": tests,
        "targets": targets,
        "ignored": ignored,
        "frontend": frontend,
    }
    stated = {key: int(claim.group(key)) for key in measured}

    print(
        f"readme counts: {tests} Rust tests in {targets} test targets ({ignored} ignored), "
        f"{frontend} frontend tests"
    )
    problems = [
        f"{key}: README says {stated[key]}, the tree produced {measured[key]}"
        for key in ("total", "targets", "ignored", "frontend")
        if stated[key] != measured[key]
    ]
    if problems:
        for problem in problems:
            print(f"  {problem}")
        print(
            f"  Update the measured-tests row in {args.readme.name}: "
            f"**{tests} Rust tests across {targets} test targets**, {ignored} of them ignored, "
            f"plus **{frontend} frontend tests**"
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
