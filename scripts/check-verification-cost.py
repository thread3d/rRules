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

import os
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
    """Unique crate names in `crate`'s dependency tree over `edges`.

    Tries `--offline` first: resolving a tree needs the lockfile, not the network, and a check that
    touches the registry fails on a machine whose cargo home is read-only — which says nothing about the
    dependency weight it is measuring. Falls back to a resolving run when the local index is too stale to
    answer, and says where to point `CARGO_HOME` when both fail, because this sandbox's `$HOME` is
    read-only and that is the usual cause.
    """
    base = ["cargo", "tree", "-p", crate, "-e", edges, "--prefix", "none"]

    # This sandbox's `$HOME` is read-only, so an unset `CARGO_HOME` means cargo cannot even read its own
    # cache consistently. Use the workspace cache when it is there: a gate inside `make ci` should not
    # require the caller to know which shell variable its environment needs.
    env = dict(os.environ)
    if not env.get("CARGO_HOME") and (REPO / ".cargo-home").is_dir():
        env["CARGO_HOME"] = str(REPO / ".cargo-home")

    for extra in (["--offline"], []):
        result = subprocess.run(
            base[:2] + extra + base[2:], cwd=REPO, capture_output=True, text=True, env=env
        )
        if result.returncode == 0:
            names = set()
            for line in result.stdout.splitlines():
                parts = line.split()
                if len(parts) >= 2 and parts[1].startswith("v"):
                    names.add(parts[0])
            return names
        last = result

    raise SystemExit(
        f"verification-cost: `cargo tree` failed for {crate}:\n{last.stderr.strip()}\n"
        f"If the message mentions a read-only file system, point cargo at a writable home: "
        f"CARGO_HOME=$PWD/.cargo-home make verification-cost"
    )


def workspace_heavy_members() -> list[str]:
    """Workspace members whose dependency trees carry the forbidden heavy crates.

    A workspace-wide build compiles every member, so a heavy member starves the verification just as a
    heavy dev-dependency does. `db_tests` was a member for two releases and doubled the engine's
    verification build time before this check existed.
    """
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"], cwd=REPO, capture_output=True, text=True
    )
    if result.returncode != 0:
        return []
    import json

    metadata = json.loads(result.stdout)
    heavy = []
    for package in metadata.get("packages", []):
        for dep in package.get("dependencies", []):
            # Only a normal, non-optional dependency is compiled by a workspace build. An optional one
            # sits behind a feature (`api`'s `sqlx` behind `postgres`) and a dev-dependency only in that
            # crate's own tests (`engine_state`'s `pg-embed`) — neither reaches the verification build.
            if dep.get("kind") is not None or dep.get("optional"):
                continue
            if dep["name"] in FORBIDDEN_DEV:
                heavy.append(
                    f"{package['name']} (a workspace member) has a required dependency on `{dep['name']}`"
                )
    return heavy


def main() -> int:
    problems = list(workspace_heavy_members())
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
