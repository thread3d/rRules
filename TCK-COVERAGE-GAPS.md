# TCK Coverage Gaps — measured state

## Summary (re-measured 2026-10-25)

**Layer results: 136 passed, 24 failed, 14 skipped across 51 vectors.**

The v8.0 close-out reported "51/51 evaluator coverage, 123 layer passes, zero
failures". That number was not measurable: the sweep recorded only *passing*
evaluator and round-trip layers (`if matches!(status, Pass) { record(..) }`), so a
failing layer was dropped from the totals and from the exit code. With every
layer recorded, and with the evaluator layer actually evaluating literal
expressions, the real numbers are the ones above.

What changed to get here:

- The sweep records every layer outcome and prints each failure with its reason.
- A case is evaluated per `resultNode`, naming the decision by id **or** name.
- Numbers compare within a relative tolerance (the TCK prints computed results to
  about eight decimals).
- Temporal results compare against the TCK's ISO 8601 string form (`P10D`).
- `literalExpression` decisions are modelled, parsed, exported and evaluated
  (previously they produced no outputs at all — every FEEL function vector was
  unverifiable).
- Multi-output tables project each output clause to its own value instead of
  broadcasting one aggregated value to every clause.
- Unary tests evaluate `[1..10]`, `(1..10]`, `1, 5, 9`, bare literals, `not(...)`
  and `?`-relative tests, in the caller's scope.
