#!/usr/bin/env python3
"""Check that the commands the README tells a reader to run actually exist.

A quick start is the first thing anyone runs, and every command in it is a claim: that this target
exists, that this binary is built, that this crate is a member. `make run` pointed at a binary called
`api` for months while the crate builds `rRules-server` — `cargo run --bin api` fails with
`no bin target named api`, and nothing noticed, because nothing ran the README.

What is checked (the mechanical part):

  - every `make <target>` names a target in the `Makefile`;
  - every `cargo run --bin <name>` names a binary a crate declares;
  - every `cargo test -p <name>` / `cargo build -p <name>` names a workspace package;
  - every `npm --prefix <dir>` names a directory that holds a `package.json`.

What is not: whether a command succeeds at runtime. That is what `make ci` is for.

Usage:
  scripts/check-readme.py [--readme README.md]

Exit codes: 0 every command resolves, 1 one does not.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

MAKE_CALL = re.compile(r"\bmake\s+([a-zA-Z][\w-]*)")
CARGO_BIN = re.compile(r"cargo\s+run\s+--bin\s+([\w-]+)")
CARGO_PACKAGE = re.compile(r"cargo\s+(?:test|build|check|run)\s+(?:[^-\n]*?)-p\s+([\w-]+)")
NPM_PREFIX = re.compile(r"npm\s+--prefix\s+([\w./-]+)")


def make_targets() -> set[str]:
    text = (REPO / "Makefile").read_text(encoding="utf-8")
    return set(re.findall(r"(?m)^([a-zA-Z][\w-]*):", text))


def declared_binaries() -> set[str]:
    names: set[str] = set()
    for manifest in REPO.rglob("Cargo.toml"):
        if any(part in {".cargo-home", "target", "node_modules"} for part in manifest.parts):
            continue
        text = manifest.read_text(encoding="utf-8", errors="replace")
        declared = set(re.findall(r'(?ms)\[\[bin\]\].*?name\s*=\s*"([\w-]+)"', text))
        if declared:
            names.update(declared)
        elif (manifest.parent / "src" / "main.rs").is_file():
            # With no `[[bin]]` block, cargo names the binary after the package.
            package = re.search(r'(?m)^name\s*=\s*"([\w-]+)"', text)
            if package:
                names.add(package.group(1))
    return names


def workspace_packages() -> set[str]:
    return {
        manifest.parent.name
        for manifest in REPO.glob("*/Cargo.toml")
        if manifest.parent.name not in {"target", "node_modules"}
    }


def bash_blocks(readme: pathlib.Path) -> list[str]:
    text = readme.read_text(encoding="utf-8")
    return re.findall(r"```bash\n(.*?)```", text, re.DOTALL)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--readme", type=pathlib.Path, default=REPO / "README.md")
    args = parser.parse_args()

    targets = make_targets()
    binaries = declared_binaries()
    packages = workspace_packages()

    problems: list[str] = []
    checked = 0

    for block in bash_blocks(args.readme):
        for line in block.splitlines():
            line = line.split("#", 1)[0]
            for target in MAKE_CALL.findall(line):
                checked += 1
                if target not in targets:
                    problems.append(f"`make {target}` — no such target in the Makefile")
            for binary in CARGO_BIN.findall(line):
                checked += 1
                if binary not in binaries:
                    problems.append(
                        f"`cargo run --bin {binary}` — no crate declares that binary "
                        f"(declared: {', '.join(sorted(binaries)) or 'none'})"
                    )
            for package in CARGO_PACKAGE.findall(line):
                checked += 1
                if package not in packages:
                    problems.append(f"`cargo … -p {package}` — not a workspace package")
            for prefix in NPM_PREFIX.findall(line):
                checked += 1
                if not (REPO / prefix / "package.json").is_file():
                    problems.append(f"`npm --prefix {prefix}` — no package.json there")

    print(f"readme commands: {checked} checked in {args.readme.name}, {len(problems)} unresolved")
    for problem in problems:
        print(f"  {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
