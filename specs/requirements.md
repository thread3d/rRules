# rRules — normative requirements

This file is the authoritative, machine-readable statement of what the system must
do. It is written for [CodeWitness](../../CodeWitness): every `# <ID> <title>` heading
below is one requirement, the paragraph under it is the claim, and CodeWitness
reports `VERIFIED` / `PARTIALLY_VERIFIED` / `FAILED` / `UNVERIFIED` / `BLOCKED` for
each one, with the evidence it executed.

Identifiers are stable. They are also cited from the code and the tests that
discharge them (`// REQ: FEEL-001` and similar), which is how CodeWitness maps a
test or a file to a requirement.

Conventions used by every requirement:

- "must" is normative; "may" is permitted, not required.
- A value named `null` is the FEEL null (`Value::Null`), not the Rust `None`.
- "the engine crate" means the `engine` library crate at `engine/`.
- Where a requirement names a DMN clause, the DMN 1.5 specification is normative
  for anything the paragraph does not restate.

Run:

```bash
codewitness verify engine --requirements specs/requirements.md --in-place
```

---

## Engine: execution

# ENG-001 Rules execute to a fixpoint

An execution evaluates the loaded rule set against working memory repeatedly until
a full pass fires no rule. During one execution each rule fires at most once, so a
rule that updates a fact cannot loop forever by re-matching its own update. The
number of passes is bounded (default 1000; overridable per engine with
`ExecutionEngine::set_config("max_iterations", …)` or globally with the
`RRULES_MAX_CHAIN_ITERATIONS` environment variable); reaching the bound ends the
execution normally rather than panicking. The result reports how many rules fired,
which rule ids fired, and the post-execution working memory.

# ENG-002 Engines are addressable by name through one registry

The four Rete-family implementations (Rete, TREAT, LEAPS, PHREAK) are available
through a single `ExecutionEngine` trait and are addressable by the names `rete`,
`treat`, `leaps` and `phreak`. `EngineRegistry::available_engines()` must report
exactly the engines that are compiled in, and constructing an engine by an unknown
name must return an error rather than a default. All four implementations must pass
the same functional tests regardless of algorithm, and each must be constructed
fresh per execution rather than shared mutably.

# ENG-003 Retraction removes a fact and its partial matches

`retract_fact(id)` removes the fact from working memory, from alpha memory and from
every beta-memory partial match that references it, and returns `true`. Retracting
an unknown id, or the same id twice, returns `false` and changes no other state.
Facts inserted after a retraction must not join against the retracted fact.

# ENG-004 Conditions compare typed values predictably

A `Pattern` condition matches when a fact of the named type exists whose named field
compares true under the condition's `ComparisonOp` (`Eq`, `Ne`, `Lt`, `Le`, `Gt`,
`Ge`). Integer operands compare numerically, floating-point operands compare
numerically, strings compare lexicographically and booleans compare by value. A
condition whose operands are not comparable must evaluate to `false` rather than
panic, and a missing field must not match.

# ENG-005 Actions mutate working memory deterministically

`UpdateFact` applies each expression to the addressed fact's fields in one atomic
step; `InsertFact` adds a new fact; `RetractFact` removes one. Applying an action to
a fact that no longer exists must not panic and must not resurrect the fact. After
all firing completes, the working memory exposed by `get_facts()` reflects every
applied action.

# ENG-006 Salience determines firing order

Rules with higher `salience` fire before rules with lower salience within one pass.
Rules with equal salience fire in a deterministic order for a given rule set, so the
same inputs produce the same result on every run. Salience outside the documented
range (-1000…1000) is rejected by `Rule::validate` instead of silently reordering
the agenda. `lock_on_active` closes a rule's agenda group for the rest of the pass
in which it fires, so the group's other rules are considered on the next pass;
`no_loop` is satisfied by the once-per-rule fixpoint (a rule never fires twice in
one execution).

# ENG-007 Firing is recorded once per rule per execution

Coverage tracking records each rule's evaluation and firing exactly once per
execution, regardless of how many times the condition was re-evaluated during
fixpoint iteration. Dead-rule detection reads the same record: a rule that was
evaluated but never fired is reported as dead, and the dead-rule view, the coverage
view and the execution result agree on the fired count for the same execution.

## DMN

# DMN-001 The parser accepts DMN 1.1–1.5 and rejects DOCTYPE

The DMN XML parser accepts documents in the DMN 1.1, 1.3 and 1.5 namespaces,
preserves element ids, annotations and DMNDI diagram metadata in the canonical IR,
and rejects any document containing a DOCTYPE declaration (XXE defence) with an
error rather than a partially populated model. Malformed XML must produce an error
value, never a panic.

