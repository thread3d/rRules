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

# (Rust file, handler **or** response struct, TypeScript file, interface, consumer). A handler is matched by
# name and its `json!({…})` keys are read; a struct has its serde field names read, renames honoured. The
# interface is read from the given file. `consumer` is the function that turns the response into what a
# screen shows — **a response field must be read there**, because naming it elsewhere (a comment, a request
# payload the page sends, a test fixture) is not consuming it. `None` means the response is used directly.
# A consumer is written `path::function`, because the interface and the function that consumes it often live in
# different files (`types.ts` declares the shape, `rules.ts` maps it).
PAIRS = [
    # `listDecisions`, `getDecision`, `getDecisionIr` and `reloadDmn` were deleted in 9.0.81: they had no
    # callers, and a correct type nothing consumes is still drift waiting to happen.
    # Declared against the response *struct*, not the handler: the handler also builds error payloads
    # (`error_code`, `message`, `policy`) and reading its `json!` blocks conflated the two shapes.
    (
        "api/src/v1/decisions.rs",
        "EvaluateDecisionResponse",
        "frontend/src/api/decisions.ts",
        "EvaluateDecisionResponse",
        None,
    ),
    (
        "api/src/v1/rules.rs",
        "RuleListResponse",
        "frontend/src/api/types.ts",
        "PaginatedRulesResponse",
        "frontend/src/api/rules.ts::getRules",
    ),
    (
        "api/src/v1/rules.rs",
        "RuleResponse",
        "frontend/src/api/rules.ts",
        "ApiRule",
        "frontend/src/api/rules.ts::toRule",
    ),
    (
        "api/src/v1/audit.rs",
        "AuditEvent",
        "frontend/src/features/governance/audit/AuditLogViewer.tsx",
        "BackendAuditEvent",
        "frontend/src/features/governance/audit/AuditLogViewer.tsx::mapBackendAuditEvent",
    ),
]

# Fields the API sends that the interface is allowed not to read, each with the reason. An entry here is a
# decision someone made, not a tolerance: "the frontend never shows this" has to be true and stay true.
UNREAD_ALLOWED = {
    "EvaluateDecisionResponse": {
        "decision_id": "the caller passed the id in; the editor does not display it back",
        "duration_ms": "the editor shows outputs, not timings",
        "rules_fired": "hit-policy diagnostics for API clients; no screen shows a count",
    },
    "ApiRule": {
        "owner": "always null until ownership is implemented on the API side; no screen has an owner column",
        "tags": "carried on the view model for filtering; no screen renders tag chips yet",
    },
    "RuleResponse": {
        "owner": "the API always answers null and no screen has an owner column; the field exists for the "
        "rule-document model, not for the interface",
        "version": "always 1 until rule versioning lands; the interface displays it from the document's own "
        "field rather than the wire one",
    },
}

TESTS = re.compile(r"__tests__|/test/")

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


def find_declaration(text: str, interface: str) -> int:
    """Where `interface` is declared, exported or module-local."""
    for prefix in ("export interface ", "interface "):
        start = text.find(f"{prefix}{interface} ")
        if start != -1:
            return start
    return -1


def interface_fields(path: pathlib.Path, interface: str) -> set[str]:
    text = path.read_text(encoding="utf-8")
    start = find_declaration(text, interface)
    if start == -1:
        raise SystemExit(f"check-frontend-types: {interface} not found in {path}")
    # Interfaces here are flat; stop at the closing brace of this one.
    end = text.index("\n}", start)
    return set(INTERFACE_FIELD.findall(text[start:end]))


BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
LINE_COMMENT = re.compile(r"^\s*//.*$", re.M)


def frontend_sources() -> dict[pathlib.Path, str]:
    """Every frontend source except tests, with comments removed.

    Two exclusions, both learned by watching the check pass when it should have failed: a *test* that names a
    field is not a consumer of it, and neither is a **comment**. The audit viewer documented its mapping in
    prose (`ip_address → ipAddress`) and that mention alone kept the check green after the code that did the
    mapping was deleted.
    """
    sources = {}
    for path in (REPO / "frontend" / "src").rglob("*"):
        if path.suffix not in {".ts", ".tsx"} or not path.is_file():
            continue
        if TESTS.search(str(path)):
            continue
        text = path.read_text(encoding="utf-8")
        text = BLOCK_COMMENT.sub("", text)
        text = LINE_COMMENT.sub("", text)
        sources[path] = text
    return sources


