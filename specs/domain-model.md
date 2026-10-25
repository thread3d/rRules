# Domain model

This document describes the data model of the rRules engine crate (`engine/`) as the code actually
implements it: the `Value` algebra, facts and working memory, the rule AST and its evaluation
semantics, the FEEL type lattice and explicit coercions, the five temporal carriers with their
lexical forms and DST policy, and the validators. Every non-obvious statement cites `path:line`;
behaviour that is stubbed, dead, or self-inconsistent is confined to `## Known gaps` rather than
described as if it worked. Unless stated otherwise, "the evaluator" means the direct rule
interpreter in `engine/src/engine/direct_eval.rs` plus the inlined copy in
`engine/src/engine/rete/mod.rs`.

Two facts frame everything below: `Rule` and `Condition` live on the *Rete path* (legacy `Integer`
/ `Float` / `String` / `Bool` / `Null` values) and the 8 FEEL variants added in Phase 30 are
explicitly non-Rete (`engine/src/data_types.rs:182-193`, `engine/src/rules.rs:353-359`); and the
engine keeps facts in a bare `HashMap<String, Fact>`, not in `WorkingMemory`
(`engine/src/engine/rete/mod.rs:18,230`).

## Value

`Value` is a 16-variant enum — 8 legacy Rete variants and 8 FEEL variants — deriving
`Debug, Clone, PartialEq, Serialize, Deserialize` but **not** `Eq` and **no** `Ord`/`PartialOrd`
(`engine/src/data_types.rs:15-58`). Because `Ord` is absent, all ordering goes through the free
function `compare_values` (`engine/src/rules.rs:388-436`).

| Variant | Rust payload | FEEL type it denotes | null-like | def |
|---|---|---|---|---|
| `Bool(bool)` | `bool` | `boolean` | no | `data_types.rs:18` |
| `Integer(i64)` | `i64` | `number` | no | `data_types.rs:20` |
| `Float(f64)` | `f64` | `number` | no (NaN possible) | `data_types.rs:22` |
| `String(String)` | `String` | `string` | no | `data_types.rs:24` |
| `DateTime(NaiveDateTime)` | `chrono::NaiveDateTime` | `date and time` (offset-less, legacy) | no | `data_types.rs:27` |
| `Object(HashMap<String, Value>)` | `HashMap<String, Value>` | `context` | no | `data_types.rs:29` |
| `Array(Vec<Value>)` | `Vec<Value>` | `list<T>` | no | `data_types.rs:31` |
| `Null` | — | `null` | **yes** | `data_types.rs:33` |
| `Number(Decimal)` | `rust_decimal::Decimal` (Decimal128) | `number` | no | `data_types.rs:38` |
| `Date(NaiveDate)` | `chrono::NaiveDate` | `date` | no | `data_types.rs:40` |
| `Time(TimeValue)` | `temporal::TimeValue` | `time` | no | `data_types.rs:42` |
| `DateTimeTz(DateTimeValue)` | `temporal::DateTimeValue` | `date and time` | no | `data_types.rs:46` |
| `DaysTimeDuration(TimeDelta)` | `chrono::TimeDelta` | `days and time duration` | no | `data_types.rs:48` |
| `YearsMonthsDuration(YearsMonthsDuration)` | `{ months: i64 }` | `years and months duration` | no | `data_types.rs:50` |
| `Range(Box<FeelRange>)` | boxed `FeelRange` | `range<T>` | no | `data_types.rs:54` |
| `Function(FeelFunction)` | `FeelFunction` | `function` | no | `data_types.rs:57` |

`Null` is the only null-like variant. Absence elsewhere is modeled with `Option`
(`FeelRange::start`/`end` at `engine/src/dmn/feel/types.rs:90,92`;
`FeelFunction::body`/`captured` at `types.rs:146,152`) or by an operator *returning* `Value::Null`
on a type error (`engine/src/rules.rs:320,334,344,372`). Two `Box`/`Arc` indirections break size
cycles: `Value::Range(Box<FeelRange>)` (`data_types.rs:51-54`) and
`FeelFunction::body: Option<Arc<FeelExpression>>` (`types.rs:112-119`). JSON shape is serde's
default external tagging (`{"Integer":42}`, `"Null"`), hence rule files are JSON ASTs
(`api/src/v1/execution.rs:150-157`).

### Numeric representation

Three numeric carriers coexist and are **not** unified:

| Carrier | Rust type | Produced by | Ordered comparison |
|---|---|---|---|
| `Integer` | `i64` | Rete literals, `numeric_op` integer arm | i64 (`rules.rs:401-409`) |
| `Float` | `f64` | Rete literals, `numeric_op` float arm | f64 (`rules.rs:411-421`) |
| `Number` | `Decimal` (Decimal128) | FEEL evaluator, `coerce_to_number` | **never** (falls through to `false`) |

Rules the code enforces:

- `Integer + Integer` stays `Integer`; any `Float` operand promotes both sides to `f64`; anything
  else (including `Number`) yields `Value::Null` (`rules.rs:360-374`).
- Integer division truncates: `Value::Integer(a / b)` for non-zero `b`; the float path yields
  `Null` when the divisor is `0.0` or NaN (`rules.rs:309-321`).
- `Div`, `Add`, `Sub`, `Mul` use unchecked `i64`/`f64` arithmetic — no `checked_*` and no overflow
  guard.
