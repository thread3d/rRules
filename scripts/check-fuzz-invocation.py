#!/usr/bin/env python3
"""Check that the fuzz recipes give `--target` to cargo-fuzz, not to libFuzzer.

`cargo fuzz` does not default `--target` to the host it runs on: `utils::default_target()` returns
`current_platform::CURRENT_PLATFORM`, the triple the *cargo-fuzz binary* was compiled for. The prebuilt
musl binary that `taiki-e/install-action` installs therefore asks cargo for
`x86_64-unknown-linux-musl`, on a runner that has neither that target's std nor a sanitizer that
accepts a statically linked libc ("sanitizer is incompatible with statically linked libc").

`make fuzz-build` passed `--target` to cargo-fuzz and succeeded; `make fuzz` put it after the `--`
that hands the remaining arguments to libFuzzer, which ignored it. The nightly Fuzz job on f84fcc1
failed exactly there, and nothing in the tree noticed, because the two recipes that must agree were
two implementations. This script reads the commands the Makefile actually produces (`make -n`) and
fails when `--target` is missing from cargo-fuzz's own options, when it appears among libFuzzer's
arguments, or when the two recipes name different triples.

Usage:
  scripts/check-fuzz-invocation.py

Exit codes: 0 both recipes give cargo-fuzz one target, 1 one of them does not.
"""

from __future__ import annotations

import os
import pathlib
import re
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# A triple for the check itself. Any triple would do; fixing it keeps the verdict independent of the
# machine, and setting it in the environment stops make from evaluating its `rustc` default at all.
TRIPLE = "x86_64-unknown-linux-gnu"

# The `--` cargo-fuzz uses to hand the rest of the line to the fuzz target (libFuzzer).
SEPARATOR = " -- "

FUZZ_INVOCATION = re.compile(r"cargo \+nightly fuzz (build|run)\b")


def invocations(dry_run: list[str]) -> list[tuple[str, str]]:
    """The `cargo +nightly fuzz ...` commands `make -n <dry_run>` prints, as (kind, command)."""
    environment = dict(os.environ, FUZZ_TARGET_TRIPLE=TRIPLE)
    result = subprocess.run(
        ["make", "-n", "-B", *dry_run],
        cwd=REPO,
        capture_output=True,
        text=True,
        env=environment,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(f"make -n {' '.join(dry_run)} failed:\n{result.stderr}")

    found = []
    text = result.stdout
    for match in FUZZ_INVOCATION.finditer(text):
        # A recipe line ends at the closing parenthesis of the subshell the command runs in; nothing
        # make expands contains one, so the first `)` after the match is the end of the command.
        end = text.find(")", match.end())
        found.append((match.group(1), text[match.start() : end if end != -1 else len(text)]))
    return found


def problems_in(commands: list[tuple[str, str]]) -> list[str]:
    """Every way a set of fuzz commands can disagree about the target triple."""
    problems: list[str] = []
    seen = {"build": 0, "run": 0}

    for kind, command in commands:
        seen[kind] += 1
        if kind == "run":
            if SEPARATOR not in command:
                problems.append(
                    f"`cargo fuzz run` passes no libFuzzer arguments: {command}"
                )
                continue
            options, fuzz_arguments = command.split(SEPARATOR, 1)
        else:
            options, fuzz_arguments = command, ""

        if f"--target {TRIPLE}" not in options:
            problems.append(f"cargo-fuzz is not given --target {TRIPLE}: {command}")
        if "--target" in fuzz_arguments:
            problems.append(
                "--target is after the `--`, so it reaches libFuzzer and cargo-fuzz falls back to "
                f"the triple it was compiled for: {command}"
            )

    # A checker that found nothing to check has not passed — that is how five tests once asserted
    # over an empty list of engines.
    for kind, label in (("build", "`make fuzz-build`"), ("run", "`make fuzz`")):
        if seen[kind] == 0:
            problems.append(f"{label} produced no `cargo +nightly fuzz {kind}` command to check")
    return problems


def self_test() -> list[str]:
    """Exercise the checker on the defect it was written for, and on the fixed form."""
    buggy = [
        ("build", f"cd engine && cargo +nightly fuzz build --target {TRIPLE}"),
        ("run", f"cargo +nightly fuzz run feel_parser{SEPARATOR}--target {TRIPLE} -runs=1"),
    ]
    fixed = [
        ("build", f"cd engine && cargo +nightly fuzz build --target {TRIPLE}"),
        ("run", f"cargo +nightly fuzz run --target {TRIPLE} feel_parser{SEPARATOR}-runs=1"),
    ]
    problems: list[str] = []
    if not problems_in(buggy):
        problems.append("the self-test accepts the buggy form: a `--target` after `--` went unnoticed")
    if problems_in(fixed):
        problems.append(f"the self-test rejects the fixed form: {problems_in(fixed)}")
    if not problems_in([]):
        problems.append("the self-test accepts an empty command list as a pass")
    return problems


def main() -> int:
    problems = self_test()
    print(f"fuzz invocation: self-test {'ok' if not problems else 'FAILED'}")

    # Every recipe that can name a triple, judged together: the defect being guarded against is the
    # two of them disagreeing, which a per-recipe verdict could not see.
    commands: list[tuple[str, str]] = []
    for dry_run in (["fuzz-build"], ["fuzz", "TARGET=feel_parser"], ["fuzz"]):
        commands.extend(invocations(dry_run))
    problems.extend(problems_in(commands))

    print(f"fuzz invocation: {len(problems)} problem(s) in the recipes")
    for problem in problems:
        print(f"  {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
