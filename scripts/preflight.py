#!/usr/bin/env python3
"""Check the machine before trusting a gate's verdict.

For two rounds `make verify` reported all 35 requirements BLOCKED because the api build exceeded its
budget with **zero diagnostics** — which is not what a compile error looks like. The cause was the
environment: `target/` had reached 862 GB on a filesystem 95% full, and every build was thrashing. A
cold full build and 1010 tests took 57 seconds once that was cleared.

The lesson is cheap to act on and was expensive to learn, so it is a gate: this script measures the
handful of environmental facts that turn "the tool is broken" into "the machine is full", and says what
to do about each. It fails only on conditions that will make the other gates lie.

Usage:
  scripts/preflight.py

Exit codes: 0 the machine can be trusted with a build, 1 it cannot.
"""

from __future__ import annotations

import os
import pathlib
import shutil
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# Free space below this makes a full workspace build thrash; the 862 GB incident had 92 GB free.
DISK_FLOOR_GB = 15
# A `target/` above this has stopped being a build cache and become a liability.
TARGET_CEILING_GB = 100


def gigabytes(path: pathlib.Path) -> float | None:
    """Size of `path` in GB, or None when it does not exist."""
    if not path.exists():
        return None
    total = 0
    for root, _dirs, files in os.walk(path):
        for name in files:
            try:
                total += (pathlib.Path(root) / name).lstat().st_size
            except OSError:
                continue
    return total / (1024**3)


def main() -> int:
    problems: list[str] = []

    print("preflight: can this machine be trusted with a build?")

    # 1. Free space on the filesystem that holds the workspace.
    usage = shutil.disk_usage(REPO)
    free_gb = usage.free / (1024**3)
    used_percent = 100 * usage.used / usage.total
    print(f"  disk      {free_gb:.0f} GB free ({used_percent:.0f}% used, floor {DISK_FLOOR_GB} GB)")
    if free_gb < DISK_FLOOR_GB:
        problems.append(
            f"only {free_gb:.0f} GB free: builds will thrash and gates will time out rather than fail. "
            f"Free space with `cargo clean` and `rm -rf db_tests/target`."
        )

    # 2. The build cache's own size: an enormous `target/` is the same problem with a head start.
    target_gb = gigabytes(REPO / "target")
    if target_gb is not None:
        print(f"  target/   {target_gb:.1f} GB (ceiling {TARGET_CEILING_GB} GB)")
        if target_gb > TARGET_CEILING_GB:
            problems.append(
                f"target/ is {target_gb:.0f} GB: `cargo clean` it. An oversized cache costs more in "
                f"thrashing than a cold rebuild costs in time."
            )

    # 3. A writable cargo home. Without one, cargo fails with a message about the registry that says
    #    nothing about the real problem, and every gate that shells out to cargo fails with it.
    cargo_home = os.environ.get("CARGO_HOME")
    candidates = [pathlib.Path(cargo_home)] if cargo_home else [pathlib.Path.home() / ".cargo"]
    writable = [path for path in candidates if path.exists() and os.access(path, os.W_OK)]
    where = cargo_home or str(pathlib.Path.home() / ".cargo")
    print(f"  cargo     {'writable' if writable else 'NOT writable'}: {where}")
    if not writable:
        problems.append(
            f"the cargo home at {where} is not writable, so cargo cannot update its registry and gates "
            f"fail for the wrong reason. Point it inside the tree: CARGO_HOME=$PWD/.cargo-home make ci"
        )

    # 4. Cache variables that tools need when `$HOME` is read-only.
    for variable in ("XDG_CACHE_HOME", "npm_config_cache", "HELM_CACHE_HOME"):
        value = os.environ.get(variable)
        if value:
            ok = os.access(value, os.W_OK) if pathlib.Path(value).exists() else False
            print(f"  {variable:14} {'writable' if ok else 'NOT writable'}: {value}")
            if not ok:
                problems.append(f"{variable}={value} is not writable")

    if problems:
        print()
        for problem in problems:
            print(f"  FAIL {problem}")
        return 1
    print("  ok        the machine is in a state where a failing gate means failing code")
    return 0


if __name__ == "__main__":
    sys.exit(main())
