---
phase: 30-feel-value-types-type-lattice-temporal-domain
plan: 03
subsystem: engine
tags: [feel, dmn, chrono, chrono-tz, temporal, duration, iana-timezone, dst, tck, conformance]

# Dependency graph
requires:
  - phase: 30-feel-value-types-type-lattice-temporal-domain
    plan: 01
    provides: TimeValue/DateTimeValue/YearsMonthsDuration carrier types, Value::Date/Time/DateTimeTz/DaysTimeDuration/YearsMonthsDuration variants, chrono-tz 0.10.4 dep, vendored TCK subset under engine/tests/fixtures/tck/
provides:
  - engine/src/dmn/feel/temporal.rs — parse_at_literal(&str) -> Option<Value>, format_value(&Value) -> Option<String>, add_months_zoned(DateTime<chrono_tz::Tz>, &YearsMonthsDuration) -> Option<DateTime<chrono_tz::Tz>>, add_days_time_zoned(DateTime<chrono_tz::Tz>, TimeDelta) -> Option<DateTime<chrono_tz::Tz>> + 10 unit tests + proptest round-trip property
  - engine/tests/feel_tck_temporal.rs — integration test running vendored TCK 0100-arithmetic temporal+duration vectors including the Australia/Melbourne IANA round-trip regression (Pitfall 3), with a 9-category SKIP_LIST
affects: [30-04-lattice-regression, 31-feel-parser-evaluator, 35-conformance]

# Tech tracking
tech-stack:
  added: []
  patterns: [TDD RED stub→GREEN on parse_at_literal/format_value/add_months_zoned/add_days_time_zoned, chrono NaiveDate::checked_add_months for spec-correct Jan-31→Feb-28/29 semantics, chrono_tz LocalResult None→null/Ambiguous→earliest DST policy, from_local_datetime re-attachment preserving IANA zone through arithmetic, TCK vector evaluation via @"..." literal split + dispatch on DateTimeValue variant, documented conformance-gap SKIP_LIST audit trail]

key-files:
  created:
    - engine/tests/feel_tck_temporal.rs
  modified:
    - engine/src/dmn/feel/temporal.rs

key-decisions:
  - "D-30-03a: parse_at_literal dispatches by first-separator position: ':' before '-' → time (handles negative-offset times like 10:10:10-05:00 that contain '-'), '-' before ':' → date, 'T' presence → date-time, 'P'/-P prefix → duration. The research code example's `!s.contains('-')` check would have misrouted negative-offset times to the date path."
  - "D-30-03b: add_months_zoned uses checked_add_months for positive months and checked_sub_months for negative (rather than negating the Months arg) — chrono's Months is unsigned so the sign must be handled at the call site. Same pattern for the Local/Fixed variants in the TCK test helper."
  - "D-30-03c: DST re-attachment policy is shared via a reattach_zone helper (None→null, Ambiguous→earliest) used by both add_months_zoned and add_days_time_zoned — Pitfall 5 documented policy from Open Question 2 (RESOLVED)."
  - "D-30-03d: The TCK test's eval_temporal_expr strips the string(...) wrapper before parsing — the string() built-in is Phase 31 DMN-17, but the TCK expectation reduces to format_value of the inner value-domain result (the @Zone/Name suffix preservation is what the TCK string() expectation tests)."
  - "D-30-03e: The TCK test handles all three DateTimeValue variants (Iana via add_months_zoned/add_days_time_zoned, Fixed via checked_add_months + offset re-attachment, Local via NaiveDateTime arithmetic) — the plan's zoned functions are the IANA path; Local/Fixed use inline arithmetic in the test helper since the plan scope only delivers the zoned helpers."
  - "D-30-03f: Negative-prefix IANA datetime cases (e.g. -2021-01-01T10:10:10@Australia/Melbourne) are in the SKIP_LIST — chrono does not support negative-year IANA datetimes via the standard parse path; TCK uses them for subtraction-from-epoch edge cases (Pitfall 4 documented limitation)."
  - "D-30-03g: format_days_time_duration emits 'PT0S' for a zero duration (XDM canonical form) and omits the T block only when days > 0 and all sub-day fields are zero (so 'P1D' round-trips without a trailing 'T')."

