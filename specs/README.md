# rRules — specification

This directory is the system's specification, extracted from the implementation
and kept as the single place a reader can learn what rRules is supposed to do.
It is written to be consumed two ways: by people (who read the prose) and by
[CodeWitness](../../../CodeWitness), which turns each numbered requirement into an
executed experiment and reports what it observed.

## Contents

| File | What it specifies |
|------|-------------------|
| `requirements.md` | **Normative requirements** with stable ids (`ENG-001`, `FEEL-001`, …). This is the file CodeWitness parses. |
| `domain-model.md` | Values, facts, working memory, rules, the FEEL type lattice, the temporal domain, validation. |
| `rule-engine.md` | Execution semantics, the four engines, fact lifecycle, salience, chaining, observability. |
| `dmn-feel.md` | DMN 1.5 canonical IR, parser/exporter, FEEL grammar and semantics, the built-in library, decision tables, DRD evaluation, TCK conformance. |
| `http-api.md` | Server composition, authentication, every registered endpoint, errors, tenancy, persistence, realtime surfaces. |

Each subsystem document records what the code does — with `path:line`
references — and, in a closing `Known gaps` section, what it does not do. The
gaps are the honest part: they are the difference between the specification and
the implementation, and several of them are the reason a requirement is reported
as `UNVERIFIED` rather than `VERIFIED`.

## Verification with CodeWitness

CodeWitness executes requirements; it does not read them for agreement. Two
projects are configured, because the code base is two crates with different
execution surfaces:

```bash
# Engine semantics, FEEL and DMN: generated scenario tests + existing suites.
codewitness verify engine --requirements specs/requirements.md --in-place

# HTTP API: the actix-web integration suites, mapped by requirement id.
codewitness verify api --requirements specs/requirements.md \
    --config api/codewitness.yaml --in-place
```

`--in-place` matters: both crates are workspace members, and CodeWitness's
default copy-the-project mode puts the copy inside the parent workspace, where
Cargo refuses to build it ("current package believes it's in a workspace when
it's not"). Running in place builds the real workspace; generated tests are
written into the crate's `tests/` directory for the duration of the run.

`engine/codewitness.yaml` declares one scenario per executable requirement: the
setup, the calls, the observation and the invariant, written in Rust. CodeWitness
compiles them as ordinary Cargo integration tests and reports
`VERIFIED` / `PARTIALLY_VERIFIED` / `FAILED` / `UNVERIFIED` / `BLOCKED` with the
evidence attached. Requirements with no scenario and no mapped test stay
`UNVERIFIED`, which is the honest answer and never a pass.

Reports are written under `<project>/.codewitness/runs/<run-id>/` as JSON and
HTML. `.codewitness/` is ignored by git: a report is an artefact of a run, not a
source file.

## Traceability

Requirement ids are stable and are cited in the code and tests that discharge
them:

- `engine/codewitness.yaml` binds a scenario to a requirement via
  `requirement: FEEL-001`.
- Test suites carry a `// Requirements covered by this suite: API-003 API-004`
  marker in their header, which is how CodeWitness maps a test class to a
  requirement.
- Subsystem documents cite the requirement ids they explain, so a reader can
  move from the prose to the claim and from the claim to the evidence.

## Current state (measured, not asserted)

`make verify` runs CodeWitness over both crates and prints the union:

| Verdict | Count | Requirements |
|---------|-------|--------------|
| VERIFIED | 35 | ENG-001…007, DMN-001…006, FEEL-001…012, API-001…007, SYS-001…003 |
| FAILED | 0 | — |
| UNVERIFIED | 0 | — |

Neither crate can execute the other's requirements: `engine` has no HTTP surface and
`api` has no evaluator. Each half therefore reports the other's requirements as
UNVERIFIED, and `scripts/verify-summary.py` unions the two reports by id. A requirement
that *neither* half executes is a gap in the verification, not a division of labour, and
it fails the gate.

The `api` half needs no live server. `api/src/app.rs` exposes `build_app`, so a scenario
constructs the same `App` the server serves and drives it with `actix_web::test` — the
auth middleware, the routing and the handlers all run, in-process, against stores rooted
in a temporary directory (`ApiState::for_testing`). Seven scenarios cover the seven API
requirements: an unauthenticated request is refused and a token is accepted, invalid
execution input is a `400`, rules are isolated per tenant, the tenant CRUD contract holds
including the second delete, decisions list and an unknown decision is a client error, a
reload returns a job in a defined state, and both metric surfaces answer.

This is what the earlier state of this document called impossible. It was not: the
obstacle was that the crate had no library target and the application was assembled
inside `main`, so nothing could build it but the server itself.

The DMN conformance numbers are maintained separately in
[`../TCK-COVERAGE-GAPS.md`](../TCK-COVERAGE-GAPS.md), which records the measured
pass/fail/skip counts per layer and every remaining divergence with its spec
reference. That file, not this one, is the authority on "how conformant is the
DMN implementation today".