def strip_interface(text: str, interface: str) -> str:
    """`text` without the declaration of `interface` — declaring a field is not reading it."""
    start = find_declaration(text, interface)
    if start == -1:
        return text
    end = text.index("\n}", start) + 2
    return text[:start] + text[end:]


def unread_keys(
    sources: dict[pathlib.Path, str],
    declaring: pathlib.Path,
    interface: str,
    keys: set[str],
    consumer: str | None,
) -> list[str]:
    """Keys the API sends that nothing consumes.

    With a `consumer`, a key counts as consumed only when it appears **inside that function** — the place
    that turns the response into what a screen shows. Without one, the whole frontend is searched, minus the
    interface declaration and minus comments.

    Counting occurrences anywhere was too weak, and measurably so: deleting the mapping that read
    `event.ip_address` left the name in a prose comment and in the interface, and the check stayed green; the
    same happened to `dsl_body`, which the create form legitimately sends as a request field. Neither is a
    consumer of a *response*.
    """
    if consumer:
        consumer_path, _, consumer_fn = consumer.partition("::")
        text = sources.get(REPO / consumer_path)
        if text is None:
            raise SystemExit(f"check-frontend-types: {consumer_path} is not a frontend source")
        joined = function_body(text, consumer_fn)
    else:
        corpus = []
        for path, text in sources.items():
            corpus.append(strip_interface(text, interface) if path == declaring else text)
        joined = "\n".join(corpus)
    return sorted(key for key in keys if not re.search(rf"\b{re.escape(key)}\b", joined))


def function_body(text: str, name: str) -> str:
    """The source of `function name(…) { … }`, or of `const name = (…) => { … }`.

    Raises when the consumer cannot be found: a pair that names a consumer which does not exist would
    otherwise compare against an empty string and report every field as unread, or — worse — be "fixed" by
    pointing it at something that happens to contain the names.
    """
    match = re.search(
        rf"(?:function\s+{name}\s*\(|const\s+{name}\s*=\s*\()", text
    )
    if not match:
        raise SystemExit(f"check-frontend-types: consumer `{name}` not found")
    start = text.index("{", match.end())
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    raise SystemExit(f"check-frontend-types: consumer `{name}` has no closing brace")


def main() -> int:
    problems = []
    sources = frontend_sources()
    for rust_path, handler, ts_path, interface, consumer in PAIRS:
        rust_file = REPO / rust_path
        sent = (
            handler_keys(rust_file, handler)
            if any(m.group(1) == handler for m in FN_START.finditer(rust_file.read_text(encoding="utf-8")))
            else struct_keys(rust_file, handler)
        )
        declared = interface_fields(REPO / ts_path, interface)

        missing = sorted(sent - declared)  # the server sends it, the interface does not declare it
        invented = sorted(declared - sent)  # the interface declares it, the server never sends it

        # Declaring is not reading. A field the API sends, the interface declares and nothing consumes is the
        # defect that let `dsl_body` be dropped: the edit page's textarea started empty and saving wrote the
        # empty string back over the stored body.
        allowed = UNREAD_ALLOWED.get(interface, {})
        unread = [
            key
            for key in unread_keys(sources, REPO / ts_path, interface, sent, consumer)
            if key not in allowed
        ]

        status = "ok" if not missing and not invented and not unread else "FAIL"
        print(f"  {status:4} {handler:18} → {interface:24} {len(sent)} keys")
        if missing:
            problems.append(f"{interface}: the API sends {missing}, which the interface does not declare")
        if invented:
            problems.append(
                f"{interface}: declares {invented}, which `{handler}` never sends — a caller reading "
                f"them gets `undefined`"
            )
        if unread:
            problems.append(
                f"{interface}: declares {unread}, which the API sends and no frontend source reads — a "
                f"declared field nobody consumes is how a page silently shows nothing, or overwrites what it "
                f"loaded"
            )

    if problems:
        print()
        for problem in problems:
            print(f"  {problem}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