- `DynamicCast::as_integer` silently truncates `Float` (`*f as i64`) and rejects `Number`; `as_float`
  accepts only `Integer`/`Float` (`engine/src/data_types.rs:142-156`). Therefore FEEL `Number`
  values are invisible to every Rete helper.
- `coerce_to_number` accepts `Number` and `Integer` only — `String`, `Bool`, `Null` → `None`, and
  notably `Float` → `None` (`engine/src/dmn/feel/coercion.rs:39-46`).

### Equality and ordering implemented by `compare_values`

`compare_values(op, left, right)` is the single comparison surface shared by
`Condition::evaluate` and `Expression::Compare` (`rules.rs:376-388`):

| Operator | Operand pair | Implementation | `rules.rs` |
|---|---|---|---|
| `Eq` | any / any | derived `PartialEq` (`left == right`), variant-wise | 391 |
| `Ne` | any / any | `left != right` | 392 |
| `Lt`/`Le`/`Gt`/`Ge` | `Integer` / `Integer` | `i64` comparison | 401-409 |
| `Lt`/`Le`/`Gt`/`Ge` | `Integer`↔`Float`, `Float`/`Float` | both `as_float` → `f64` | 411-421 |
| `Lt`/`Le`/`Gt`/`Ge` | `String` / `String` | lexicographic (`str` `Ord`) | 423-431 |
| `Lt`/`Le`/`Gt`/`Ge` | every other pair | `false`, never panics | 432-433 |

Consequences (all directly checkable in the arms above):

- Cross-variant equality is always false: `Integer(1)` vs `Float(1.0)` → `Eq` false, `Ne` true.
- `Number(1) < Number(2)` is **false**: `as_float` returns `None` for `Number`
  (`data_types.rs:150-156`), so Decimal never reaches an ordering arm.
- `Null` equals only `Null`; every ordered op involving `Null` returns `false`.
- `Float(NAN)` is not equal to itself (asserted at `data_types.rs:330-337`), so `Eq` is false and
  `Ne` true.
- `Bool`, `Object`, `Array`, `Date`, `Time`, `DateTimeTz`, both durations, `Range` and `Function`
  support `Eq`/`Ne` (via `PartialEq`, so chrono date-times compare by instant) but every ordered op
  returns `false`.

## Facts and working memory

### Fact identity and payload

| Aspect | Implementation | def |
|---|---|---|
| Identity | `id: Uuid`, generated by `Uuid::new_v4()` in `Fact::new` (random v4 — not sequential, not monotonic, not time-ordered) | `engine/src/fact.rs:16,29` |
| Explicit id | `Fact::with_id` accepts a caller id (tests, reloads) | `fact.rs:37-48` |
| Type tag | `fact_type_id: String`, free-form, not validated against any registry | `fact.rs:18` |
| Payload | `payload: HashMap<String, Value>`, untyped per fact | `fact.rs:20` |
| Timestamp | `created_at: SystemTime::now()` at construction; no clock seam, not part of identity, not compared anywhere | `fact.rs:22,32,46` |
| Field access | `get(&str) -> Option<&Value>`, `update_field(name, Value)` | `fact.rs:51-58` |
| Schema types | `FactType { name, typed_fields: HashMap<String, ValueType>, dynamic_fields }` and `ValueType` are declared but referenced by no other code path (only re-exported at `engine/src/lib.rs:65`) | `data_types.rs:74-93` |

### Two working-memory implementations

1. `WorkingMemory` (`fact.rs:141-149`) — `HashMap<Uuid, Fact>` plus a `FactRetrievalIndex` and an
   unused `changes: Vec<WorkingMemoryChange>` (`fact.rs:147-156`, both `#[allow(dead_code)]`).
   Owned by `environment::Engine` (`engine/src/environment.rs:82`) and used by the testing
   snapshots (`engine/src/testing/mod.rs:143-148`).
2. `HashMap<String, Fact>` keyed by `fact.id.to_string()` inside each executor, with
   `pub type FactId = String` (`engine/src/engine/rete/mod.rs:18,230`;
   `engine/src/engine/direct_eval.rs:69,193`). This is the store the running engines mutate.

`WorkingMemory` operations and their exact semantics:

| Operation | Semantics | def |
|---|---|---|
| `insert_fact` | Rejects an empty payload unless `fact_type_id == "generic"`; rejects a duplicate id; on success adds to index and map | `fact.rs:169-183` |
| `update_fact` | Merges the update map into the payload, then removes and re-adds the index entry; unknown id → `FactNotFound` | `fact.rs:186-205` |
| `retract_fact` | Returns `Ok(Some(fact))` on removal, `Ok(None)` when the id is unknown (not an error), and removes index entries | `fact.rs:208-216` |
| `get_fact` | By `Uuid` | `fact.rs:219-221` |
| `get_facts_by_type` | Resolves ids through the primary index, skips ids missing from the map | `fact.rs:224-229` |
| `get_all_facts` | `facts.values().collect()` | `fact.rs:232-234` |
| `match_pattern` | Stub: always returns an empty `Vec` | `fact.rs:237-240` |
| Errors | `EmptyPayload`, `DuplicateId(Uuid)`, `FactNotFound(Uuid)`, `Validation(String)` | `fact.rs:243-253` |

The index keys secondary entries by `(field_name, format!("{:?}", value))` (`fact.rs:91-96`) — a
`Debug`-string proxy for the value, so index membership follows `Debug` formatting, not `PartialEq`.

