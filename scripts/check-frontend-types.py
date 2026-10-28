#!/usr/bin/env python3
"""Compare the interface's declared response shapes with the keys the API actually sends.

A TypeScript interface asserts something no compiler can check: that the server sends those fields.
When it does not, the interface is wrong and every caller is silently wrong with it — fields arrive
`undefined`, dates compare as `NaN`, and statuses never match. This project has paid for that twice: the
rule list mapped nothing and "every field arrived as `undefined`", and `decisions.ts` expected a
`model_version` the backend never sent. Both were fixed by hand, and nothing stopped the next one.

This script reads the JSON a handler builds (`serde_json::json!({ "key": …, … })` in the Rust source) and
the field names in the interface that describes it, and fails when the sets differ. It is deliberately
narrow — a declared table of pairs, one handler and one interface each — because a general parser would
be a bigger thing to trust than the check is worth. Pairs are added as they are needed, and each one is
a contract that now cannot drift silently.

Usage:
  scripts/check-frontend-types.py

Exit codes: 0 every declared pair agrees, 1 one does not.
"""

from __future__ import annotations

import pathlib
import re
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

# (Rust file, handler **or** response struct, TypeScript file, interface). A handler is matched by name
# and its `json!({…})` keys are read; a struct has its serde field names read, renames honoured. The
# interface is read from the given file.
PAIRS = [
    (
        "api/src/v1/decisions.rs",
        "list_decisions",
        "frontend/src/api/decisions.ts",
        "DecisionListResponse",
    ),
    (
        "api/src/v1/decisions.rs",
        "get_decision",
        "frontend/src/api/decisions.ts",
        "DecisionDocument",
    ),
    (
        "api/src/v1/decisions.rs",
        "get_decision_ir",
        "frontend/src/api/decisions.ts",
        "DmnIrResponse",
    ),
    (
        "api/src/v1/decisions.rs",
        "reload_dmn",
        "frontend/src/api/decisions.ts",
        "ReloadDmnResponse",
    ),
    (
        "api/src/v1/rules.rs",
        "RuleListResponse",
        "frontend/src/api/types.ts",
        "PaginatedRulesResponse",
    ),
]

JSON_KEY = re.compile(r'"([a-z_][a-z0-9_]*)":')
INTERFACE_FIELD = re.compile(r'^\s{2}([a-z_][a-z0-9_]*)\??:', re.M)
FN_START = re.compile(r'^\s*pub async fn (\w+)\(', re.M)


def handler_keys(path: pathlib.Path, handler: str) -> set[str]:
    text = path.read_text(encoding="utf-8")
    match = FN_START.search(text, 0)
    start = None
    for match in FN_START.finditer(text):
        if match.group(1) == handler:
            start = match.start()
            break
    if start is None:
        raise SystemExit(f"check-frontend-types: {handler} not found in {path}")

    # The next handler (or the end of the impl) bounds this one.
    following = FN_START.search(text, start + 1)
    body = text[start : following.start() if following else len(text)]

    keys: set[str] = set()
    for block in re.findall(r"json!\(\{(.*?)\}\)", body, re.S):
        found = set(JSON_KEY.findall(block))
        # An interface describes the success payload; the error branches in the same handler build a
        # different shape (`{"error": …}`) and comparing them would report a difference that is not one.
        if found == {"error"}:
            continue
        keys.update(found)
    if not keys:
        raise SystemExit(
            f"check-frontend-types: {handler} builds no `json!({{…}})` response, so its shape cannot be "
            f"compared — declare the pair against a handler that does"
        )
    return keys


def struct_keys(path: pathlib.Path, struct: str) -> set[str]:
    """The JSON keys a `#[derive(Serialize)]` struct emits, honouring `#[serde(rename = "…")]`."""
    text = path.read_text(encoding="utf-8")
    start = text.find(f"pub struct {struct} ")
    if start == -1:
        raise SystemExit(f"check-frontend-types: struct {struct} not found in {path}")
    end = text.index("\n}", start)
    body = text[start:end]

    keys = set()
    renamed = None
    for line in body.splitlines():
        rename = re.search(r'#\[serde\(rename = "([^"]+)"\)\]', line)
        if rename:
            renamed = rename.group(1)
            continue
        field = re.match(r"\s+pub (\w+):", line)
        if field:
            keys.add(renamed or field.group(1))
            renamed = None
    if not keys:
        raise SystemExit(f"check-frontend-types: struct {struct} has no readable fields")
    return keys


def interface_fields(path: pathlib.Path, interface: str) -> set[str]:
    text = path.read_text(encoding="utf-8")
    start = text.find(f"export interface {interface} ")
    if start == -1:
        raise SystemExit(f"check-frontend-types: {interface} not found in {path}")
    # Interfaces here are flat; stop at the closing brace of this one.
    end = text.index("\n}", start)
    return set(INTERFACE_FIELD.findall(text[start:end]))


def main() -> int:
    problems = []
    for rust_path, handler, ts_path, interface in PAIRS:
        rust_file = REPO / rust_path
        sent = (
            handler_keys(rust_file, handler)
            if any(m.group(1) == handler for m in FN_START.finditer(rust_file.read_text(encoding="utf-8")))
            else struct_keys(rust_file, handler)
        )
        declared = interface_fields(REPO / ts_path, interface)

        missing = sorted(sent - declared)  # the server sends it, the interface does not declare it
        invented = sorted(declared - sent)  # the interface declares it, the server never sends it

        status = "ok" if not missing and not invented else "FAIL"
        print(f"  {status:4} {handler:18} → {interface:24} {len(sent)} keys")
        if missing:
            problems.append(f"{interface}: the API sends {missing}, which the interface does not declare")
        if invented:
            problems.append(
                f"{interface}: declares {invented}, which `{handler}` never sends — a caller reading "
                f"them gets `undefined`"
            )

    if problems:
        print()
        for problem in problems:
            print(f"  {problem}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
