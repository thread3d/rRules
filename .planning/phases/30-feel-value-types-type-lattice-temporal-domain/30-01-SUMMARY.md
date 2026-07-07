---
phase: 30-feel-value-types-type-lattice-temporal-domain
plan: 01
subsystem: engine
tags: [feel, dmn, rust_decimal, chrono, chrono-tz, serde, type-lattice, tck]

# Dependency graph
requires:
  - phase: 29-dmn-1-5-model-xml-parser-canonical-ir
    provides: engine/src/dmn module tree (model/parser/exporter) that feel/ sits beside; rust_decimal 1.42 already in Cargo.toml
provides:
  - engine/src/dmn/feel/ module tree (types, temporal, numeric) with skeleton type declarations
  - FeelType enum (13 lattice nodes), FeelRange + FeelFunction carrier structs
  - TimeValue / DateTimeValue (Fixed/Iana/Local) / YearsMonthsDuration temporal carriers
  - Value enum extended additively with 8 FEEL variants (Number, Date, Time, DateTimeTz, DaysTimeDuration, YearsMonthsDuration, Range, Function)
  - chrono-tz 0.10.4 dep with serde feature (IANA timezone support)
  - Vendored OMG TCK subset under engine/tests/fixtures/tck/ (Apache-2.0, commit 0dbcaf9b)
  - Wave-0 tck_precision_audit gate confirming research assumption A1 (rust_decimal sufficiency)
affects: [30-02-numeric, 30-03-temporal, 30-04-lattice-regression, 31-feel-parser-evaluator, 35-conformance]

