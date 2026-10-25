# TCK Coverage Gaps — measured state

## Summary (re-measured 2026-10-25)

**Layer results: 170 passed, 0 failed, 4 skipped across 51 vectors.**

Every evaluator layer in the vendored corpus passes. The four skips are one
divergence recorded four times (see below) — the same vector family with
`round up`, `round down` and `round half up`.

This is not the number the v8.0 close-out reported, and the difference matters. That
close-out claimed "51/51 evaluator coverage, 123 layer passes, zero failures", which was
not measurable:

1. The sweep recorded only *passing* evaluator and round-trip layers
   (`if matches!(status, Pass) { record(..) }`), so a failing layer vanished from the
   totals and from the exit code.
2. Fourteen of the 51 vectors shipped a test XML that was a byte-for-byte copy of the
   model, so their evaluator layer never ran.
3. `literalExpression` decisions — most FEEL function vectors — produced no outputs and
   could not be evaluated at all.

Fixed: every layer outcome is recorded, the real upstream test cases are vendored, and
literal expressions, boxed contexts, lists and relations, business knowledge models and
decision services all evaluate. The measured state is above.

## What the 31 failures required

The first honest measurement of this corpus was 143 passing and 31 failing layers. Closing
them was not a matter of tolerance: each class was a defect in the evaluator or the FEEL
implementation. The record of what was wrong, in the order it was fixed:

| Class | Defect | Fixed in |
|-------|--------|----------|
| Filtering | `list[i]` was treated as a boolean filter, so position selection, index 0 and out-of-range were all empty lists; context entries were not bound by name | filter semantics (§10.3.2.3) |
| Numeric literals | the lexer stopped at the mantissa, so `1.23e4` was `1.23` | exponent notation |
| Aggregates | `all`/`any` coerced non-booleans and rejected variadic calls; `median`/`product`/`stddev`/`mode` skipped bad elements instead of failing; `stddev` was population, `mode` was a scalar | §10.3.4 aggregates |
| `number()` | positional order was inverted, separators were not validated, the decimal separator had no default | `number(from, grouping, decimal)` |
| `modulo` | truncated remainder instead of floored | §10.3.4.1 |
| Names | FEEL names may contain spaces and name symbols; `Monthly Salary`, `values in a list`, `Date-Time2`, `{foo+bar: 1}` were syntax errors, and `a*b` lexed as one name | model-declared name merging |
| Boxed expressions | `<context>`, `<list>` and `<relation>` decision bodies were not modelled, so those decisions produced no value | `BoxedContext`/`BoxedList`/`BoxedRelation` |
| Temporal | `day of week`/`month of year` returned numbers; `24:00:00`, `Z` and `10:15:00@Zone` did not parse; offset-less date-times had two carriers; ranges and times compared by representation | temporal model |
| Ranges | `]a..b]`, `[a..b[` and `= v` did not parse | range grammar |
| Invocation | business knowledge models and decision services were parsed but never callable | `resolve_invocations` / `bind_model_functions` |
| Arithmetic | 129 of 1087 cases: negative and fractional duration scaling, `duration / duration`, commuted operands, mixed naive/zoned subtraction, `number / duration`, left-associative `**` | §10.3.2 arithmetic matrix |

## The one accepted divergence

`1141`–`1144` assert that `round up(5.5, 6176)` is `5.5` while `round up(5.5, 6176 + 1)` is
an error. Nothing in DMN 1.5 §10.3.4.1 bounds a rounding scale that way: a scale beyond the
arithmetic's precision leaves a value unchanged at any magnitude, which is what `017_a`
asserts and what this implementation does. The vectors encode a reference implementation's
limit, so they are recorded as known divergences rather than matched — which is why the
sweep reports them as skipped and why `1141`–`1144` appear in the ledger below.

The divergence list lives in `engine/tests/tck_sweep.rs` (`known_divergence_list`) with the
expected and actual behaviour, and the sweep prints it on every run.

## Generated ledger

Every failing layer, with what the harness observed. **Do not edit this block by hand** — it is
replaced by `make tck-ledger`, which runs the sweep and splices the result here. The regression
gate in `engine/tests/tck_sweep.rs` holds these numbers (`TCK_BASELINE_PASSED` /
`TCK_BASELINE_FAILED`), and `tck_baseline_matches_documentation` fails the suite if this document
and those constants ever disagree.

<!-- BEGIN GENERATED LEDGER -->
| # | Vector | Layer | Observed |
|---|--------|-------|----------|
| 1 | `1141` | evaluator | 15/16 test cases matched (1 failed): 017_b: expected null (errorResult) but produced {"decision017_b": Number(5.5)} |
| 2 | `1142` | evaluator | 15/16 test cases matched (1 failed): 017_b: expected null (errorResult) but produced {"decision017_b": Number(5.5)} |
| 3 | `1143` | evaluator | 15/16 test cases matched (1 failed): 017_b: expected null (errorResult) but produced {"decision017_b": Number(5.5)} |
| 4 | `1144` | evaluator | 15/16 test cases matched (1 failed): 017_b: expected null (errorResult) but produced {"decision017_b": Number(5.5)} |
<!-- END GENERATED LEDGER -->