# DMN-002 Export round-trips through the canonical IR

For every document the parser accepts, exporting the parsed model to XML and parsing
that output again must produce an equal canonical IR
(`parse(export(parse(x))) == parse(x)`). Round-trip equality is structural: it does
not depend on attribute order, whitespace, or CDATA versus escaped text.

# DMN-003 Decision tables implement every hit policy

A decision table evaluates all matching rules and resolves them by its declared hit
policy: `UNIQUE` (exactly one match, otherwise an error), `ANY` (all matches agree),
`PRIORITY` (highest-priority output wins), `FIRST` (first match in rule order),
`RULE ORDER` (all matches in rule order), `OUTPUT ORDER` (all matches sorted by
output value) and `COLLECT` (aggregated with `SUM`, `MIN`, `MAX` or `COUNT`).
`COLLECT` with no aggregator returns the unaggregated list. A table with no matching
rule returns `null`.

# DMN-004 Input entries obey three-valued matching

An input entry such as `> 5`, `[1..10]`, `"a", "b"` or `-` is evaluated as a FEEL
unary test against the input expression's value. A unary test that evaluates to
`null` (including any test applied to a `null` input) does not match; only an
exactly-true test matches. `-` always matches.

# DMN-005 DRD evaluation respects dependencies and rejects cycles

Evaluating a decision resolves its required decisions first, in dependency order, and
makes their outputs available as inputs to dependent decisions. Independent
decisions in the same dependency level may be evaluated in parallel; the result must
be identical to sequential evaluation. A dependency cycle must be reported as an
error and must not hang or recurse without bound.

# DMN-006 Decision evaluation reports outputs and fired rules

`DecisionEvaluator::evaluate` returns a map of output name to value for the
evaluated decision, the number of rules that fired, and the number of decisions
resolved in the DRD chain. An unknown decision id returns `Err`. Requesting a trace
adds ordered trace events and leaves the result values unchanged.

## FEEL

# FEEL-001 Numbers are exact decimal values

FEEL numeric arithmetic uses exact decimal semantics, not binary floating point:
`0.1 + 0.2 = 0.3` must evaluate to `true`, and a division that does not terminate
must round to the context's decimal scale rather than to a binary approximation.
Integer and decimal operands compare by numeric value, so `1 = 1.0` is `true`.

# FEEL-002 There is no implicit conversion between numbers and strings

The only implicit conversions are the three the specification defines: singleton
value to singleton list, singleton list to value, and type conformance. Therefore
`1 + "2"` must evaluate to `null`, and `"1" = 1` must be `false` rather than a
coerced comparison.

# FEEL-003 Null propagates through arithmetic, comparison and boolean logic

Any arithmetic operator or built-in function applied to a `null` operand returns
`null` rather than throwing or coercing, and ordered comparisons involving `null`
(`1 < null`) are `null`. Equality treats `null` as an ordinary value — `null = null`
is `true` and `null = 1` is `false` — as the vendored OMG DMN TCK vector
`0068-feel-equality` requires. Boolean `and`/`or`/`not` follow three-valued truth
tables, including `true or null` → `true`, `false and null` → `false` and
`not(null)` → `null`.

# FEEL-004 Multi-word built-in function names are callable

Built-in functions whose names contain spaces — `upper case`, `lower case`,
`string length`, `string join`, `list contains`, `substring before`,
`substring after`, `starts with`, `ends with`, `get value`, `get entries`,
`date and time`, `years and months duration` and the `round ...` family — must be
parseable as a single callee and must evaluate to their specified result. Operator
words such as `and`, `or` and `in` must never be merged into a function name.

# FEEL-005 Rounding functions honour their scale argument

`round up`, `round down`, `round half up` and `round half down` each take an optional
second argument giving the number of decimal places, defaulting to 0. Rounding
happens at that scale, with ties resolved away from zero for the "up" variants and
toward zero for the "down" variants: `round half up(1.25, 1)` is `1.3`,
`round half down(1.25, 1)` is `1.2` and `round up(1.121, 2)` is `1.13`. A null or
non-integral scale yields `null`.

# FEEL-006 sort applies a supplied comparator

`sort(list)` returns the list in ascending order. `sort(list, function(x, y))`
returns the list ordered by the comparator: a negative or `true` result places `x`
before `y`, a positive or `false` result places `y` before `x`. Sorting an empty
list returns an empty list, and a null list returns `null`.

# FEEL-007 get entries returns key/value contexts

`get entries(context)` returns a list of contexts, each with the entries `key` (a
string) and `value` (the corresponding value). The order is deterministic (sorted by
key). `get entries(null)` is `null`.