# Tech tracking
tech-stack:
  added: [chrono-tz 0.10.4 (serde feature)]
  patterns: [additive Value extension without #[non_exhaustive], manual serde for types holding DateTime<chrono_tz::Tz>, boundary-aware numeric tokenizer for TCK auditing, TDD RED/GREEN on skeleton type declarations]

key-files:
  created:
    - engine/src/dmn/feel/mod.rs
    - engine/src/dmn/feel/types.rs
    - engine/src/dmn/feel/temporal.rs
    - engine/src/dmn/feel/numeric.rs
    - engine/tests/tck_precision_audit.rs
    - engine/tests/fixtures/tck/README.md
    - engine/tests/fixtures/tck/LICENSE-ASL-2.0.txt
    - engine/tests/fixtures/tck/0100-arithmetic/
    - engine/tests/fixtures/tck/1100-feel-decimal-function/
    - engine/tests/fixtures/tck/1141/
    - engine/tests/fixtures/tck/1142/
    - engine/tests/fixtures/tck/1143/
    - engine/tests/fixtures/tck/1144/
    - engine/tests/fixtures/tck/0002-input-data-number/
  modified:
    - engine/Cargo.toml (chrono-tz dep + provenance comment)
    - engine/src/dmn/mod.rs (pub mod feel; + doc)
    - engine/src/data_types.rs (8 new Value variants + is_compatible_with FEEL arms)
    - engine/src/testing/mod.rs (from_value FEEL arms)
    - api/src/v1/execution.rs (engine_value_to_json FEEL arms)

key-decisions:
  - "D-30-01a: chrono-tz added with the `serde` feature (plan action text omitted it, but DateTimeValue must derive Serialize/Deserialize and its Iana arm holds DateTime<chrono_tz::Tz>, which requires Tz: Serialize+Deserialize)"
  - "D-30-01b: Manual Serialize/Deserialize for DateTimeValue via a (NaiveDateTime, chrono_tz::Tz) repr — chrono only impls Deserialize for DateTime<Utc/FixedOffset/Local>, NOT generic DateTime<Tz>; the IANA zone round-trips as its name string (Pitfall 3 mitigation preserved)"
  - "D-30-01c: FeelType derives Serialize+Deserialize (not just Debug/Clone/PartialEq as plan action said) because FeelFunction derives serde and holds Vec<FeelType>/Box<FeelType> — the serde bound propagates"
  - "D-30-01d: DynamicCast::is_compatible_with and api engine_value_to_json got explicit FEEL-variant arms (debug-string / false) instead of a wildcard — preserves the exhaustive-match safety net (30-RESEARCH.md Pitfall 1) so a future new variant still triggers a compiler error"
  - "D-30-01e: TCK precision-audit tokenizer is boundary-aware (requires non-alphanumeric chars on both sides of a numeric token) so hex-hash fragments in testCase IDs (e.g. 177e63 inside 001_af177e63fc) are NOT mistaken for scientific-notation numbers"
  - "D-30-01f: A1 CONFIRMED — no value-domain literal in the vendored TCK subset exceeds 29 significant digits; rust_decimal locked for Value::Number (backing type unchanged); bigdecimal 0.4.10 fallback documented in the audit's stderr for Phase 31 if a high-scale string() vector ever demands it"

patterns-established:
  - "Additive Value extension: new FEEL variants appended after the 8 legacy variants; legacy DateTime(NaiveDateTime) NOT repurposed; no #[non_exhaustive] (exhaustive match is the safety net)"
  - "Manual serde for chrono_tz::Tz-bearing types: serialize as (NaiveDateTime, Tz), reconstruct via from_local_datetime (earliest on Ambiguous, error on None)"
  - "FEEL values never route through Rete-path DynamicCast / engine_value_to_json — they map to debug-string / false at the exhaustive-match safety sites"

requirements-completed: [DMN-08, DMN-10]

# Coverage metadata
coverage:
  - id: D1
    description: "engine/src/dmn/feel/ module skeleton with FeelType, FeelRange, FeelFunction, TimeValue, DateTimeValue, YearsMonthsDuration type declarations"
    requirement: "DMN-10"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/types.rs#feeltype_has_all_lattice_nodes + feel_range_carrier_round_trips + feel_function_carrier_constructs"
        status: pass
      - kind: unit
        ref: "engine/src/dmn/feel/temporal.rs#date_time_value_iana_serde_round_trips + date_time_value_fixed_serde_round_trips + date_time_value_local_serde_round_trips"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel:: (11 passed)"
        status: pass
    human_judgment: false
  - id: D2
    description: "Value enum extended additively with 8 FEEL variants (Number/Date/Time/DateTimeTz/DaysTimeDuration/YearsMonthsDuration/Range/Function); legacy DateTime untouched; no #[non_exhaustive]; all exhaustive-match consumer sites updated"
    requirement: "DMN-10"
    verification:
      - kind: unit
        ref: "engine/src/data_types.rs#tests (13 existing Value tests green after extension)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib data_types:: (13 passed)"
        status: pass
      - kind: automated
        ref: "grep -c '#[non_exhaustive]' engine/src/data_types.rs == 0"
        status: pass
    human_judgment: false
  - id: D3
    description: "chrono-tz 0.10.4 dependency added (with serde feature) for IANA timezone support in DateTimeValue"
    requirement: "DMN-10"
    verification:
      - kind: automated
        ref: "grep -c 'chrono-tz' engine/Cargo.toml >= 1; cargo build -p engine exits 0"
        status: pass
    human_judgment: false
  - id: D4
    description: "Vendored OMG TCK subset (0100-arithmetic, 1100-feel-decimal-function, 1141-1144 round functions, 0002-input-data-number) with provenance README + Apache-2.0 license"
    requirement: "DMN-08"
    verification:
      - kind: automated
        ref: "test -f engine/tests/fixtures/tck/README.md && grep dmn-tck/tck + commit hash + Apache license terms present"
        status: pass
      - kind: manual_procedural
        ref: "README records source repo, commit 0dbcaf9b, Apache-2.0 license/provenance (30-VALIDATION Manual-Only Verifications row)"
        status: pass
    human_judgment: false
  - id: D5
    description: "Wave-0 tck_precision_audit gate confirming research assumption A1 (no vendored TCK value-domain literal exceeds 29 significant digits; rust_decimal sufficient; bigdecimal fallback documented)"
    requirement: "DMN-08"
    verification:
      - kind: integration
        ref: "engine/tests/tck_precision_audit.rs#tck_precision_audit (A1 confirmed, 0 offenders)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --test tck_precision_audit (5 passed)"
        status: pass
    human_judgment: false
  - id: D6
    description: "Full engine test suite green after the additive Value extension (first full regression check, Success Criterion 4)"
    requirement: "DMN-10"
    verification:
      - kind: automated
        ref: "cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak (242 lib + integration tests, 0 failed, 3 consecutive runs)"
        status: pass
    human_judgment: false

# Metrics
duration: 12min
completed: 2026-07-07
status: complete
---

# Phase 30 Plan 01: FEEL Value Type Foundation Summary

**Additive Value enum extension (8 FEEL variants), engine/src/dmn/feel/ module skeleton (FeelType lattice + temporal carriers), chrono-tz dep, and vendored OMG TCK subset with a green Wave-0 precision-audit gate confirming rust_decimal sufficiency (A1)**

## Performance

- **Duration:** 12 min
- **Started:** 2026-07-07T04:12:43Z
- **Completed:** 2026-07-07T04:24:52Z
- **Tasks:** 2
- **Files modified:** 16 (6 source + 10 TCK vendor files incl. README + LICENSE)

## Accomplishments
- Created the `engine/src/dmn/feel/{mod,types,temporal,numeric}.rs` module tree with placeholder type declarations sufficient for downstream plans (02 numeric, 03 temporal, 04 lattice+regression) to build on.
- Extended `engine::data_types::Value` additively with all 8 FEEL variants (`Number(rust_decimal::Decimal)`, `Date`, `Time`, `DateTimeTz`, `DaysTimeDuration`, `YearsMonthsDuration`, `Range`, `Function`); legacy `DateTime(NaiveDateTime)` untouched; no `#[non_exhaustive]` — exhaustive match remains the safety net.
- Vendored the OMG DMN TCK subset (7 directories, Apache-2.0, upstream commit `0dbcaf9b`) with a provenance README + retained license file.
- Landed the Wave-0 `tck_precision_audit` gate: **A1 CONFIRMED** — no value-domain literal in the vendored subset exceeds 29 significant digits; `rust_decimal` locked for `Value::Number`; `bigdecimal` fallback documented in the test's stderr for Phase 31 if a high-scale `string()` vector ever demands it.
- Full `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` green (242 lib + integration tests, 0 failed) across 3 consecutive runs — zero regression from the additive Value extension.

## Task Commits

Each task followed the TDD RED→GREEN cycle (plan `type: tdd` per-task):

1. **Task 1 (30-01-01): FEEL module skeleton + Value extension** —
   - `f635a4a5d` (test) RED: failing feel skeleton tests + chrono-tz dep + module wiring
   - `59fbf2dbd` (feat) GREEN: implement FEEL value type foundation + Value extension
2. **Task 2 (30-01-02): Vendor TCK subset + precision-audit gate** —
   - `7d820980d` (test) RED: failing TCK precision-audit gate (Wave-0 DMN-08)
   - `83d1be74a` (feat) GREEN: vendor OMG TCK subset + green Wave-0 precision-audit gate

## Files Created/Modified
- `engine/src/dmn/feel/mod.rs` — module wiring (`pub mod numeric; pub mod temporal; pub mod types;`) + doc
- `engine/src/dmn/feel/types.rs` — `FeelType` enum (13 lattice nodes), `FeelRange`, `FeelFunction` carriers + skeleton tests
- `engine/src/dmn/feel/temporal.rs` — `TimeValue`, `DateTimeValue` (Fixed/Iana/Local) with manual serde, `YearsMonthsDuration` + round-trip tests
- `engine/src/dmn/feel/numeric.rs` — stub module (Plan 02 fills in the Decimal128 helpers)
- `engine/src/data_types.rs` — 8 new `Value` variants + `is_compatible_with` FEEL arms
- `engine/src/testing/mod.rs` — `ValueSnapshot::from_value` FEEL-variant arms (debug-string fallback)
- `api/src/v1/execution.rs` — `engine_value_to_json` FEEL-variant arms (debug-string fallback)
- `engine/Cargo.toml` — `chrono-tz = { version = "0.10.4", features = ["serde"] }` + provenance comment
- `Cargo.lock` — chrono-tz + phf transitive deps
- `engine/src/dmn/mod.rs` — `pub mod feel;` + doc
- `engine/tests/tck_precision_audit.rs` — Wave-0 gate scanning vendored TCK for >29-sig-digit literals
- `engine/tests/fixtures/tck/README.md` — provenance (source repo, commit hash, Apache-2.0, included dirs)
- `engine/tests/fixtures/tck/LICENSE-ASL-2.0.txt` — upstream license retained for attribution
- `engine/tests/fixtures/tck/{0100-arithmetic,1100-feel-decimal-function,1141,1142,1143,1144,0002-input-data-number}/` — vendored TCK subset (.dmn + test-case XML per dir)

## Decisions Made
- D-30-01a: Added chrono-tz with the `serde` feature (plan action omitted it, but `DateTimeValue`'s `Iana` arm holds `DateTime<chrono_tz::Tz>` which needs `Tz: Serialize+Deserialize` for the derive).
- D-30-01b: Manual `Serialize`/`Deserialize` for `DateTimeValue` via a `(NaiveDateTime, chrono_tz::Tz)` repr — chrono only impls `Deserialize` for `DateTime<Utc/FixedOffset/Local>`, not generic `DateTime<Tz>`. The IANA zone round-trips as its name string (`"Australia/Melbourne"`), preserving Pitfall 3's mitigation. DST ambiguous → earliest; non-existent local time → serde error.
- D-30-01c: `FeelType` derives `Serialize+Deserialize` (plan said only `Debug, Clone, PartialEq`) because `FeelFunction` derives serde and holds `Vec<FeelType>`/`Box<FeelType>` — the bound propagates.
- D-30-01d: `DynamicCast::is_compatible_with` and `api::engine_value_to_json` got explicit FEEL-variant arms (not a wildcard) to preserve the exhaustive-match safety net (Pitfall 1) — a future new `Value` variant still triggers a compiler error at these sites.
- D-30-01e: The precision-audit tokenizer is boundary-aware (requires non-alphanumeric chars on both sides of a numeric token) so hex-hash fragments in TCK `testCase` IDs (e.g. `177e63` inside `001_af177e63fc`) are not mistaken for scientific-notation numbers. This eliminated 12 false positives.
- D-30-01f: A1 confirmed — `rust_decimal` locked for `Value::Number` (backing type unchanged); `bigdecimal 0.4.10` deferred to Phase 31, localized to the rounding built-ins if a high-scale `string()` vector demands it.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] chrono-tz added with `serde` feature**
- **Found during:** Task 1 (feel module skeleton)
- **Issue:** Plan action said `cargo add chrono-tz --package engine` (no features), but `DateTimeValue` must `#[derive(Serialize, Deserialize)]` and its `Iana` arm holds `DateTime<chrono_tz::Tz>`; without the `serde` feature on chrono-tz, `Tz` does not impl serde and the derive fails.
- **Fix:** `cargo add chrono-tz --package engine --features serde`; recorded the feature in the Cargo.toml provenance comment.
- **Files modified:** engine/Cargo.toml
- **Verification:** `cargo test -p engine --lib dmn::feel::` green (DateTimeValue serde round-trip tests pass).
- **Committed in:** f635a4a5d (RED) / 59fbf2dbd (GREEN)

**2. [Rule 3 - Blocking] Manual serde for DateTimeValue**
- **Found during:** Task 1 (GREEN)
- **Issue:** `#[derive(Serialize, Deserialize)]` on `DateTimeValue` fails: chrono only impls `Deserialize` for `DateTime<Utc>`, `DateTime<FixedOffset>`, `DateTime<Local>` — NOT for generic `DateTime<Tz>`. So `DateTime<chrono_tz::Tz>` cannot be derived.
- **Fix:** Implemented `Serialize`/`Deserialize` manually via a `DateTimeValueRepr` intermediate that holds the `Iana` arm as `(NaiveDateTime, chrono_tz::Tz)` (both serde-able); reconstruct via `tz.from_local_datetime(&naive)` (earliest on Ambiguous, error on None).
- **Files modified:** engine/src/dmn/feel/temporal.rs
- **Verification:** 3 serde round-trip tests (Iana/Fixed/Local) pass.
- **Committed in:** 59fbf2dbd

**3. [Rule 3 - Blocking] FeelType derives Serialize+Deserialize**
- **Found during:** Task 1 (GREEN)
- **Issue:** Plan said `FeelType` derives only `Debug, Clone, PartialEq`, but `FeelFunction` derives `Serialize, Deserialize` and holds `Vec<FeelType>` / `Box<FeelType>` — the serde bound propagates to the inner type, so the derive on `FeelFunction` fails without `FeelType: Serialize + Deserialize`.
- **Fix:** Added `Serialize, Deserialize` to `FeelType`'s derive line.
- **Files modified:** engine/src/dmn/feel/types.rs
- **Verification:** `cargo test -p engine --lib dmn::feel::` green.
- **Committed in:** 59fbf2dbd

**4. [Rule 3 - Blocking] is_compatible_with FEEL-variant arms**
- **Found during:** Task 1 (GREEN)
- **Issue:** `DynamicCast::is_compatible_with` in `data_types.rs` is an exhaustive match over the original 8 `Value` variants with NO wildcard; adding 8 new variants broke compilation (compiler-forced).
- **Fix:** Added explicit arms for the 8 FEEL variants returning `false` (FEEL values are not Rete-compatible; they never route through this path per Pitfall 1). No wildcard — preserves the exhaustive-match safety net.
- **Files modified:** engine/src/data_types.rs
- **Verification:** `cargo build` + `cargo test -p engine --lib data_types::` green.
- **Committed in:** 59fbf2dbd

**5. [Rule 3 - Blocking] api engine_value_to_json FEEL-variant arms**
- **Found during:** Task 1 (GREEN, full-workspace build)
- **Issue:** `api/src/v1/execution.rs::engine_value_to_json` is an exhaustive match over `engine::Value` (no wildcard); the Value extension broke the api crate compilation (compiler-forced cross-crate exhaustive site, not listed in the plan's files_modified).
- **Fix:** Added explicit debug-string fallback arms for the 8 FEEL variants (consistent with `testing/mod.rs::from_value`). FEEL values never legitimately reach this Rete-path reverse mapping; the debug string makes any stray FEEL value visibly traceable.
- **Files modified:** api/src/v1/execution.rs
- **Verification:** `cargo build` (full workspace) green.
- **Committed in:** 59fbf2dbd

**6. [Rule 1 - Bug] TCK precision-audit tokenizer boundary rule**
- **Found during:** Task 2 (GREEN)
- **Issue:** The initial numeric-literal tokenizer extracted `177e63` and `46e59` as 66/61-sig-digit scientific-notation literals, refuting A1 — but these are hex-hash fragments inside alphanumeric TCK `testCase` IDs (`001_af177e63fc`, `010_4b46e59c8b`), NOT value-domain numbers. The `e`/`E` exponent heuristic matched the hex `e` inside the ID.
- **Fix:** Made the tokenizer boundary-aware: a numeric token is only extracted when bounded on both sides by non-alphanumeric characters (or string boundaries). Digit runs embedded in alphanumeric identifiers are now excluded.
- **Files modified:** engine/tests/tck_precision_audit.rs
- **Verification:** Added `iter_literals_excludes_id_fragments` unit test; the full audit now reports 0 offenders (A1 confirmed).
- **Committed in:** 83d1be74a

---

**Total deviations:** 6 auto-fixed (5 Rule 3 blocking, 1 Rule 1 bug)
**Impact on plan:** All auto-fixes were necessary for the foundation to compile and for the precision audit to produce a correct A1 verdict. No scope creep — the type shapes and Value variant list match the plan exactly; the fixes were implementation details (serde feature, manual serde impl, compiler-forced exhaustive arms, tokenizer precision).

## Issues Encountered
- `cargo test -p engine` (no features) fails to compile feature-gated integration tests (`rete_execution.rs`, `coverage.rs`, `engine_execution_tests.rs`) that import `engine::engine::rete` etc. behind `#[cfg(feature = "engine-rete")]`. **This is pre-existing** (confirmed at baseline commit `5e800555b`) — the engine algorithm modules are feature-gated and those test files are not themselves feature-gated. The full regression check therefore runs with `--features engine-rete,engine-treat,engine-leaps,engine-phreak`, which is green (242 lib + integration tests, 0 failed, 3 consecutive runs). The `tck_precision_audit` integration test does NOT need the engine features and passes standalone.
- The `monitoring::coverage::tests::test_coverage_tracking_singleton` test lacks a `#[serial]` guard and races with sibling coverage singleton tests (pre-existing latent race — `clear()` + record without serialization). It was observed to fail once in a full-suite run but passes in isolation and across 3 consecutive full-suite re-runs. Root cause is unrelated to the additive Value extension; out of scope per the deviation rules (pre-existing, in an unrelated file). Logged here for visibility.

## User Setup Required
None — no external service configuration required. The only new dependency (`chrono-tz`) is a pure-Rust crate that embeds the IANA tzdata at compile time; no system `tzdata` install needed.

## Next Phase Readiness
- The `engine/src/dmn/feel/` module tree exists with typed carriers; Plan 02 can implement `feel_numeric_eq` + `0.1 + 0.2 = 0.3` in `numeric.rs` against `Value::Number(rust_decimal::Decimal)`.
- Plan 03 can implement `parse_at_literal` / `format_value` / `add_months_zoned` in `temporal.rs` against `TimeValue` / `DateTimeValue` / `YearsMonthsDuration` (the IANA-zone-preserving serde round-trip is already proven).
- Plan 04 can implement `conforms` / `equivalent` / `implicit_convert` / `resolve_type_ref` in `types.rs` against the `FeelType` enum, and run the regression audit over the Rete-path `Value` wildcards (the FEEL variants currently fall through `_ => None`/`_ => {}` wildcards in `validation.rs`/`rules.rs` by design — Plan 04 Task 2 decides whether to tighten them).
- A1 is resolved: `rust_decimal` is locked for the value domain; `bigdecimal` is a documented Phase-31-localized fallback only.

## TDD Gate Compliance
Both tasks followed the RED→GREEN cycle (plan per-task `tdd="true"`). Git log confirms:
1. `test(30-01): ...` commits (RED) — `f635a4a5d`, `7d820980d`
2. `feat(30-01): ...` commits (GREEN) — `59fbf2dbd`, `83d1be74a`

RED gates failed as expected (undeclared types / missing fixtures); GREEN gates pass. No refactor commits needed.

## Self-Check: PASSED

**Files created (verified present):**
- FOUND: engine/src/dmn/feel/mod.rs
- FOUND: engine/src/dmn/feel/types.rs
- FOUND: engine/src/dmn/feel/temporal.rs
- FOUND: engine/src/dmn/feel/numeric.rs
- FOUND: engine/tests/tck_precision_audit.rs
- FOUND: engine/tests/fixtures/tck/README.md
- FOUND: engine/tests/fixtures/tck/LICENSE-ASL-2.0.txt
- FOUND: engine/tests/fixtures/tck/0100-arithmetic/
- FOUND: engine/tests/fixtures/tck/1100-feel-decimal-function/
- FOUND: engine/tests/fixtures/tck/1141/ ... 1144/
- FOUND: engine/tests/fixtures/tck/0002-input-data-number/

**Commits (verified in git log):**
- FOUND: f635a4a5d (test 30-01 RED)
- FOUND: 59fbf2dbd (feat 30-01 GREEN Task 1)
- FOUND: 7d820980d (test 30-01 RED Task 2)
- FOUND: 83d1be74a (feat 30-01 GREEN Task 2)

---
*Phase: 30-feel-value-types-type-lattice-temporal-domain*
*Completed: 2026-07-07*