The executor store has different semantics: `assert_fact` inserts with `HashMap::insert`, silently
overwriting an existing key and skipping both the empty-payload and duplicate-id guards
(`engine/src/engine/rete/mod.rs:274-296`); `ExecutionEngine::retract_fact` returns `Ok(false)` for
an unknown id (`rete/mod.rs:580-592`).

### Ordering guarantees

None. Every read path iterates a `HashMap` or a `HashSet`-backed index and then `collect`s:
`get_all_facts` (`fact.rs:232-234`), `FactRetrievalIndex::get_by_type`
(`fact.rs:128-135`), the evaluator's fact snapshot (`direct_eval.rs:106`,
`rete/mod.rs:412`), and `get_facts` for the API (`rete/mod.rs:637-639`). Consequently these
"first match" resolutions are order-dependent on hash iteration:

- `Expression::FieldRef` reads the **first** fact whose `fact_type_id` matches
  (`rules.rs:274-282`).
- `Action::UpdateFact` targets the exact id key, else the **first** fact of that `fact_type_id`
  (`direct_eval.rs:201-218`).
- `Action::RetractFact` removes the exact id key, else the **first** fact of that `fact_type_id`
  (`direct_eval.rs:228-240`).

## Rules

### Shapes

`Rule` derives `Debug, Clone, PartialEq, Serialize, Deserialize` (`engine/src/rules.rs:15-31`):

| Field | Type | Default (`Rule::new`) | Consumed by an evaluator? |
|---|---|---|---|
| `id` | `String` | caller | yes (fired-set guard, coverage) |
| `name` | `String` | caller | yes (execution tracker labels) |
| `salience` | `i32` | `0` | **no** |
| `no_loop` | `bool` | `false` | **no** |
| `lock_on_active` | `bool` | `false` | **no** |
| `condition` | `Condition` | `Condition::True` | yes |
| `actions` | `Vec<Action>` | empty | yes |

`RuleBuilder` exposes the same fields fluently (`rules.rs:66-110`). `Rule::validate` checks only
that `id` and `name` are non-empty (`rules.rs:48-57`); despite the doc comment `-1000 to 1000`
(`rules.rs:21`) and the `RuleValidationError::InvalidSalience` variant (`rules.rs:613`), the range is
never checked.

`RuleSet { pub rules: Vec<Rule>, compiled: bool }` (`rules.rs:476-481`) has no serde derive.
`add_rule` validates then rejects duplicate ids (`rules.rs:493-504`); `validate_all` collects
per-rule errors into `RuleValidationResult { errors, warnings }` and returns `Ok(warnings)` when
there are none (`rules.rs:515-530`); `compile` only sets `compiled = true` (`rules.rs:533-540`).

### Condition

```text
Condition = True | False
          | And(Vec<Condition>) | Or(Vec<Condition>) | Not(Box<Condition>)
          | Compare { left: Box<Condition>, op: ComparisonOp, right: Box<Condition> }
          | Pattern { fact_type: String, field: String, value: Value, op: ComparisonOp }
```
(`rules.rs:117-144`)

Evaluation, `Condition::evaluate(&[&Fact]) -> bool` (`rules.rs:168-208`):

| Variant | Semantics | `rules.rs` |
|---|---|---|
| `True` / `False` | constant | 170-171 |
| `And(cs)` | `iter().all(...)` — empty vector is `true` | 172 |
| `Or(cs)` | `iter().any(...)` — empty vector is `false` | 173 |
| `Not(c)` | negation of the single child | 174 |
| `Compare` | resolves **both** sides through `to_value`; if either is `None` the result is `false`; otherwise `compare_values(op, l, r)` | 175-185 |
| `Pattern` | existentially quantifies over facts of `fact_type`: true when at least one such fact has `field` present and `compare_values(op, field_value, value)` is true; false when no fact of that type exists or the field is absent | 186-206 |

`to_value` returns `Some` only for `True`/`False` (as `Value::Bool`) and `None` for everything else
(`rules.rs:211-217`). So `Compare` is a boolean-leaf comparison only: `Compare { left: And(..), .. }`
or any nested `Pattern`/`Compare`/`Not` operand evaluates to `false`, not to a nested truth value.
`Pattern` is existential, not universal: one matching fact satisfies it, and a type with no matching
fact yields `false` (test at `rules.rs:1184-1195`). A matching fact missing the field also yields
`false` because `Option::map` short-circuits to `unwrap_or(false)` (`rules.rs:200-205`, test at
`rules.rs:1198-1209`).

### ComparisonOp

`ComparisonOp = Eq | Ne | Lt | Le | Gt | Ge`, `Copy + Eq + Serialize + Deserialize`
(`rules.rs:147-155`). Per-operand behaviour is exactly the `compare_values` matrix above; there is
no type-directed dispatch beyond "Integer/Integer → i64, else both-floats → f64, else both-strings
→ lexicographic, else false". The enum implements no `Default`; the `Pattern::op` doc comment records
that `Eq` is the operator used at construction sites which do not specify one (`rules.rs:139-142`).

### Action

```text
Action = InsertFact { fact_type: String, fields: HashMap<String, Value> }
       | UpdateFact { id: String, fields: HashMap<String, Expression> }
       | RetractFact { id: String }
       | CallFunction { name: String, args: Vec<Value> }
       | LogMessage { level: LogLevel, message: String }
```
(`rules.rs:443-463`; `LogLevel = Debug | Info | Warning | Error`, `rules.rs:466-472`)