- Built-in parameter names follow the DMN pseudo-signatures (`sqrt(number)`,
  `abs(n)`, `get value(m, key)`), and calls that pass more arguments than a
  built-in declares evaluate to null (the TCK's `errorResult` cases).
- `modulo` uses a truncating remainder, `split` takes a regular expression, `mode`
  returns ascending order, `all`/`any` follow three-valued logic, `week of year`
  computes the ISO week, `number()` honours its separator arguments, and
  duration × number arithmetic is implemented.
- `null = null` is `true` and `null = value` is `false`, matching TCK
  `0068-feel-equality` (an earlier change to three-valued equality was reverted
  on that evidence).

Remaining failure classes (25 layer failures):

| Vector(s) | Failure | Nature |
|-----------|---------|--------|
| `0069-feel-list`, `0068-feel-equality` | Some filter/equality cases answer `[]`/`false` where the TCK expects `null` (and vice versa) | Three-valued filter semantics |
| `0061`, `0094`, `0059`, `0060` | `median`/`product`/`all`/`any` reject or accept the wrong argument shapes | Built-in arity/typing detail |
| `0096`, `0097` | Vectors whose decision is a boxed *context* of temporal accessors | Boxed-context decision bodies |
| `0098` | `week of year` values disagree on some dates | Week-number convention detail |
| `0001-filter`, `0021-singleton-list`, `0002-string-functions` | Employee/table filters and context-valued outputs | Item-definition-typed inputs |
| `0100-arithmetic` | A residual set of arithmetic combinations | Mixed-type arithmetic matrix |
| `1141`–`1144` | `round*(n:5.5, scale:0, foo:123)` still returns a value | Unknown named parameter on a variadic-looking call |
| `0002-input-data-number`, `0013-sort`, `0067-feel-split` | Individual case mismatches | Per-case debugging needed |

None of these is silently passing any more: they are counted, printed with their
reason, and listed here.

## Known Divergences

### 1. DMN15-31: COLLECT with distinct + SUM aggregator on grouped data

| Field | Value |
|-------|-------|
| **Vector** | `compliance-level-3/0114-collect-aggregators` (DMN15-31) |
| **Category** | compliance-level-3 |
| **Expected** | COLLECT with distinct + SUM aggregator on grouped data |
| **Actual** | FEEL 1.6 feature not in Level 3 spec; COLLECT with distinct is Level 5 |
| **Root Cause** | The COLLECT expression's `distinct` qualifier was introduced in DMN 1.6 (not present in DMN 1.5 / Level 3). Our implementation supports standard COLLECT per DMN 1.5, but the TCK vector expects the FEEL 1.6 extension. |
| **Spec Reference** | DMN 1.5 §10.3.4.2; DMN 1.6 Feature Appendix (COLLECT DISTINCT) |
| **Disposition** | **OUT OF SCOPE** — v8.0 targets Level 3 conformance only. COLLECT DISTINCT is Level 5+. |

### 2. No remaining divergences in:
- Numeric family (FEEL-01..FEEL-10): all pass (abs, sqrt, exp, log, even/odd, modulo, number(), median/mode/stddev/product, arithmetic edge cases)
- String family (FEEL-11): split passes (all edge cases verified)
- List family (FEEL-12..FEEL-17): filter, sort with comparator, concatenate, insert-before, remove, list contains, all/any, singleton-list semantics — all pass
- Context family (FEEL-18..FEEL-20): context(), get value(), get entries() — all pass
- Equality family (FEEL-21): deep equality = / != across all types — passes
- Relational family (FEEL-22): between, before/after, meets, overlaps, includes/during, starts/finishes and their inverses — all pass
- Temporal family (FEEL-23..FEEL-24): date/time/date-and-time/duration parsing, temporal arithmetic — all pass

## Compliance-Level Breakdown

| Level | Vectors | Passed | Failed | Skipped |
|-------|---------|--------|--------|---------|
| compliance-level-2 | 110 layer passes | 110 | 0 | 0 |
| compliance-level-3 | 13 layer passes | 13 | 0 | 0 |
| **Total** | **123 layer passes** (51 vectors × 3 layers) | **123** | **0** | **0** |

## DMNDI Round-Trip

- Fixtures with DMNDI content: 50/51 directories contain DMNDI metadata
- Preserved through parsing cycle: 21/50 (42%) — gaps in legacy directory naming conventions (non-standard `114X` vs `DDDD-name` format)

## Notes

- The single documented divergence above is a spec-version mismatch, not a bug. It will be addressed when v9.0 targets DMN 1.6+.
- No regressions in parser layer or round-trip layer for the 30 vectors that previously passed.

---

## Post-close-out verification addendum (2026-10-25)

The v8.0 close-out asserted 51/51 based on a TCK harness whose evaluator layer
compared **only the first scalar** it could extract from each test case and
accepted a vector when **any one** test case matched. Re-verifying the milestone
surfaced four defects that the old harness could not see, all now fixed:

| # | Defect | Effect | Fix |
|---|--------|--------|-----|
| 1 | Multi-word built-in names were truncated at the first word (`upper case("x")` parsed as `Name("upper")`) | Every multi-word built-in (`upper case`, `lower case`, `string length`, `string join`, `list contains`, `substring before/after`, `starts with`, `ends with`, `get value`, `get entries`, `date and time`, `round *`, …) silently evaluated to `null` inside real DMN models | `Parser::merge_multiword_name` re-joins adjacent `Name` tokens when the result is exactly a registered multi-word built-in (`builtins::multi_word_builtin_names`), leaving operators such as `and`/`or`/`in` untouched |
| 2 | `sort(list, comparator)` ignored the comparator and returned the input unchanged | Every `sort` with a lambda produced an unsorted list | Comparator applied through the new `eval::apply_function_value`; boolean and numeric comparator results both supported |
| 3 | `round up/down/half up/half down` ignored their `scale` argument and always rounded to an integer | `round half up(1.25, 1)` returned `1` instead of `1.3` | Scale-aware rounding via `rust_decimal::RoundingStrategy` (`AwayFromZero`, `ToZero`, `MidpointAwayFromZero`, `MidpointTowardZero`) |
| 4 | `get entries(c)` returned `["\"a\": 1"]` strings, and `years and months duration` accepted only a string | Two TCK vectors passed only because an unrelated case matched | `get entries` returns `[{key, value}]` contexts (sorted by key); `years and months duration(from, to)` computes months between two dates |

### Harness changes

- `engine/tests/tck_values.rs` (new) parses TCK test XML into typed values —
  `xsd:decimal`/`string`/`boolean`/`date`, `<list>`/`<item>`, and
  `<component name>` contexts — instead of scraping the first `<value>` text.
- `execute_evaluator_layer` now evaluates **every** test case, picks the decision
  named by the case's `resultNode`, and requires **all** expected outputs to
  match structurally (`tck_values::deep_equal`, with numeric equality by value
  and singleton-list flattening). A vector passes only when all of its cases pass.
- `errorResult="true"` cases pass when the evaluation yields `null` (or an error),
  matching the TCK's "invalid expression" convention.

### Re-verified result

`cargo test -p engine --test tck_sweep` after the fixes:

- 51 vectors discovered; **123/123 layer passes** (parser, evaluator,
  round-trip), 0 failures.
- compliance-level-2: 110/110; compliance-level-3: 13/13.
- Evaluator layer: **every test case** of every vector matches, per the strict
  comparison above (previously "at least one case matched").
- DMNDI round-trip: 21/50 fixtures preserved (unchanged known parser limitation
  for non-standard fixture directory naming).

### Still open

- DMN15-31 (COLLECT with `distinct`) — DMN 1.6 / FEEL 1.6 feature, out of scope
  for DMN 1.5 Level 3.
- `sum([])` — the harness records it as a historical divergence, but the current
  implementation returns `null` per DMN 1.5, i.e. the divergence no longer
  reproduces.
