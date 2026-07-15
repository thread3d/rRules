# Requirements: rRules — Milestone v8.0 Close TCK Evaluator Gaps

**Created:** 2026-07-15
**Status:** Approved
**Scope:** Implement missing FEEL built-in functions to raise TCK evaluator coverage from 21/51 to 51/51 vectors

---

## Phase Structure

| # | Name | Requirements | Status |
|---|------|--------------|--------|
| 40 | Numeric Built-in Functions | FEEL-01..FEEL-10 | not_started |
| 41 | String + List Built-in Functions | FEEL-11..FEEL-17 | not_started |
| 42 | Context + Relational + Temporal Built-in Functions | FEEL-18..FEEL-24 | not_started |
| 43 | TCK Conformance Verification + Close-out | FEEL-25, FEEL-26 | not_started |

**Coverage:** 26/26 v8.0 requirements (FEEL-01..FEEL-26) mapped to exactly one phase, 0 orphans, 4/4 phases with ≥1 requirement.

---

## Phase 40: Numeric Built-in Functions

**Requirements:** FEEL-01..FEEL-10
**Goal:** Implement all missing numeric FEEL built-in functions (§10.3.4.1) so the 14 numeric TCK vectors pass the evaluator layer.

### FEEL-01: `abs()` — Absolute value
- Implement `abs(number)` → returns absolute value of number
- Must handle Integer, Float (Number), and null (propagates)
- TCK vector: `0050-feel-abs-function`

### FEEL-02: `sqrt()` — Square root
- Implement `sqrt(number)` → returns square root
- Must return null for negative inputs (per DMN spec)
- TCK vector: `0051-feel-sqrt-function`

### FEEL-03: `exp()` — Exponential
- Implement `exp(number)` → returns e^n
- TCK vector: `0052-feel-exp-function`

### FEEL-04: `log()` — Natural logarithm
- Implement `log(number)` → returns ln(n)
- Must return null for non-positive inputs
- TCK vector: `0053-feel-log-function`

### FEEL-05: `even()` / `odd()` — Parity checks
- Implement `even(number)` → returns true if integer part is even
- Implement `odd(number)` → returns true if integer part is odd
- TCK vectors: `0054-feel-even-function`, `0055-feel-odd-function`

### FEEL-06: `modulo()` — Modulo operation
- Implement `modulo(dividend, divisor)` → returns remainder
- Must follow DMN 1.5 §10.3.4.1 semantics (not Rust %)
- TCK vector: `0056-feel-modulo-function`

### FEEL-07: `number()` — String to number conversion
- Implement `number(from, grouping, decimal)` → parse string to number
- Must handle grouping/decimal separators per locale-independent spec
- TCK vector: `0058-feel-number-function`

### FEEL-08: Statistical functions — `median()`, `mode()`, `stddev()`
- Implement `median(list)` → returns median value
- Implement `mode(list)` → returns mode (most frequent value)
- Implement `stddev(list)` → returns population standard deviation
- All must return null for empty lists
- TCK vectors: `0061-feel-median-function`, `0062-feel-mode-function`, `0063-feel-stddev-function`

### FEEL-09: `product()` — Product of list
- Implement `product(list)` → returns product of all elements
- Must return null for empty list
- TCK vector: `0094-feel-product-function`

### FEEL-10: Extended arithmetic edge cases
- Handle empty-list SUM/MIN/MAX → null (known divergence)
- Handle negation edge cases
- TCK vectors: `0100-arithmetic`, `0099-arithmetic-negation`

---

## Phase 41: String + List Built-in Functions

**Requirements:** FEEL-11..FEEL-17
**Goal:** Implement missing string and list FEEL built-in functions (§10.3.4.2, §10.3.4.3).

### FEEL-11: `split()` — String splitting
- Implement `split(string, delimiter)` → returns list of substrings
- TCK vector: `0067-feel-split-function`

### FEEL-12: `filter()` — List filtering
- Implement `filter(list, function)` → returns filtered list
- Must support FEEL lambda/function as second argument
- TCK vector: `0001-filter`

### FEEL-13: `concatenate()` — List concatenation
- Implement `concatenate(list1, list2, ...)` → returns concatenated list
- TCK vector: `0010-concatenate`

### FEEL-14: `insert()` / `remove()` — List manipulation
- Implement `insert(list, position, element)` → returns new list
- Implement `remove(list, position)` → returns new list
- TCK vector: `0011-insert-remove`

### FEEL-15: `sort()` / `list contains()` — List ordering and membership
- Implement `sort(list)` → returns sorted list (default ascending)
- Implement `sort(list, function)` → returns sorted list with comparator
- Implement `list contains(list, element)` → returns boolean
- TCK vectors: `0013-sort`, `0069-feel-list`

### FEEL-16: `all()` / `any()` — Quantifier functions
- Implement `all(list)` → returns true if all elements are true
- Implement `any(list)` → returns true if any element is true
- TCK vectors: `0059-feel-all-function`, `0060-feel-any-function`