Mutation is performed by `apply_action(action, &mut HashMap<String, Fact>)`
(`engine/src/engine/direct_eval.rs:193-248`):

| Action | Effect | `direct_eval.rs` |
|---|---|---|
| `UpdateFact` | Snapshots (clones) all facts, resolves the target (exact key, else first fact of that type; no match → no-op), then evaluates each field `Expression` against the **pre-action** snapshot and inserts the result into the payload | 195-223 |
| `InsertFact` | `Fact::new(fact_type, fields.clone())` → fresh v4 UUID → inserted under `id.to_string()`; bypasses `WorkingMemory`'s empty-payload and duplicate-id guards | 224-227 |
| `RetractFact` | Removes the exact key, else the first fact whose `fact_type_id == id` | 228-240 |
| `CallFunction` | No-op | 241-243 |
| `LogMessage` | No-op | 244-246 |

`Expression` (`rules.rs:231-259`) is the RHS language: `Literal(Value)`, `FieldRef { fact_type,
field }`, `Add`/`Sub`/`Mul`/`Div`, `StringConcat`, `Compare { op, left, right }`, and
`Call { name, args }`. `Expression::evaluate` (`rules.rs:270-347`) resolves `FieldRef` to the first
fact of the type or `Value::Null` when absent, applies `numeric_op` for arithmetic, requires both
`String` operands for `StringConcat` (`rules.rs:332-335`), returns `Value::Bool` for `Compare`, and
returns `Value::Null` for `Call` (stub, `rules.rs:342-345`).

### Firing order, salience, and loop control

The evaluator is a fixpoint loop over `rules` in **load order** — `RuleSet.rules` is cloned
verbatim by `load_ruleset` (`rete/mod.rs:630-633`) and iterated by index
(`direct_eval.rs:100`, `rete/mod.rs:406`). Each rule fires at most once per `execute()` call, guarded
by a `fired_set` of rule ids (`direct_eval.rs:102-104,130`; `rete/mod.rs` mirrors this). The loop
stops when a full pass fires nothing new (`direct_eval.rs:134-136`).

`salience`, `no_loop` and `lock_on_active` are carried in the AST and exposed through the builder,
but no evaluator reads them: `salience` is consumed only by the standalone
`Agenda`/`ConflictResolver` (`engine/src/salience/agenda.rs:58-62`,
`engine/src/salience/conflict_resolution.rs:47-50`), which is exported (`engine/src/lib.rs:61-62`)
and exercised by tests but called by no engine or API handler. Firing order is therefore not
salience-ordered. `RRULES_MAX_CHAIN_ITERATIONS` (default `1000`) caps the loop; see
`## Invariants and limits`.

## FEEL type lattice

`FeelType` is a closed 14-node enum (`engine/src/dmn/feel/types.rs:41-75`): `Null`, `Any`, `Number`,
`String`, `Boolean`, `Date`, `Time`, `DateTime`, `DaysTimeDuration`, `YearsMonthsDuration`,
`List(Box<FeelType>)`, `Context(Vec<(String, FeelType)>)`, `Range(Box<FeelType>)`, and
`Function { params: Vec<FeelType>, ret: Box<FeelType> }`. The relations are free functions, not a
trait graph (`types.rs:18-20`).

### Equivalence `≡`

`equivalent(a, b)` is structural equality (`a == b`) — reflexive, symmetric, transitive
(`types.rs:199-218`). At the type level `Date ≡ DateTime` deliberately does **not** hold, so that
`conforms(DateTime, Date)` cannot become true via equivalence-implies-conformance (`types.rs:204-216`).

### Conformance `<:`

`conforms(a, b)` is matched top-down in this order (`types.rs:238-268`):

| Rule | Result | `types.rs` |
|---|---|---|
| `b == Any` | `true` for every `a` | 243 |
| `equivalent(a, b)` | `true` (placed before the `Null` arm so `Null <: Null` holds) | 246 |
| `b == Null` (and `a != Null`) | `false` — nothing else conforms to `Null` | 248 |
| `Date <: DateTime` | `true`; reverse is `false` | 250 |
| `List(t) <: List(s)` | `conforms(t, s)` | 252 |
| `Range(t) <: Range(s)` | `conforms(t, s)` | 254 |
| `Context(ae) <: Context(be)` | `ae.len() >= be.len()` and each target entry has exactly one source entry with the same key and a conforming type | 257, 276-288 |
| `Function{(ap,ar)} <: Function{(bp,br)}` | `ap.len() == bp.len() && ∀i. conforms(bp_i, ap_i) && conforms(ar, br)` — contravariant parameters, covariant return | 261-265 |
| otherwise | `false` | 266 |

Property tests assert reflexivity, transitivity, `Null <: Any`, "`Null` conforms only to `Any` (or
itself)", and equivalence⇒conformance (`types.rs:505-560`).

### Implicit conversions (exactly three) and the rejected ones

`implicit_convert(Value, &FeelType, ConvertCtx) -> Option<Value>` (`types.rs:298-315`) implements
only DMN §10.3.2.9.4:

