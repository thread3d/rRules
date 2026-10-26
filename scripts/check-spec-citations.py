#!/usr/bin/env python3
"""Check that every `path:line` citation in `specs/` still resolves.

The specification earns its keep by citing the code: "this is implemented, here". A
citation to a line that no longer exists, or to a file that moved, is worse than no
citation — it looks like evidence and is not. This script resolves each one:

  - `path/to/file.rs:120` and ranges (`120-140`)             → the file must exist and
    the line must be inside it;
  - a bare `file.rs:120` (no directory)                      → resolved by unique basename
    within the repository;
  - `path:120` inside a code block or prose that is not a file (`Cargo.toml:9`) is
    checked the same way.

This is a bounds check, and only a bounds check. A citation can also be wrong *inside* the file —
a sentence naming `web::scope("/v1")` can cite a line that has since become a comment about
something else — and no mechanical rule tried here separated that from a table row's prose: a
heuristic comparing the named symbol with the cited text flagged 646 of 1162 citations, almost all
of them ordinary loose wording. A checker that cries wolf that often is worse than none, so it was
removed rather than shipped. Citations that resolve but mislead are found by reading.

Exit codes: 0 all citations resolve (or drift under the tolerated count), 1 when
`--strict` and something does not resolve.

`--strict` is what CI runs; the local default reports without failing so a writer can
see the state of a draft.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SPECS = REPO / "specs"

# `path/file.ext:12` or `path/file.ext:12-34`, either in backticks or bare.
CITATION = re.compile(
    r"(?<![\w/])((?:[\w.-]+/)*[\w.-]+\.(?:rs|ts|tsx|toml|ya?ml|sh|md|json)):(\d+)(?:-(\d+))?"
)

SKIP_DIRS = {".git", "target", "node_modules", "dist", ".codewitness", "corpus", "artifacts"}



def repo_files() -> list[Path]:
    out = []
    for root, dirs, files in os.walk(REPO):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not d.startswith(".cargo-home")]
        for name in files:
            if name.endswith((".rs", ".ts", ".tsx", ".toml", ".yaml", ".yml", ".sh", ".md", ".json")):
                out.append(Path(root) / name)
    return out


def build_basename_index(files: list[Path]) -> dict[str, list[Path]]:
    index: dict[str, list[Path]] = {}
    for path in files:
        index.setdefault(path.name, []).append(path)
    return index


def line_count(path: Path) -> int:
    try:
        return sum(1 for _ in path.open("rb"))
    except OSError:
        return 0


def resolve(
    cited: str, index: dict[str, list[Path]], doc_hint: list[Path], last: int = 0
) -> tuple[Path | None, str]:
    resolved = (REPO / cited).resolve()
    if not str(resolved).startswith(str(REPO)):
        return None, "outside the repository (readers cannot open it)"
    direct = REPO / cited
    if direct.is_file():
        return direct, "path"
    if "/" not in cited:
        matches = index.get(cited, [])
        if len(matches) == 1:
            return matches[0], "basename"
        if len(matches) > 1:
            # A bare file name is disambiguated by the document's own full-path
            # citations: a spec that writes `engine/src/dmn/feel/parser.rs` elsewhere
            # means that one.
            hinted = [m for m in matches if m in doc_hint]
            if len(hinted) == 1:
                return hinted[0], "basename+hint"
            # Several same-named files, as `parser.rs` is: accept the citation when at
            # least one of them contains the cited range, and only complain when the
            # line cannot belong to any of them.
            pool = hinted or matches
            if last and any(0 < last <= line_count(m) for m in pool):
                best = next(m for m in pool if 0 < last <= line_count(m))
                return best, "basename-by-range"
            return None, f"ambiguous basename ({len(pool)} files), range fits none"
        return None, "no such file"
    # A path written relative to a crate rather than the repository root.
    suffix = "/" + cited
    matches = [p for p in index.values() for p in p if str(p).endswith(suffix)]
    if len(matches) == 1:
        return matches[0], "suffix"
    return None, "no such file" if not matches else f"ambiguous suffix ({len(matches)} files)"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--strict", action="store_true", help="fail when a citation does not resolve")
    parser.add_argument("--verbose", action="store_true", help="print every citation")
    args = parser.parse_args()

    index = build_basename_index(repo_files())
    total = 0
    problems: list[tuple[str, int, str, str]] = []

    for doc in sorted(SPECS.glob("*.md")):
        text = doc.read_text(encoding="utf-8", errors="replace")
        lines = text.splitlines()
        # Full paths this document cites, used to disambiguate bare file names it also uses.
        doc_hint = []
        for match in CITATION.finditer(text):
            target, _ = resolve(match.group(1), index, [], 0)
            if target is not None:
                doc_hint.append(target)
        for lineno, line in enumerate(lines, start=1):
            for match in CITATION.finditer(line):
                cited, first, last = match.group(1), int(match.group(2)), match.group(3)
                total += 1
                last_num = int(last) if last else first
                target, how = resolve(cited, index, doc_hint, last_num)
                if target is None:
                    problems.append((doc.name, lineno, match.group(0), how))
                    continue
                try:
                    count = sum(1 for _ in target.open("rb"))
                except OSError as error:
                    problems.append((doc.name, lineno, match.group(0), f"unreadable: {error}"))
                    continue
                last = int(last) if last else first
                if first < 1 or last > count:
                    problems.append(
                        (doc.name, lineno, match.group(0), f"line {last} beyond end of file ({count} lines)")
                    )
                if args.verbose:
                    print(f"  ok  {doc.name}:{lineno} → {target.relative_to(REPO)} [{how}]")

    print(f"spec citations: {total} checked, {len(problems)} unresolved")
    for doc, lineno, cited, why in problems[:40]:
        print(f"  {doc}:{lineno}: `{cited}` — {why}")
    if len(problems) > 40:
        print(f"  … and {len(problems) - 40} more")

    if args.strict and problems:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
