#!/usr/bin/env python3
"""Report the verification as one result instead of two halves.

`engine` can speak to 28 of the 35 requirements and `api` to the other 7 — neither can
execute the other's. Run separately, each therefore reports the other half as UNVERIFIED,
and a reader sees "28 verified, 28 unverified" for a specification in which every
requirement is in fact verified by one crate or the other.

This script unions the two latest reports by requirement id and states the truth once. It
exits non-zero if any requirement is FAILED, or if a requirement is unverified in *both*
halves — which is a real gap rather than a division of labour.

Usage:
  scripts/verify-summary.py [--engine PATH] [--api PATH]
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


def latest_report(root: Path) -> Path | None:
    runs = sorted(
        (path for path in (root / ".codewitness" / "runs").glob("*/report.json")),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    return runs[0] if runs else None


def load(path: Path) -> dict[str, dict]:
    report = json.loads(path.read_text())
    return {item["id"]: item for item in report.get("requirements", [])}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--engine", type=Path, default=None)
    parser.add_argument("--api", type=Path, default=None)
    args = parser.parse_args()

    engine_path = args.engine or latest_report(REPO / "engine")
    api_path = args.api or latest_report(REPO / "api")
    if not engine_path or not api_path:
        print("verify-summary: no reports found; run `make verify` first", file=sys.stderr)
        return 2

    engine, api = load(engine_path), load(api_path)
    ids = sorted(set(engine) | set(api))

    # A requirement is verified when *either* crate executed it.
    verified, failed, unverified = [], [], []
    for rid in ids:
        statuses = [report[rid]["status"] for report in (engine, api) if rid in report]
        if "FAILED" in statuses:
            failed.append((rid, statuses))
        elif "VERIFIED" in statuses:
            verified.append(rid)
        elif "BLOCKED" in statuses and "UNVERIFIED" not in statuses:
            unverified.append((rid, statuses))
        else:
            unverified.append((rid, statuses))

    print(f"verification union over {len(ids)} requirements")
    print(f"  engine report: {engine_path.relative_to(REPO)}")
    print(f"  api report   : {api_path.relative_to(REPO)}")
    print(f"  verified   : {len(verified)}")
    print(f"  failed     : {len(failed)}")
    print(f"  unverified : {len(unverified)}")
    for rid, statuses in failed:
        print(f"  FAILED {rid}: {statuses}")
    for rid, statuses in unverified:
        print(f"  unverified {rid}: {statuses}")

    if failed:
        return 1
    if unverified:
        print(
            "\nverify-summary: a requirement that neither crate executes is a gap in the "
            "verification, not a division of labour",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