| `ConvertCtx` | Conversion | Guard | `types.rs` |
|---|---|---|---|
| `Filter` | to singleton list: `T → List<T>` | target must be `List(inner)` and `type_of(v) <: inner`; result `Value::Array([v])` | 323-328 |
| `Invocation` | from singleton list: `[T] → T` | value must be a 1-element `Value::Array` whose element conforms to the target | 335-347 |
| `Binding` | conforms-to | `conforms(type_of(v), target)` else `None` | 307-313 |

Deliberately rejected:

- **`number ↔ string` in every context.** `conforms` is false for those pairings, so `Filter`,
  `Invocation` and `Binding` all return `None` (assertions at `types.rs:664-714`).
- The explicit coercion module repeats the prohibition for operator-level coercion:
  `coerce_to_string` accepts only `String`, `coerce_to_number` only `Number`/`Integer`,
  `coerce_to_bool` only `Bool` (`engine/src/dmn/feel/coercion.rs:39-72`) — so `1 + "2"` has no path
  to a value.

`type_of` maps every one of the 16 `Value` variants with no wildcard (`types.rs:354-395`); both
`Integer` and `Float` map to `Number` (`types.rs:363-364`), legacy `DateTime` and `DateTimeTz` both
map to `DateTime` (`types.rs:367,386`), an empty `Array` becomes `List(Any)` (`types.rs:373-380`),
an `Object` becomes a `Context` with one entry per key (`types.rs:368-372`), and a `Range`'s inner
type comes from the start endpoint, else the end, else `Any` (`types.rs:401-407`).
`resolve_type_ref` parses `feel:` type references and fails closed (`None`) on anything unrecognized
(`types.rs:417-445`).

### Null propagation

- `type_of(&Value::Null) == FeelType::Null` (`types.rs:381`), and `Null` conforms only to `Any` and
  itself (`types.rs:243-248`); a `Binding` conversion of a null to any concrete type therefore
  yields `None`, which the evaluator maps to `null`.
- Every `coerce_to_*` returns `None` for `Value::Null` (`coercion.rs:39-72`), so operators that
  depend on coercion produce `Value::Null` rather than a truth value (`rules.rs:320,334,372`).
- `Condition` treats a missing field as `false`, not as a three-valued unknown
  (`rules.rs:200-205`).

## Temporal domain

Five carriers, all reachable as `Value` variants:

| Carrier | Shape | `Value` variant | serde |
|---|---|---|---|
| `chrono::NaiveDate` | calendar date, no offset | `Date` | derived |
| `TimeValue` | one of `Fixed(NaiveTime)`, `Offset(DateTime<FixedOffset>)`, `Local(NaiveTime)` | `Time` | derived |
| `DateTimeValue` | one of `Fixed(DateTime<FixedOffset>)`, `Iana(DateTime<chrono_tz::Tz>)`, `Local(NaiveDateTime)` | `DateTimeTz` | manual, via `DateTimeValueRepr` |
| `chrono::TimeDelta` | fixed seconds | `DaysTimeDuration` | derived |
| `YearsMonthsDuration { months: i64 }` | `years*12 + months`, may be negative | `YearsMonthsDuration` | derived |

`TimeValue` is at `engine/src/dmn/feel/temporal.rs:52-60`, `DateTimeValue` at `temporal.rs:73-81`,
`YearsMonthsDuration` at `temporal.rs:163-167`. The IANA case cannot derive serde because
`DateTime<Tz>` is not `Deserialize` for arbitrary `Tz`, so it is serialized as
`{kind: "Iana", value: {naive, tz}}` and the zone round-trips as its IANA name
(`temporal.rs:33-42,88-149`). IANA is a separate variant precisely so the zone survives arithmetic
and re-emission instead of collapsing to a fixed offset (`temporal.rs:20-27,227-232`).

### Lexical forms

`parse_at_literal(inner)` parses the text inside `@"..."` and dispatches on the prefix/shape
(`temporal.rs:190-225`); any failure returns `None` (never panics):

| Input shape | Accepted form | Carrier produced | `temporal.rs` |
|---|---|---|---|
| starts `P` or `-P` | duration | see duration rows | 197-199 |
| contains `T` | `YYYY-MM-DDTHH:mm:ss`, optionally suffixed with a fixed offset or `@IANA/Zone` | `DateTimeTz` | 201-203, 426-464 |
| `:` before `-` (or no `-`) | `HH:mm:ss` or `HH:mm:ss±HH:mm` | `Time` | 206-216, 409-422 |
| `-` before `:` (or no `:`) | `YYYY-MM-DD`, year part ≤ 4 digits | `Date` | 218-224 |
| `[n]Y[n]M` only | years-months duration | `YearsMonthsDuration` | 297-318, 323-350 |
| any of `T`/`D`/`H`/`S` present | `[n]D T[n]H[n]M[n]S` | `DaysTimeDuration` | 297-318, 354-405 |

Fixed offsets are parsed by `parse_fixed_offset` (`temporal.rs:469-486`). Offset-bearing `time`
values are anchored to `1970-01-01` before the offset is attached (`temporal.rs:414-417`). Years
longer than 4 digits are rejected outright (`temporal.rs:218-223`). Seconds are mandatory in every
time and date-time form — the formats are a fixed `%H:%M:%S` / `%Y-%m-%dT%H:%M:%S`
(`temporal.rs:413,420,431,453,462`) — and duration fields are integer-only
(`temporal.rs:354-405`), so fractional seconds are not accepted.

