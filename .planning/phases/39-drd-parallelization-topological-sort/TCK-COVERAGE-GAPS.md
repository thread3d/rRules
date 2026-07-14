# TCK Coverage Gaps — PERF-12 (Phase 39)

**Generated:** 2026-07-14
**TCK source:** https://github.com/dmn-tck/tck (commit `0dbcaf9`)
**Vendored vectors:** 21 → 51 (target was 35+)

## Summary

PERF-12 expanded the vendored OMG DMN TCK corpus from 21 to 51 Level 3
directories covering the FEEL built-in function families. The sweep
(`cargo test --test tck_sweep`) is green: **51 vectors discovered, 123
layer-passes, 0 failed, 0 skipped.**

The sweep counts a vector as **failed** only on a *parser-layer* or
*DMNDI-layer* failure. Every vendored vector parses and round-trips
structurally. The **evaluator** layer is lenient (a vector records an
evaluator pass if *any* test case's output matches the expected value);
a non-matching evaluator layer does **not** fail the sweep — it simply
records no evaluator pass for that vector.

## Coverage status by layer

| Layer | Vectors passing |
|-------|-----------------|
| Parser (parse + export + re-parse structural compare) | 51 / 51 |
| Round-trip (IR equality) | 51 / 51 |
| Evaluator (output matches expected) | 21 / 51 |

The 21 vectors that pass the evaluator layer are the original Phase 35
corpus (simple tables, hit policies, basic arithmetic, list ops, for-loops,
date-time, decision services that resolve through implemented built-ins,
and `1100-feel-decimal-function`).

## Remaining evaluator-layer gaps (30 vectors)

These vectors parse and round-trip cleanly but the evaluator does not yet
produce outputs matching the TCK expected values. They are the roadmap for
future FEEL built-in implementation. Grouped by DMN 1.5 §10.3 function
family:

### Numeric built-ins (§10.3.4)
- `0050-feel-abs-function` — `abs()`
- `0051-feel-sqrt-function` — `sqrt()`
- `0052-feel-exp-function` — `exp()`
- `0053-feel-log-function` — `log()`
- `0054-feel-even-function` — `even()`
- `0055-feel-odd-function` — `odd()`
- `0056-feel-modulo-function` — `modulo()`
- `0058-feel-number-function` — `number()`
- `0061-feel-median-function` — `median()`
- `0062-feel-mode-function` — `mode()`
- `0063-feel-stddev-function` — `stddev()`
- `0094-feel-product-function` — `product()`
- `0100-arithmetic` — extended arithmetic edge cases (e.g. empty-list
  SUM/MIN/MAX → null; documented known divergence, see TCK_RESULTS.md)
- `0099-arithmetic-negation` *(not vendored — candidate for next batch)*

### String built-ins (§10.3.4.2)
- `0067-feel-split-function` — `split()`

### List built-ins (§10.3.4.3)
- `0001-filter` — `filter()`
- `0010-concatenate` — `concatenate()`
- `0011-insert-remove` — `insert()`, `remove()`
- `0013-sort` — `sort()`, `list contains()`
- `0021-singleton-list` — singleton/list semantics
- `0059-feel-all-function` — `all()`
- `0060-feel-any-function` — `any()`
- `0069-feel-list` — `distinct values()`, `list contains()`

### Context built-ins (§10.3.4.4)
- `0057-feel-context` — context construction / navigation
- `0080-feel-getvalue-function` — `get value()`
- `0081-feel-getentries-function` — `get entries()`

### Relational / range (§10.3.4.5, §10.3.4.9)
- `0068-feel-equality` — deep equality across types
- `0071-feel-between` — `between()`

### Temporal built-ins (§10.3.4.3.2)
- `0096-feel-day-of-week-function` — `day of week()`
- `0097-feel-month-of-year-function` — `month of year()`
- `0098-feel-week-of-year-function` — `week of year()`

### Decision services / BKMs (pre-existing gaps)
- `0085-decision-services` — multi-output decision service output shaping
- `1141`, `1142`, `1143`, `1144` — DMN 1.4/1.5 feature vectors

## Known divergences (carried from Phase 35)

Recorded in `TCK_RESULTS.md` → Known Divergences:

1. **`DMN15-31-collect-distinct-sum`** — COLLECT with `distinct` is a DMN 1.6
   (FEEL 1.6) feature, outside Level 3 scope. No action for Level 3.
2. **`sum-empty-list`** (`0100-arithmetic`) — `SUM` of an empty list returns
   `Decimal(0)` in our evaluator (rust_decimal empty-iterator default) where
   the spec requires `null`. Affects MIN/MAX of empty lists too. Spec ref:
   DMN 1.5 §11.3.4.

## Next-step work (future phases, out of v7.0 scope)

Implementing the §10.3.4 numeric/string/list/context built-in functions
above would flip the 30 eval-gap vectors green and raise evaluator-layer
coverage from 21/51 toward 51/51. Each family is independently
implementable in `engine/src/dmn/feel/builtins.rs` + `eval.rs`, with the
TCK vectors providing ready-made regression tests through the existing
sweep harness.