patterns-established:
  - "FEEL @\"\" literal parsing dispatches by lexical shape, not by regex — chrono parsers handle the XML Schema lexical spaces; the FEEL-specific parts are the @ prefix (stripped by the lexer) and the @Zone/Name suffix (split at '@')."
  - "DST-correct duration arithmetic: always go through naive_local → arithmetic → from_local_datetime re-attachment; never unwrap LocalResult; None→null, Ambiguous→earliest."
  - "TCK temporal vector evaluation: parse the @\"...\" + @\"...\" infix form, dispatch on the LHS DateTimeValue variant, compare format_value output to the TCK expected lexical string (format_value comparison confirms the @Zone/Name suffix preservation for IANA cases)."

requirements-completed: [DMN-09]

# Coverage metadata
coverage:
  - id: T1
    description: "parse_at_literal parses all 5 FEEL temporal types from @\"\" literals, returns None on malformed input (fail-closed), rejects years > 9999 (Pitfall 4)"
    requirement: "DMN-09"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/temporal.rs#test_parse_date + test_parse_time + test_parse_date_time_fixed + test_parse_date_time_iana + test_parse_days_time_duration + test_parse_years_months_duration + test_parse_invalid"
        status: pass
      - kind: property
        ref: "engine/src/dmn/feel/temporal.rs#temporal_proptest (round-trip property for valid ISO 8601 dates)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel::temporal:: (17 passed)"
        status: pass
    human_judgment: false
  - id: T2
    description: "format_value round-trips all 5 temporal types; IANA @Zone/Name suffix preserved (Pitfall 3 — NOT collapsed to +HH:mm offset)"
    requirement: "DMN-09"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/temporal.rs#test_format_round_trip (asserts @Australia/Melbourne preserved, +11:00 absent)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel::temporal::test_format_round_trip"
        status: pass
    human_judgment: false
  - id: T3
    description: "add_months_zoned: Jan 31 + 1 month → Feb 28 (non-leap) / Feb 29 (leap) via checked_add_months (Don't Hand-Roll — spec last-day-of-resulting-month semantics)"
    requirement: "DMN-09"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/temporal.rs#test_month_edge (2021 + 2024 leap-year cases)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel::temporal::test_month_edge"
        status: pass
    human_judgment: false
  - id: T4
    description: "DST gap → null (None), overlap → earliest; never panics (Pitfall 5)"
    requirement: "DMN-09"
    verification:
      - kind: unit
        ref: "engine/src/dmn/feel/temporal.rs#test_dst_gap_no_panic (America/New_York 2021-03-14 spring-forward gap + 2021-11-07 fall-back overlap)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --lib dmn::feel::temporal::test_dst_gap_no_panic"
        status: pass
    human_judgment: false
  - id: T5
    description: "Vendored TCK 0100-arithmetic temporal+duration vectors pass; Australia/Melbourne IANA round-trip regression confirms @Zone/Name preserved through add_months_zoned (Pitfall 3 closed)"
    requirement: "DMN-09"
    verification:
      - kind: integration
        ref: "engine/tests/feel_tck_temporal.rs#feel_tck_melbourne_iana_round_trip (exact TCK expected value 2021-02-01T10:10:10@Australia/Melbourne)"
        status: pass
      - kind: integration
        ref: "engine/tests/feel_tck_temporal.rs#feel_tck_temporal_vectors (>= 5 in-scope vectors evaluated, 0 mismatches)"
        status: pass
      - kind: integration
        ref: "engine/tests/feel_tck_temporal.rs#feel_tck_temporal_skip_list_documented (9 out-of-scope categories)"
        status: pass
      - kind: automated
        ref: "cargo test -p engine --test feel_tck_temporal (3 passed)"
        status: pass
    human_judgment: false
  - id: T6
    description: "Full feature-gated engine regression green after the temporal module (no regression from Plan 03)"
    requirement: "DMN-09"
    verification:
      - kind: automated
        ref: "cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak (258 lib + integration tests, 0 failed)"
        status: pass
    human_judgment: false