`format_value(&Value)` is the inverse and returns `None` for non-temporal variants
(`temporal.rs:235-245`), emitting:

| Carrier | Emitted | `temporal.rs` |
|---|---|---|
| `Date` | `%Y-%m-%d` | 237 |
| `Time` `Fixed`/`Local` | `%H:%M:%S` (identical output for both) | 492 |
| `Time` `Offset` | `%H:%M:%S±HH:mm` | 493-497, 520-527 |
| `DateTimeTz` `Fixed` | `%Y-%m-%dT%H:%M:%S±HH:mm` | 503-507 |
| `DateTimeTz` `Iana` | `%Y-%m-%dT%H:%M:%S@Zone/Name` — zone name, never an offset | 508-515 |
| `DaysTimeDuration` | `P<n>DT<n>H<n>M<n>S`, `PT0S` for zero, leading `-` when negative | 529-564 |
| `YearsMonthsDuration` | `P<n>Y<n>M` (`P0M` when zero), leading `-` when negative | 566-583 |

### Timezone handling and DST policy

Three offset models are supported: a fixed offset (`FixedOffset`), an IANA zone
(`chrono_tz::Tz`), or none (`Local`/`Fixed` carriers store a naive value). Zoned arithmetic goes
through one helper, `reattach_zone` (`temporal.rs:590-596`), whose policy is:

| `LocalResult` | Result | Meaning |
|---|---|---|
| `Single(dt)` | `Some(dt)` | unambiguous local time |
| `Ambiguous(earliest, _)` | `Some(earliest)` | DST overlap resolves to the earliest occurrence |
| `None` | `None` | spring-forward gap → null |

Arithmetic actually implemented (both IANA-only):

- `add_months_zoned(DateTime<Tz>, &YearsMonthsDuration)` — adds/subtracts `chrono::Months` on the
  **naive local** component via `checked_add_months`/`checked_sub_months` (so Jan 31 + 1 month =
  Feb 28/29), then re-attaches the zone; `None` on overflow or DST gap (`temporal.rs:254-268`).
- `add_days_time_zoned(DateTime<Tz>, TimeDelta)` — `checked_add_signed` on the naive local
  component, then re-attaches the zone; `None` on overflow or DST gap (`temporal.rs:276-286`).
  Because the addition happens on the wall clock, crossing a transition shifts the wall-clock time,
  not the instant.

Deserialization of an IANA date-time uses the same overlap policy (ambiguity → earliest) but treats
a DST gap as a hard `serde` error rather than `null` (`temporal.rs:136-144`).

## Validation

### Result shape

`ValidationResult { is_valid: bool, errors: Vec<ValidationError>, warnings: Vec<ValidationWarning> }`
derives `Debug, Default` (`engine/src/validation.rs:10-18`). `ValidationResult::new()` starts
`is_valid = true` (`validation.rs:21-27`); `with_error` sets `is_valid = false` and pushes
(`validation.rs:29-33`); `with_warning` pushes only (`validation.rs:35-38`); `extend` merges and
clears validity if the other side had errors (`validation.rs:41-47`). Errors and warnings are
`{ message: String, location: Option<String> }` with `Display` rendering `"{message} at {location}"`
when a location exists (`validation.rs:51-84`). Neither type nor `ValidationResult` derives
`Serialize`.

### SyntaxValidator

- `validate_expression(&str)` — balance of `(`/`)` and of `[`/`]` only. An unexpected closer yields
  error "Unexpected closing parenthesis/bracket" with `location: Some("position {i}")`; an unclosed
  opener yields "Unclosed …" with `location: None` (`validation.rs:96-148`).
- `validate_operator_sequence(&[&str])` — membership in the fixed whitelist
  `["==", "!=", "<", "<=", ">", ">=", "&&", "||", "!"]`; anything else is an error with a positional
  location (`validation.rs:151-165`).
- Nothing in the module calls either method (`## Known gaps`).

### SemanticValidator

Constructed from `fact_types: HashMap<String, Vec<String>>` (fact type → known field names) and an
optional list of known function names (`validation.rs:175-197`).

| Check | Outcome | `validation.rs` |
|---|---|---|
| Unknown fact type in `Pattern` | **error** `Unknown fact type '{t}'` | 234-239 |
| Unknown field in a known type | **warning** `Unknown field '{f}' in fact type '{t}'` | 242-252 |
| Field value vs declared type | no-op (all arms empty) | 280-309 |
| `Compare` between incompatible types | warning "Comparison between incompatible types" | 213-226 |
| `CallFunction` with an unknown name | **warning** `Unknown function '{n}'` | 267-273 |
| Circular rule references | no-op | 351-358 |

`validate_rule_set` runs the circular-reference check and then validates every rule's condition and
actions (`validation.rs:333-348`). `types_compatible` accepts Integer↔Float, String/String,
Bool/Bool, anything involving `Null`, and otherwise requires identical discriminants
(`validation.rs:321-330`) — but it is only reached from `Compare`, whose operands resolve via
`condition_to_value`, which returns `Some` only for `True`/`False` leaves
(`validation.rs:312-318`), so in practice the comparison is always Boolean/Boolean.

`ValidationPipeline` chains syntax then semantics (`validation.rs:363-416`); its syntax stage
`validate_rule_syntax` always returns `Ok(())` (`validation.rs:413-415`), and
`with_fact_types` stores the semantic validator via `std::mem::transmute` to
`SemanticValidator<'static>` (`validation.rs:383-387`).

