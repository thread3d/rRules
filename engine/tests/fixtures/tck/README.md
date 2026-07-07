# OMG DMN TCK — Vendored Subset

This directory holds a **sparse subset** of the OMG DMN Technology
Compatibility Kit (DMN TCK), vendored into the rRules engine test suite as
conformance vectors for the FEEL value domain (Phase 30, DMN-08/DMN-09).

## Provenance

| Field | Value |
|-------|-------|
| Source repository | https://github.com/dmn-tck/tck |
| Cloned commit hash | `0dbcaf9b98bc3af4e36d44a7aed95e9e85703a13` |
| Clone date | 2026-07-07 |
| Clone method | `git clone --depth 1 --filter=blob:none --sparse` + `git sparse-checkout` (only the directories below) |
| License | Apache License 2.0 (`LICENSE-ASL-2.0.txt` in the source repo root) |
| Distribution terms | The DMN TCK is a community-led conformance kit; per its README, joining and using the TCK is free, and the repo is published under Apache-2.0. Redistribution of the test-case XML/DMN files under Apache-2.0 is permitted so long as the license terms are retained. |

> **OMG TCK license/distribution terms (summary):** The DMN TCK is a
> community-led proposal for a verifiable, executable method to demonstrate
> DMN conformance. The repository is licensed under the Apache License 2.0.
> Attribution is given here; the upstream `LICENSE-ASL-2.0.txt` applies to the
> vendored files. No OMG-proprietary normative content is redistributed — only
> the community-authored TCK test vectors.

## Included directories

Only the directories Phase 30 needs are vendored (the full TCK is intentionally
NOT copied). The short names on disk map to the upstream paths as follows:

| Local path | Upstream path (`TestCases/…`) | Purpose |
|------------|-------------------------------|---------|
| `0100-arithmetic/` | `compliance-level-3/0100-arithmetic` | FEEL arithmetic + temporal/duration round-trip vectors (incl. `@Australia/Melbourne`) |
| `1100-feel-decimal-function/` | `compliance-level-3/1100-feel-decimal-function` | `decimal()` built-in numeric vectors |
| `1141/` | `compliance-level-3/1141-feel-round-up-function` | `round up` scale-boundary vectors |
| `1142/` | `compliance-level-3/1142-feel-round-down-function` | `round down` scale-boundary vectors |
| `1143/` | `compliance-level-3/1143-feel-round-half-up-function` | `round half up` scale-boundary vectors |
| `1144/` | `compliance-level-3/1144-feel-round-half-down-function` | `round half down` scale-boundary vectors |
| `0002-input-data-number/` | `compliance-level-2/0002-input-data-number` | Input-data `number` typing vectors |

Each directory contains the upstream `.dmn` model file and its `-test-01.xml`
test-case file (plus, for `0100-arithmetic` and `1100`, an upstream README and a
`generateTests.js` helper — retained verbatim).

## Usage

- `engine/tests/tck_precision_audit.rs` (Plan 30-01 Task 2) walks this tree and
  asserts no value-domain numeric literal exceeds 29 significant digits
  (research assumption A1 — `rust_decimal` sufficiency).
- Plans 30-02 and 30-03 run the numeric / temporal vectors against
  `Value::Number` / the temporal parse-format helpers.

## Updating

To refresh this subset from a newer TCK HEAD:

```bash
git clone --depth 1 --filter=blob:none --sparse https://github.com/dmn-tck/tck.git
cd tck
git sparse-checkout add \
  TestCases/compliance-level-3/0100-arithmetic \
  TestCases/compliance-level-3/1100-feel-decimal-function \
  TestCases/compliance-level-3/1141-feel-round-up-function \
  TestCases/compliance-level-3/1142-feel-round-down-function \
  TestCases/compliance-level-3/1143-feel-round-half-up-function \
  TestCases/compliance-level-3/1144-feel-round-half-down-function \
  TestCases/compliance-level-2/0002-input-data-number
# then copy each into engine/tests/fixtures/tck/<short-name>/ and update the
# commit hash in this README.
```

Vendored 2026-07-07 for Phase 30 (FEEL value types + type lattice + temporal
domain). See `.planning/phases/30-feel-value-types-type-lattice-temporal-domain/30-RESEARCH.md`
for the inspection that selected this subset.