#!/usr/bin/env python3
"""Compile the Rust examples in `docs/user-guide.md`.

A guide that shows code is making a claim about the public API. The claim rots exactly like any
other: a type is renamed, a builder method changes, an import that was implicit in the author's
editor is missing from the snippet. The reader then copies something that does not build.

This script extracts every ```rust block, wraps each in a function, writes a throwaway crate inside
`target/` (never committed) that depends on the engine crate by path, and compiles it. Compilation
is the assertion — the snippets describe how to *construct* rules, so building them is what they
promise.

Usage:
  scripts/check-doc-snippets.py [--doc docs/user-guide.md]

Exit codes: 0 every snippet compiles, 1 something does not (the compiler's message is shown).
"""

from __future__ import annotations

import argparse
import pathlib
import re
import shutil
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
SNIPPET = re.compile(r"```rust\n(.*?)```", re.DOTALL)


def blocks(doc: pathlib.Path) -> list[str]:
    return [match.group(1) for match in SNIPPET.finditer(doc.read_text(encoding="utf-8"))]


def build_crate(snippets: list[str], workdir: pathlib.Path) -> None:
    shutil.rmtree(workdir, ignore_errors=True)
    (workdir / "src").mkdir(parents=True)
    (workdir / "Cargo.toml").write_text(
        "[package]\n"
        'name = "doc-snippets"\n'
        'version = "0.0.0"\n'
        'edition = "2021"\n'
        "\n"
        "[dependencies]\n"
        f'engine = {{ path = "{REPO / "engine"}" }}\n'
        # A guide example may name the uuid type it stores in a fact; the reader would add it,
        # so the check does too.
        'uuid = "1"\n'
        "\n"
        "[workspace]\n"
        "\n",
        encoding="utf-8",
    )

    # Each block becomes a function. Snippets end in `let rule = ...;`, which is a statement, so a
    # function body is the right wrapper; anything returning a value is allowed to be unused.
    parts = []
    for index, body in enumerate(snippets, start=1):
        indented = "\n".join("    " + line for line in body.rstrip().splitlines())
        parts.append(f"#[allow(unused_variables, dead_code)]\nfn snippet_{index}() {{\n{indented}\n}}\n")
    parts.append("fn main() {}\n")
    (workdir / "src" / "main.rs").write_text("\n".join(parts), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--doc", type=pathlib.Path, default=REPO / "docs" / "user-guide.md")
    parser.add_argument("--keep", action="store_true", help="leave the generated crate in place")
    args = parser.parse_args()

    snippets = blocks(args.doc)
    if not snippets:
        print(f"doc-snippets: no ```rust blocks in {args.doc}", file=sys.stderr)
        return 1

    workdir = REPO / "target" / "doc-snippets"
    build_crate(snippets, workdir)

    result = subprocess.run(
        ["cargo", "build", "--quiet"],
        cwd=workdir,
        capture_output=True,
        text=True,
        env={**__import__("os").environ, "CCACHE_DISABLE": "1"},
    )
    if result.returncode != 0:
        print(f"doc-snippets: {len(snippets)} blocks from {args.doc.name}, compilation failed\n")
        print(result.stderr.strip())
        if not args.keep:
            shutil.rmtree(workdir, ignore_errors=True)
        return 1

    print(f"doc-snippets: {len(snippets)} blocks from {args.doc.name} compile")
    if not args.keep:
        shutil.rmtree(workdir, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