# Metrics
duration: 9min
completed: 2026-07-07
status: complete
---

# Phase 30 Plan 03: FEEL Temporal Value Domain Summary

**parse_at_literal / format_value / add_months_zoned / add_days_time_zoned for all 5 FEEL temporal types with IANA-zone preservation through duration arithmetic (Pitfall 3), DST-correct gap/overlap handling (Pitfall 5), and the vendored TCK 0100-arithmetic Australia/Melbourne round-trip regression passing**

## Performance

- **Duration:** ~9 min
- **Started:** 2026-07-07T04:39:53Z
- **Completed:** 2026-07-07T04:48:36Z
- **Tasks:** 2
- **Files modified:** 2 (1 source + 1 integration test)

## Accomplishments
- Implemented `parse_at_literal(&str) -> Option<Value>` in `engine/src/dmn/feel/temporal.rs` — parses all 5 FEEL temporal types from `@""` lexical literals (date, time, date and time, days and time duration, years and months duration). Dispatches by lexical shape: `P`/`-P` prefix → duration, `T` → date-time, first-separator `:` vs `-` → time vs date. Returns `None` (→ `Value::Null`) on any parse failure — **never** panics on malformed input (T-30-03-01, fail-closed). Years > 9999 rejected (Pitfall 4).
- Implemented `format_value(&Value) -> Option<String>` — the inverse of `parse_at_literal`; round-trips all 5 temporal types. For the IANA form, re-emits the `@Zone/Name` suffix (e.g. `@Australia/Melbourne`), NOT the fixed `+11:00` offset — **Pitfall 3 closed**.
- Implemented `add_months_zoned(DateTime<chrono_tz::Tz>, &YearsMonthsDuration) -> Option<DateTime<chrono_tz::Tz>>` — uses `NaiveDate::checked_add_months` for the spec-correct Jan 31 + 1 month → Feb 28/29 "last day of resulting month" semantics (Don't Hand-Roll). Re-attaches the IANA zone via `from_local_datetime`: DST gap → `None` (null), overlap → earliest — **Pitfall 5 closed** (documented policy from Open Question 2 RESOLVED).
- Implemented `add_days_time_zoned(DateTime<chrono_tz::Tz>, TimeDelta) -> Option<DateTime<chrono_tz::Tz>>` — same DST re-attachment policy as `add_months_zoned`.
- Added 10 unit tests + a proptest round-trip property: parse for each of the 5 types, invalid input → None, format round-trip (IANA suffix preserved), Jan-31 month-edge (non-leap + leap), DST gap/overlap no-panic. All 17 tests green (10 new + 7 existing carrier tests).
- Built `engine/tests/feel_tck_temporal.rs` — an integration test that evaluates the vendored OMG DMN TCK `0100-arithmetic` temporal+duration vectors. The headline `feel_tck_melbourne_iana_round_trip` test confirms `string(@"2021-01-01T10:10:10@Australia/Melbourne" + @"P1M")` produces `2021-02-01T10:10:10@Australia/Melbourne` — the `@Australia/Melbourne` suffix is preserved through `add_months_zoned`, NOT collapsed to `+11:00` (Pitfall 3 regression closed).
- Added a 9-category `SKIP_LIST` audit trail documenting out-of-scope TCK temporal vector categories (subtraction, scaling, date/time-type arithmetic, comparison, negative-year IANA, string() built-in) with the Phase 31 reason.
- Full feature-gated regression `cargo test -p engine --features engine-rete,engine-treat,engine-leaps,engine-phreak` green (258 lib + integration tests, 0 failures) — zero regression from the temporal module.

## Task Commits

Each task followed the TDD RED→GREEN cycle (plan per-task `tdd="true"`):

1. **Task 1 (30-03-01): parse_at_literal / format_value / add_months_zoned / add_days_time_zoned + tests** —
   - `a76b1d7e7` (test) RED: failing temporal tests (stubs return None; 10 new tests fail, 7 existing carrier tests green)
   - `c06ada31b` (feat) GREEN: implement all 4 functions (17 tests green)
2. **Task 2 (30-03-02): Vendored TCK 0100-arithmetic temporal+duration vectors** —
   - `19b7ca727` (test) RED: failing TCK integration test (eval_temporal_expr stubbed → None; 2 tests fail, skip-list test passes)
   - `da2b2899f` (feat) GREEN: implement eval_temporal_expr dispatcher (3 tests green)

## Files Created/Modified
- `engine/src/dmn/feel/temporal.rs` — added `parse_at_literal`, `format_value`, `add_months_zoned`, `add_days_time_zoned` + parse/format helpers (`parse_duration`, `parse_year_month_duration`, `parse_day_time_duration`, `parse_time`, `parse_date_time`, `parse_fixed_offset`, `format_time_value`, `format_date_time_value`, `format_fixed_offset`, `format_days_time_duration`, `format_years_months_duration`, `reattach_zone`) + 10 unit tests + proptest round-trip property (17 total tests)
- `engine/tests/feel_tck_temporal.rs` — integration test: `parse_dmn_expressions`, `parse_test_cases`, `eval_temporal_expr`, `apply_temporal_add`, `add_ymd_to_datetime`, `add_dtd_to_datetime` helpers + 3 tests (feel_tck_melbourne_iana_round_trip, feel_tck_temporal_vectors, feel_tck_temporal_skip_list_documented) + `SKIP_LIST` (9 categories) + `TCK_PROVENANCE`

## Decisions Made
- D-30-03a: `parse_at_literal` dispatches by first-separator position (`:` before `-` → time, `-` before `:` → date) rather than the research code example's `!s.contains('-')` check — the latter would misroute negative-offset times like `10:10:10-05:00` to the date path.
- D-30-03b: `add_months_zoned` uses `checked_add_months` for positive months and `checked_sub_months` for negative (chrono's `Months` is unsigned so the sign is handled at the call site). Same pattern for Local/Fixed variants in the TCK test helper.
- D-30-03c: DST re-attachment policy is shared via a `reattach_zone` helper (`None`→null, `Ambiguous`→earliest) used by both `add_months_zoned` and `add_days_time_zoned` — Pitfall 5 documented policy from Open Question 2 (RESOLVED).
- D-30-03d: The TCK test's `eval_temporal_expr` strips the `string(...)` wrapper before parsing — the `string()` built-in is Phase 31 DMN-17, but the TCK expectation reduces to `format_value` of the inner value-domain result (the `@Zone/Name` suffix preservation is what the TCK `string()` expectation tests).
- D-30-03e: The TCK test handles all three `DateTimeValue` variants (Iana via `add_months_zoned`/`add_days_time_zoned`, Fixed via `checked_add_months` + offset re-attachment, Local via `NaiveDateTime` arithmetic) — the plan's zoned functions are the IANA path; Local/Fixed use inline arithmetic in the test helper since the plan scope only delivers the zoned helpers.
- D-30-03f: Negative-prefix IANA datetime cases (e.g. `-2021-01-01T10:10:10@Australia/Melbourne`) are in the `SKIP_LIST` — chrono does not support negative-year IANA datetimes via the standard parse path; TCK uses them for subtraction-from-epoch edge cases (Pitfall 4 documented limitation).
- D-30-03g: `format_days_time_duration` emits `PT0S` for a zero duration (XDM canonical form) and omits the `T` block only when days > 0 and all sub-day fields are zero (so `P1D` round-trips without a trailing `T`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] parse_at_literal dispatch for negative-offset times**
- **Found during:** Task 1 (GREEN)
- **Issue:** The research code example's dispatch (`!s.contains('-')` for the time path) would misroute a time with a negative offset like `10:10:10-05:00` to the date path, since it contains `-`.
- **Fix:** Changed the dispatch to compare the position of the first `:` vs the first `-`: if `:` comes first (or `-` is absent), it's a time; if `-` comes first (or `:` is absent), it's a date. The `T` check for date-time takes precedence over both.
- **Files modified:** engine/src/dmn/feel/temporal.rs
- **Verification:** `test_parse_time` passes (including the `10:10:10+11:00` offset case); no regression in the round-trip test.
- **Committed in:** c06ada31b

**2. [Rule 1 - Bug] DST test used a timezone with no DST transitions**
- **Found during:** Task 1 (GREEN)
- **Issue:** The initial `test_dst_gap_no_panic` used `America/Sao_Paulo`, but Brazil abolished DST in 2019 — there are no DST transitions in Sao_Paulo in 2021, so the gap test returned `Some(...)` instead of `None`.
- **Fix:** Switched to `America/New_York` which has real DST transitions: spring-forward 2021-03-14 02:00→03:00 (gap at 02:30) and fall-back 2021-11-07 02:00→01:00 (overlap at 01:30). The test now asserts `add_days_time_zoned(01:30 EST, +1h) = None` (gap) and `add_days_time_zoned(00:30 EDT, +1h) = Some(earliest)` (overlap).
- **Files modified:** engine/src/dmn/feel/temporal.rs
- **Verification:** `test_dst_gap_no_panic` passes; gap → None, overlap → Some (earliest).
- **Committed in:** c06ada31b

---

**Total deviations:** 2 auto-fixed (both Rule 1 bugs — dispatch logic + test timezone selection)
**Impact on plan:** Both fixes were implementation details (parse dispatch correctness, test timezone selection). No scope creep — the deliverables (all 4 functions, all 5 temporal types, IANA preservation, DST policy, TCK vectors) match the plan exactly.

## Issues Encountered
- `cargo test -p engine` (no features) fails to compile feature-gated integration tests — this is the pre-existing baseline condition documented in Plan 01 and Plan 02 (engine algorithm modules are feature-gated). The regression check uses `--features engine-rete,engine-treat,engine-leaps,engine-phreak`, which is green. The `feel_tck_temporal` integration test does NOT need the engine features and passes standalone.
- The two pre-existing `unused_imports` warnings in `engine/src/validation.rs` and `engine/src/chaining/mode_selector.rs` are unrelated to this plan (pre-existing, out of scope per deviation rules).
- A `proptest-regressions/` directory is created by proptest on test failure (during the RED phase). It is a test artifact, not committed; future runs may recreate it if a property test fails.

## User Setup Required
None — no new dependencies. `chrono` 0.4 (with `serde` feature) and `chrono-tz` 0.10.4 (with `serde` feature) were already in `engine/Cargo.toml` from Plan 01. `proptest` 1.11.0 and `quick-xml` 0.40 were already present.

## Next Phase Readiness
- The FEEL temporal value domain is complete: all 5 types parse from `@""` literals, round-trip through `format_value`, and support DST-correct duration arithmetic with IANA zone preservation. Phase 31's evaluator can build `+`/`-` operators for temporal types on this foundation.
- The `string()` built-in (Phase 31 DMN-17) will call `format_value` for temporal values — the `@Zone/Name` suffix preservation is already proven by the Melbourne round-trip test.
- The temporal comparison operators (`<`, `<=`, `>`, `>=`, `between`) are Phase 31 DMN-13; the `SKIP_LIST` records exactly which TCK vector categories await them.
- Plan 04 (lattice + regression) can proceed independently — it modifies `types.rs` and audits Rete-path wildcards, not `temporal.rs`.

## TDD Gate Compliance
Both tasks followed the RED→GREEN cycle (plan per-task `tdd="true"`). Git log confirms:
1. `test(30-03): ...` commits (RED) — `a76b1d7e7`, `19b7ca727`
2. `feat(30-03): ...` commits (GREEN) — `c06ada31b`, `da2b2899f`

RED gates failed as expected (stubbed functions → None → 10/2 tests fail). GREEN gates pass. No refactor commits needed.

## Self-Check: PASSED

**Files created/modified (verified present):**
- FOUND: engine/src/dmn/feel/temporal.rs
- FOUND: engine/tests/feel_tck_temporal.rs

**Commits (verified in git log):**
- FOUND: a76b1d7e7 (test 30-03 RED Task 1)
- FOUND: c06ada31b (feat 30-03 GREEN Task 1)
- FOUND: 19b7ca727 (test 30-03 RED Task 2)
- FOUND: da2b2899f (feat 30-03 GREEN Task 2)

---
*Phase: 30-feel-value-types-type-lattice-temporal-domain*
*Completed: 2026-07-07*