### Is validation used by the API?

No. No crate outside `engine/src/validation.rs` references `SyntaxValidator`,
`SemanticValidator`, `ValidationPipeline`, or `ValidationResult`. The API's rule-loading path is
`build_ruleset_from_dir` (`api/src/v1/execution.rs:168`), which deserializes JSON rule files and
calls `RuleSet::add_rule` — i.e. only `Rule::validate` (non-empty id and name) plus duplicate-id
rejection — logging a `tracing::warn!` and skipping the offending file/rule instead of failing
startup (`api/src/v1/execution.rs:197,210,217-219`). The API does perform HTTP-level request
validation (`api/src/v1/rules.rs:308`, `api/src/v1/execution.rs:31`), which is unrelated to this
module.

## Invariants and limits

Invariants enforced by the code:

- A fact with an empty payload is rejected unless its type is exactly `"generic"`
  (`engine/src/fact.rs:171-173`).
- A duplicate fact id is rejected by `WorkingMemory::insert_fact` (`fact.rs:176-178`); ids are
  v4 UUIDs from `Fact::new` (`fact.rs:29`).
- `Rule::validate` requires non-empty `id` and `name` (`engine/src/rules.rs:49-56`), and
  `RuleSet::add_rule` additionally rejects duplicate ids (`rules.rs:497-500`).
- `Pattern` never returns unconditionally true: a missing fact type and a missing field both yield
  `false` (`rules.rs:197-205`).
- Ordered comparison never panics; unknown pairs fall back to `false` (`rules.rs:432-433`).
- Division by zero, including `0.0` and NaN divisors, yields `Value::Null` (`rules.rs:310-321`);
  `FieldRef` on a missing type/field yields `Value::Null` (`rules.rs:274-282`).
- `conforms` is reflexive, `T <: Any` holds for every `T`, and `Null` conforms only to `Any` or
  itself (`engine/src/dmn/feel/types.rs:241-248`, property tests `types.rs:505-533`).
- `type_of` is total and exhaustive over all 16 `Value` variants, with no wildcard
  (`types.rs:354-395`).
- `resolve_type_ref` fails closed on unrecognized references (`types.rs:417-445`,
  `_ => None` at `types.rs:443`).
- `parse_at_literal` returns `None` on malformed input and never panics
  (`engine/src/dmn/feel/temporal.rs:190-225`); `reattach_zone` never unwraps `LocalResult`
  (`temporal.rs:590-596`).
- Session operations after `destroy()` return `Err` (`engine/src/environment.rs:176-183,186-189`)
  and `is_valid()` reports the flag (`environment.rs:200-202`).

Limits and guards (with defaults and configuration points):

| Limit | Default | Configured by | `path:line` |
|---|---|---|---|
| Chain/fixpoint iterations | `1000` | `RRULES_MAX_CHAIN_ITERATIONS` env var, read at each `execute` call | `engine/src/engine/direct_eval.rs:73-76`; `engine/src/engine/rete/mod.rs:378-381` |
| Same knob at API startup | warn only if unparseable | `api/src/main.rs:76-83` | |
| Rules fired per `execute` call | 1 per rule (fired-set guard) | code constant | `direct_eval.rs:102-104,130` |
| `EngineBuilder::max_iterations` | `1000` | builder method; stored but never read by execution | `engine/src/environment.rs:29,47-50,65` |
| FEEL lexer tokens | `10_000` | `Lexer::with_max_tokens` | `engine/src/dmn/feel/parser.rs:170-194,207-215` |
| FEEL parser recursion depth | `256` (`DEFAULT_MAX_DEPTH`) | internal constant; exceeding it yields an `Err` node | `parser.rs:1010-1012,1088-1122` |
| FEEL token-cache entries | `10_000` (FIFO eviction) | `DEFAULT_MAX_TOKEN_CACHE_ENTRIES` | `parser.rs:798,824` |
| FEEL AST-cache entries | `10_000` | `FeelAstCache::with_capacity` | `engine/src/dmn/feel/cache.rs:57,210` |
| Range materialization steps | `1_000_000` | `MAX_RANGE_ITERATIONS`; exceeding it yields `Null` | `engine/src/dmn/feel/eval.rs:1048,1076-1079` |
| Backward-trace exploration depth | `10` | `TraceExplorer::new` | `engine/src/backward_chaining.rs:109` |
| Inference-trace payload | `None` unless `collect_trace` is true **and** a rule fired | `set_collect_trace` | `direct_eval.rs:165-176`; `engine/src/engine/registry.rs:98` |

## Known gaps

- **Two disjoint working memories.** `WorkingMemory` (with its guards and index) is never used by
  any executing engine, which store facts in `HashMap<String, Fact>`
  (`engine/src/fact.rs:141-149` vs `engine/src/engine/rete/mod.rs:230`,
  `engine/src/engine/direct_eval.rs:69`). `Action::InsertFact` therefore bypasses the
  empty-payload and duplicate-id guards it appears to respect (`direct_eval.rs:224-227`), and
  `assert_fact` silently overwrites a same-id fact (`rete/mod.rs:279`).
- **~~`salience`, `no_loop`, `lock_on_active` are dead weight.~~** Closed 2026-10-25: both
  evaluators order passes by `salience` (higher first, ties in load order), `lock_on_active` closes
  the rule's `agenda_group` for the rest of the pass, and `no_loop` is satisfied by the
  once-per-rule fixpoint. `Agenda`/`ConflictResolver` remain exported but unused — the ordering
  logic is in `direct_eval` and `rete` rather than in those types.
