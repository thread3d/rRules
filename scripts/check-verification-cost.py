#!/usr/bin/env python3
"""Keep the dependency weight of the verified crates where the verification can afford it.

In 9.0.45 the API's PostgreSQL tests were added to `api/tests/`, which made `pg-embed` a **dev-
dependency of `api`**. Every build and every coverage pass of that crate then compiled the embedded
server, and `make verify` — which builds the crate and measures it — began exhausting its budget and
reporting all 35 requirements BLOCKED. Nothing noticed for ten releases because a blocked run still
prints a report; it took raising the timeout to expose it.

The tests now live in `db_tests/`, which nothing verifies. This check keeps that true: it fails when the
crates the verification builds grow past what the verification can afford, so the next heavy
dev-dependency is a red gate rather than a silent 30-minute timeout.

Usage:
  scripts/check-verification-cost.py

Exit codes: 0 within budget, 1 over it.
"""

from __future__ import annotations

import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# The crates CodeWitness builds and measures for the requirement verification.
VERIFIED = ("api", "engine")

# Unique crate names in each verified crate's `normal` + `dev` tree, and the ceilings.
#
# `api` sits at 262 after the database tests moved out; the ceiling leaves room for ordinary churn and
# is far below the ~600 the embedded-server tree added. Raise these deliberately, in a commit that says
# why, and re-check `make verify`'s duration in the same change.
CEILINGS = {"api": 320, "engine": 330}

# Crates that must not be dev-dependencies of a verified crate: each drags a large tree in for tests
# that do not need to run during verification.
FORBIDDEN_DEV = ("pg-embed", "sqlx", "reqwest", "zip")


def tree(crate: str, edges: str) -> set[str]:
    result = subprocess.run(
        ["cargo", "tree", "-p", crate, "-e", edges, "--prefix", "none"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"verification-cost: `cargo tree` failed for {crate}:\n{result.stderr.strip()}")
    names = set()
    for line in result.stdout.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[1].startswith("v"):
            names.add(parts[0])
    return names


def main() -> int:
    problems = []
    print("verification-cost: what the requirement verification builds")
    for crate in VERIFIED:
        normal_dev = tree(crate, "normal,dev")
        dev_only = tree(crate, "dev") - tree(crate, "normal")
        ceiling = CEILINGS[crate]
        print(f"  {crate:8} {len(normal_dev):4} crates (normal+dev), ceiling {ceiling}")

        if len(normal_dev) > ceiling:
            problems.append(
                f"{crate}: {len(normal_dev)} crates exceeds the ceiling of {ceiling}; the verification "
                f"builds and measures this tree, and a heavier one starves its budget (see "
                f"db_tests/Cargo.toml for what that looked like)"
            )

        for crate_name in sorted(FORBIDDEN_DEV):
            if crate_name in dev_only:
                problems.append(
                    f"{crate}: `{crate_name}` is a dev-dependency; heavy test-only trees belong in a "
                    f"crate the verification does not build (the `db_tests` pattern)"
                )

    for problem in problems:
        print(f"  FAIL {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