### FEEL-17: Singleton/list semantics
- Ensure singleton lists and single elements behave correctly per DMN 1.5
- TCK vector: `0021-singleton-list`

---

## Phase 42: Context + Relational + Temporal Built-in Functions

**Requirements:** FEEL-18..FEEL-24
**Goal:** Implement missing context, relational, and temporal FEEL built-in functions (§10.3.4.4, §10.3.4.5, §10.3.4.9, temporal).

### FEEL-18: Context construction / navigation
- Implement context literal evaluation with proper scoping
- TCK vector: `0057-feel-context`

### FEEL-19: `get value()` — Context value access
- Implement `get value(context, key)` → returns value for key
- TCK vector: `0080-feel-getvalue-function`

### FEEL-20: `get entries()` — Context entry listing
- Implement `get entries(context)` → returns list of key-value entries
- TCK vector: `0081-feel-getentries-function`

### FEEL-21: Deep equality across types
- Implement `=` / `!=` for all FEEL types (including context, list, temporal)
- TCK vector: `0068-feel-equality`

### FEEL-22: `between()` — Range check
- Implement `between(value, start, end)` → returns true if value in [start, end]
- TCK vector: `0071-feel-between`

### FEEL-23: Temporal built-in functions
- Implement missing temporal functions: `date()`, `time()`, `date and time()`, `duration()`
- Handle string→temporal parsing and temporal arithmetic
- TCK vectors: temporal family vectors

### FEEL-24: Remaining built-in families
- Implement any remaining §10.3.4 functions not covered above
- Ensure all 51 TCK vectors have evaluator-layer passes

---

## Phase 43: TCK Conformance Verification + Close-out

**Requirements:** FEEL-25, FEEL-26
**Goal:** Verify all 51 TCK vectors pass the evaluator layer and close out the milestone.

### FEEL-25: Full TCK evaluator sweep
- Run `cargo test --test tck_sweep` and verify 51/51 vectors pass all three layers (parser, round-trip, evaluator)
- Document any remaining known divergences with spec references
- Update TCK-COVERAGE-GAPS.md to reflect 51/51 evaluator coverage

### FEEL-26: Milestone verification and close-out
- Write VERIFICATION.md for all phases
- Run milestone audit
- Tag v8.0

---

## Success Criteria

### Functional
- All 51 vendored TCK vectors pass parser + round-trip + evaluator layers
- Zero regressions in existing DMN evaluation path (hit policies, DRD, existing built-ins)
- All FEEL built-in functions handle null propagation correctly per DMN 1.5 §10.3.4

### Quality
- Each new built-in function has unit tests covering normal, edge, and null cases
- TCK sweep remains green (0 failed, 0 skipped)
- No panics in fuzz targets after built-in additions

---

## Requirements Coverage

| Requirement | Phase | Status | Notes |
|-------------|-------|--------|-------|
| FEEL-01: abs() | 40 | not_started | TCK 0050 |
| FEEL-02: sqrt() | 40 | not_started | TCK 0051 |
| FEEL-03: exp() | 40 | not_started | TCK 0052 |
| FEEL-04: log() | 40 | not_started | TCK 0053 |
| FEEL-05: even()/odd() | 40 | not_started | TCK 0054, 0055 |
| FEEL-06: modulo() | 40 | not_started | TCK 0056 |
| FEEL-07: number() | 40 | not_started | TCK 0058 |
| FEEL-08: median()/mode()/stddev() | 40 | not_started | TCK 0061, 0062, 0063 |
| FEEL-09: product() | 40 | not_started | TCK 0094 |
| FEEL-10: arithmetic edge cases | 40 | not_started | TCK 0100, 0099 |
| FEEL-11: split() | 41 | not_started | TCK 0067 |
| FEEL-12: filter() | 41 | not_started | TCK 0001 |
| FEEL-13: concatenate() | 41 | not_started | TCK 0010 |
| FEEL-14: insert()/remove() | 41 | not_started | TCK 0011 |
| FEEL-15: sort()/list contains() | 41 | not_started | TCK 0013, 0069 |
| FEEL-16: all()/any() | 41 | not_started | TCK 0059, 0060 |
| FEEL-17: singleton/list semantics | 41 | not_started | TCK 0021 |
| FEEL-18: context construction | 42 | not_started | TCK 0057 |
| FEEL-19: get value() | 42 | not_started | TCK 0080 |
| FEEL-20: get entries() | 42 | not_started | TCK 0081 |
| FEEL-21: deep equality | 42 | not_started | TCK 0068 |
| FEEL-22: between() | 42 | not_started | TCK 0071 |
| FEEL-23: temporal built-ins | 42 | not_started | temporal family |
| FEEL-24: remaining families | 42 | not_started | catch-all |
| FEEL-25: full TCK sweep | 43 | not_started | 51/51 verification |
| FEEL-26: milestone close-out | 43 | not_started | audit + tag |

**Coverage:** 26/26 v8.0 requirements (FEEL-01..FEEL-26) mapped to exactly one phase, 0 orphans, 4/4 phases with ≥1 requirement.