# FEEL-008 Duration between two dates is computable

`years and months duration(from, to)` returns the whole-month duration between two
date or date-and-time values, negative when `to` precedes `from`. The single-string
form `years and months duration("P1Y2M")` returns the corresponding duration.

# FEEL-009 Built-in functions accept named arguments

A built-in function may be invoked with named arguments using the parameter names
the DMN specification gives (`abs(n: -1)`, `round half up(n: 1.25, scale: 1)`,
`number(from: "1.000,5", grouping separator: ".", decimal separator: ",")`). A name
that is not a parameter of that function makes the call evaluate to `null`.

# FEEL-010 abs is defined for durations

`abs(n)` returns the absolute value of a number, and the absolute value of a
duration: `abs(duration("-P1D"))` is a one-day duration and
`abs(duration("-P1Y"))` is a one-year duration. `abs(null)` is `null`.

# FEEL-011 Temporal values keep their zone and their DST semantics

The five temporal types (date, time, date-and-time, days-and-time duration,
years-and-months duration) round-trip through their ISO 8601 lexical forms. A
date-and-time carrying an IANA zone keeps that zone through duration arithmetic, and
adding a duration that lands in a daylight-saving gap or overlap resolves
deterministically (gap → `null`, overlap → the earlier instant) rather than shifting
silently.

# FEEL-012 The parser is bounded and never panics

The FEEL parser enforces a maximum token count and a maximum recursion depth and
reports both as parse errors in the returned AST (an `Err` node) instead of panicking
or exhausting the stack. Parsing arbitrary malformed text must terminate and return
a value.

## HTTP API

# API-001 Every business route requires a valid JWT

Except for an explicit public allowlist (`/v1/auth/login`, `/health`, `/swagger-ui`,
`/api-docs`, `/metrics`, `/v1/metrics/ws`), every request under `/v1` without a valid
`Authorization: Bearer <token>` receives `401`. Tokens are HS256 with an explicit
algorithm check, so a token signed with another algorithm or a wrong secret is
rejected.

# API-002 Execution rejects invalid input with a client error

`POST /v1/execute` returns `400` for an unknown engine name or an unparseable request
body, and never `500` for user input. A successful execution returns the real fired
rule count and the post-execution working memory, not the echoed request facts.

# API-003 Rules are isolated per tenant

Rule create/list/get/update/delete are scoped by the `X-Tenant-ID` header, defaulting
to `default` when absent. A rule created under one tenant id is invisible to another
tenant id: listing returns only the caller's rules, and fetching another tenant's
rule id returns `404`.

# API-004 Tenants have a complete CRUD contract

`POST /v1/tenants` creates a tenant (`201`) and rejects a blank name (`400`) and a
duplicate name (`409`). `GET /v1/tenants` lists tenants. `GET /v1/tenants/{id}`
returns `200` or `404`. `DELETE /v1/tenants/{id}` returns `204` when it deletes and
`404` when the id is unknown, including when deleted twice.

# API-005 Decisions are managed and evaluated over HTTP

The `/v1/decisions` resource supports list, create, get, update, delete, import,
export, IR inspection, hot reload, and `POST /v1/decisions/{id}/evaluate`, which
returns the decision's outputs, the number of rules fired and an optional trace.
Evaluating an unknown decision returns a client error.

# API-006 Reload is tracked by a job with explicit states

`POST /v1/rules/reload` swaps the shared rule set atomically and returns the new
generation; with `async_reload: true` it returns `202` and a job id, and
`GET /v1/jobs/{id}` reports that job's state transitions
(`pending` → `running` → `completed`/`failed`). A reload must never expose a
partially loaded rule set to a concurrent execution.

# API-007 The three metric surfaces agree

For the same execution, the Prometheus `/metrics` counters, the coverage/dead-rule
JSON endpoints and the `MetricsSnapshot` broadcast must report the same fired-rule
count and the same active-session count.

## System

# SYS-001 The engine crate builds and tests standalone

`cargo build -p engine` and `cargo test -p engine` must succeed without extra
feature flags, so the crate's own test suite exercises the real engines. Feature
flags may exist to trim the build, but the default must be the full engine surface.

# SYS-002 Untrusted input never panics the server

Malformed FEEL expressions, malformed DMN documents and malformed JSON bodies must
produce an error path (a parse error, a `null` value, a `4xx` response) and must
never abort the process, panic a request worker, or leave shared state inconsistent.

# SYS-003 Global singletons are test-isolated

Process-wide state (rule-execution tracking, coverage tracking, decision tracking)
must be resettable so that tests observing it are independent of test order and of
parallel execution.
