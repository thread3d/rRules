# TCK Coverage Gaps — measured state

## Summary (re-measured 2026-10-25)

**Layer results: 143 passed, 31 failed, 0 skipped across 51 vectors.**

The v8.0 close-out reported "51/51 evaluator coverage, 123 layer passes, zero
failures". Fourteen of those vectors shipped a test XML that was a copy of the
model, so their evaluator layer was never executed at all; the real upstream
cases are now vendored and every vector runs every layer. That number was not measurable: the sweep recorded only *passing*
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

Remaining failure classes (31 layer failures):

| Vector(s) | Failure | Nature |
|-----------|---------|--------|
| `0100-arithmetic` | A residual set of arithmetic combinations (boxed-context operands, mixed temporal types) | Arithmetic matrix breadth |
| `0069-feel-list`, `0068-feel-equality` | Filter and equality cases where the three-valued answer differs from the TCK's | Three-valued filter semantics |
| `0085-decision-services` | BKM invocation / decision-service outputs | Business-knowledge models are parsed but not evaluated |
| `0033-for-loops` | A `for` whose body yields nulls | Iteration body scoping |
| `0058`–`0063` | Argument-shape and aggregation details (`number()`, `all`/`any` with nulls, `median`, `mode`, `stddev`) | Built-in typing detail |
| `0096`–`0098` | Vectors whose decision is a boxed context of temporal accessors; ISO week edge dates | Boxed-context decision bodies |
| `0001-filter`, `0021-singleton-list`, `0002-string-functions`, `0002-input-data-number` | Item-definition-typed inputs and context-valued outputs | Typed input coercion |
| `0056`, `0057`, `0067`, `0094`, `1141`–`1144` | Individual case mismatches (modulo sign detail, context keys with spaces, regex split, `product` arity, unknown named parameter) | Per-case debugging |

The baseline gate in `engine/tests/tck_sweep.rs` (143 passed / 31 failed) makes a regression fail
the suite while these documented gaps remain.