- **~~`Rule::validate` does not enforce the documented salience range.~~** Closed 2026-10-25:
  `validate` returns `InvalidSalience` outside -1000…=1000. `RuleSet::compile` still only sets a
  flag.
- **~~Two max-chain-iteration knobs, only one live.~~** Partly closed 2026-10-25: each engine now
  honours `set_config("max_iterations", …)`, which overrides `RRULES_MAX_CHAIN_ITERATIONS`; the
  legacy `EngineBuilder` still stores its value without effect because the `Engine`/`Session`
  surface below is a placeholder.
- **The `Environment`/`Session` execution surface is a placeholder.** `Engine::load_rules` is a
  no-op, `Engine::execute_forward`/`execute_backward` return fixed messages
  (`environment.rs:122-147`), `Session::insert_fact` discards its argument (`environment.rs:176-183`),
  `Session::execute` returns "Not implemented" (`environment.rs:186-192`), and `Session` holds a
  raw `*const Engine` that is never dereferenced (`environment.rs:154,180-181`).
  `forward_chaining::execute_forward` and `backward_chaining::execute_backward` are stubs
  (`engine/src/forward_chaining.rs:46-57`, `engine/src/backward_chaining.rs:80-93`), even though
  `lib.rs` re-exports both as entry points (`engine/src/lib.rs:71-73`).
- **Validator module is unused by the API and partly unreachable.** `validate_rule_syntax` now
  real (it runs `Rule::validate` and syntax-checks string operands), and the `unsafe` `transmute`
  is gone (the type owns its data), but no crate outside `engine/src/validation.rs` calls the
  module yet;
  `validate_field_value` is a no-op (`validation.rs:280-309`); `check_circular_references` is a
  no-op (`validation.rs:351-358`); the "Comparison between incompatible types" warning is
  unreachable because `condition_to_value` resolves only `True`/`False` leaves
  (`validation.rs:213-226` vs `312-318`). `ValidationResult` derives `Default`, which yields
  `is_valid = false` for a pristine value and contradicts `ValidationResult::new()`
  (`validation.rs:10,21-27`), and no validation type is `Serialize`d for the API.
- **~~`Value::Number` (Decimal) cannot be ordered anywhere.~~** Closed 2026-10-25: `compare_values`
  promotes across Integer/Number/Float and orders decimals exactly, so `Number(1) < Number(2)` and
  `1 = 1.0` behave. `Value` still has no `Ord`/`PartialOrd`; ordering remains a free function.
- **~~"First fact of type" resolution is hash-order dependent.~~** Closed 2026-10-25:
  `Expression::FieldRef`, `Action::UpdateFact` and `Action::RetractFact` all resolve to the fact with
  the lowest id (`first_of_type`), so the same working memory always yields the same choice.
  `get_all_facts` still returns hash order, which is documented rather than relied on.
- **Stubs inside the rule language.** `Expression::Call` returns `Value::Null`
  (`rules.rs:342-345`), `Action::CallFunction` and `Action::LogMessage` are no-ops
  (`direct_eval.rs:241-246`), and `WorkingMemory::match_pattern` always returns an empty vector
  (`fact.rs:237-240`).
- **~~`PhreakEngine` retraction is unimplemented.~~** Closed 2026-10-25: PHREAK removes the fact
  from working memory, alpha and beta memory, and prunes its truth-maintenance graph via
  `TruthMaintenance::forget_fact`. All four engines now retract identically
  (`engine/tests/retract_integration.rs`, `engine/tests/rule_semantics.rs`).
- **`TimeValue::Fixed` is unreachable from the lexer.** `parse_time` only ever produces
  `TimeValue::Offset` or `TimeValue::Local` (`temporal.rs:409-422`); `Fixed` is constructed only in
  tests (`temporal.rs:608`) and formats identically to `Local` (`temporal.rs:492`). The carrier's
  three-case documentation (`temporal.rs:44-51`) therefore describes one case that parsing never
  yields.
- **Offset seconds are silently dropped.** `parse_fixed_offset` documents `+HH:mm[:ss]` and an
  inline comment says non-zero seconds are rejected (`temporal.rs:466-468,480-483`), but the
  implementation consumes the field and never uses or rejects it: `secs` is computed from hours and
  minutes only (`temporal.rs:484-485`).
- **DST gap policy differs between arithmetic and deserialization.** Arithmetic maps a gap to
  `None`/null (`temporal.rs:590-596`), while `Deserialize` returns a hard `serde` error for the same
  condition (`temporal.rs:139-144`).
- **Temporal arithmetic is IANA-only and sub-second-less.** Month and day-time addition exist only
  for `DateTime<chrono_tz::Tz>` (`temporal.rs:254-286`); no helper handles `Fixed`, `Local`,
  `Date`, or duration subtraction, and duration parsing/formatting is whole-second only
  (`temporal.rs:354-405,529-564`).
- **`FactType`/`ValueType` are unreferenced.** Declared in `data_types.rs:74-93` and re-exported
  (`engine/src/lib.rs:65`), but no code consults them; the `SemanticValidator` uses its own
  `HashMap<String, Vec<String>>` registry instead (`validation.rs:177`).
