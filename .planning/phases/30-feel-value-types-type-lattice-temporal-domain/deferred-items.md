# Phase 30 — Deferred Items (out-of-scope discoveries)

Items discovered during plan execution that are out of scope of the current
plan's `files_modified` and were therefore NOT modified. Logged per the GSD
executor deviation-rule scope boundary ("pre-existing warnings, linting
errors, or failures in unrelated files are out of scope — log to
deferred-items.md").

## 1. `engine/src/engine/leaps/mod.rs:66` — Value wildcard `_ => None`

**Found during:** Plan 30-04 Task 2 (regression audit enumeration step).
**Site:** `compute_from_facts` — `match val { Value::Integer(i) => Some(*i as f64), Value::Float(f) => Some(*f), _ => None }`.
**Classification:** Rete-only (LEAPS algorithm module, feature-gated behind `engine-leaps`). The FEEL path never routes through LEAPS. The wildcard catches the 8 FEEL variants + legacy DateTime/String/Bool/Object/Array/Null and returns `None` (not a numeric fact) — correct for the Rete LEAPS path.
**Why deferred:** Plan 30-04's `files_modified` scope is `validation.rs` and `rules.rs` only. The leaps module is feature-gated and out of scope. Adding a "Rete-only" comment here would be consistent with the audit but is a Phase-31-or-later cleanup.
**Risk:** None — LEAPS is Rete-only; FEEL values never reach this site. Equivalent to the documented wildcards in validation.rs/rules.rs.
**Recommended action:** A future consistency pass can add the "Rete-only — FEEL variants handled in engine/src/dmn/feel/" citation comment here, matching the 30-04 audit pattern.

## 2. Pre-existing clippy errors (26, with `-D warnings`)

**Found during:** Plan 30-04 Task 1/2 clippy verification.
**Count:** 26 errors under `cargo clippy -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak -- -D warnings` at the baseline (before any 30-04 changes). After 30-04: still 26, with 0 new errors and 0 errors in `engine/src/dmn/feel/types.rs`.
**Why deferred:** Pre-existing, in unrelated files (chaining/mode_selector, engine modules, monitoring, etc.). The 30-04 Task 2 acceptance criterion is "no NEW warnings from the audit" — satisfied. A full clippy cleanup is a separate tech-debt item.
**Recommended action:** Track as tech-debt; a dedicated `chore` phase should resolve the 26 baseline clippy errors.

## 3. Pre-existing flaky test `monitoring::coverage::tests::test_coverage_tracking_singleton`

**Found during:** Plan 30-04 full-suite regression runs.
**Symptom:** Fails intermittently (1-2 of 269 tests) due to a missing `#[serial]` guard on the coverage singleton — sibling singleton tests race on `clear()` + record without serialization.
**Why deferred:** Pre-existing latent race in `engine/src/monitoring/coverage.rs`, unrelated to the FEEL lattice work. Documented in Plan 01 and Plan 03 summaries. The rest of the suite (267 lib tests excluding the 3 coverage-singleton variants) is green.
**Recommended action:** Add `#[serial]` (serial_test dev-dep) to the coverage singleton test family in a future fix phase.