# Phase 30: FEEL Value Types + Type Lattice + Temporal Domain - Research

**Researched:** 2026-07-07
**Domain:** FEEL semantic value domain — Decimal128 numbers, ISO 8601 temporal types, DMN 1.5 type lattice
**Confidence:** HIGH (numeric recommendation backed by direct TCK vector inspection; lattice rules quoted verbatim from OMG plain-text spec; temporal model verified against chrono/chrono-tz docs and TCK round-trip vectors)

## Summary

Phase 30 extends `engine::data_types::Value` additively with the FEEL semantic value domain so that Phase 31 can build the evaluator on a correct foundation. The three deliverables are: (1) a Decimal128 numeric path, (2) the five FEEL temporal types, and (3) the FEEL type lattice with conformance, equivalence, and the three spec-defined implicit conversions.

**The single most important deliverable is the numeric-backend choice.** The OMG DMN TCK was inspected directly (sparse clone of `github.com/dmn-tck/tck`). The numeric conformance vectors in `compliance-level-3/0100-arithmetic`, `1100-feel-decimal-function`, `1141`–`1144` (round up/down/half-up/half-down), and `compliance-level-2/0002-input-data-number` were examined. The headline finding: **no TCK vector representing a value literal or arithmetic result requires more than 9 significant fractional digits** (the largest observed is `65.123456`, 8 fractional digits). The Decimal128 scale-boundary tests (`1141` test case `017_a`: `round up(5.5, 6176)`, expected `5.5`; `016_a`: scale `-6111`, **commented out** in the TCK) test *scale-argument acceptance*, not value precision — the expected value `5.5` is numerically equal regardless of representational scale. `rust_decimal::Decimal` (28–29 significant digits, already in `engine/Cargo.toml` at `1.42` with `serde`, `macros`, `maths` features) is therefore sufficient for the Phase 30 value domain.

**Primary recommendation:** Use `rust_decimal::Decimal` for `Value::Number`. Do NOT switch to `bigdecimal` in Phase 30 — no inspected TCK value-domain vector fails on 28–29 digits. Document a **deferred scale-preservation gap** for Phase 31's review: the `round up/down/half-up/half-down`, `floor`, `ceiling`, and `decimal` built-ins accept a `scale` argument up to 6176; `rust_decimal` cannot carry scale > 28, so a high-scale result that is later `string()`-formatted may emit fewer trailing zeros than a strict reading of the spec requires. If Phase 31 finds a TCK `string(round up(x, 6176))` vector that demands 6176 trailing zeros, the swap is localized to those built-ins (a `bigdecimal`-backed rounding helper) and does NOT require changing the `Value::Number` variant, because `5.5` (rust_decimal) and `5.5000…0` (bigdecimal, scale 6176) are numerically equal and compare equal under FEEL numeric equality.

**Temporal:** model the five FEEL types with `chrono` (`NaiveDate`, `NaiveTime`, `DateTime<FixedOffset>` OR `DateTime<chrono_tz::Tz>` for the IANA-zone form) and a custom `YearsMonthsDuration { months: i64 }` (total months, normalized). `chrono::TimeDelta` models days-and-time-duration. `chrono::NaiveDate::checked_add_months(Months::new(n))` handles the Jan 31 + 1 month edge case with the spec-correct "last day of resulting month" semantics. `chrono-tz` (`LocalResult::single/earliest/latest`) resolves DST gaps/overlaps; FEEL spec says local times without offset are "only sometimes comparable to UTC" — model `Time`/`DateTime` as an enum that carries *either* a fixed offset *or* an IANA zone *or* no offset (local), and the TCK round-trip format `2021-02-01T10:10:10@Australia/Melbourne` confirms the IANA zone must be preserved through arithmetic and re-emitted by `to_string()`.

**Type lattice:** the OMG DMN 1.5 plain-text spec (`omg-dmn-taskforce/plain-text-spec`, §10.3.2.9) was read directly. §10.3.2.9.4 defines **exactly three** implicit conversions: *to singleton list*, *from singleton list*, and *conforms to*. There is **NO `number ↔ string` implicit conversion in the lattice** — the roadmap's "number↔string" wording is imprecise; number↔string happens via the explicit `string()` / `number()` built-in functions (Phase 31). The lattice is `Null <: T <: Any` for every `T`, with `list<T> <: list<S>` iff `T <: S`, contravariant-function/covariant-return conformance, and `range<T> <: range<S>` iff `T <: S`. A `date` is spec-equivalent to a `date and time` at UTC midnight (`§10.3.2.3.5`), so `date <: date and time`. Null propagates: `Null <: T` for all `T`, and any operation on null per ternary logic yields null (Phase 31).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DMN-08 | FEEL Decimal128 numeric domain (`rust_decimal`) — `0.1 + 0.2 = 0.3`; precision validated against OMG TCK numeric vectors (fallback to `bigdecimal` if 28–29 digits prove insufficient) | TCK vectors inspected directly; no value-domain vector exceeds 9 fractional digits; `rust_decimal` retained, `bigdecimal` fallback documented as Phase-31-localized if a high-scale `string()` vector demands it. `0.1+0.2=0.3` holds because `rust_decimal` is decimal, not binary float. |
| DMN-09 | FEEL date/time/duration value types — `date`, `time`, `date and time` (IANA tz via `chrono-tz`), `days and time duration`, `years and months duration`; ISO 8601 `@""` lexical literals parse and round-trip | FEEL `@""` grammar (rule 65) + lexical spaces (§10.3.2.3) verified; chrono/chrono-tz mapping table + DST handling + TCK round-trip format `@Australia/Melbourne` confirmed from `0100-arithmetic` vectors. |
| DMN-10 | FEEL type lattice — conformance (`<:`), equivalence (`≡`), implicit conversions (number↔string, singleton-list↔value), null propagation | Spec §10.3.2.9 read verbatim; the three real implicit conversions listed; number↔string corrected to Phase 31 built-ins; Rust representation (trait + table) designed. |
</phase_requirements>

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Decimal128 numeric value representation | Engine core (`engine::data_types`) | — | Value domain is a core engine type shared by Rete + DMN/FEEL; must live alongside existing `Value`. |
| Temporal value representation (date/time/duration) | Engine core (`engine::data_types`) | `engine::dmn::feel::temporal` (lexical parse/format helpers) | The `Value` variants are core; the `@""` lexer is a FEEL-parser concern (Phase 31) but the parse/format helpers belong with the type so round-trip tests can run in Phase 30. |
| Type lattice (conformance/equivalence/conversion) | Engine core (`engine::dmn::feel::types` or `data_types::feel_type`) | — | Lattice is consumed by the evaluator (Phase 31) and by IR `type_ref` resolution; a single authoritative module prevents divergence. |
| IR `type_ref` → lattice node resolution | `engine::dmn` (model) | `engine::dmn::feel::types` | Phase 29 IR carries `type_ref: Option<String>` (e.g. `feel:number`); Phase 30 adds the resolver that maps these strings to lattice nodes. |
| DST-correct duration arithmetic | Engine core temporal helpers | — | `YearsMonthsDuration + DateTime<Tz>` must use `checked_add_months` on the zoned datetime, not fixed-second offsets. |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rust_decimal` | 1.42 (pinned in `engine/Cargo.toml`; latest 1.42.1) | FEEL Decimal128 numeric domain (`Value::Number(Decimal)`) | Already in deps (supply-chain verified 2026-07-06, repo `paupino/rust-decimal`, 115M downloads). Decimal (not binary float) — `0.1+0.2=0.3` holds. 96-bit mantissa, 28–29 significant digits. Features `serde` (IR (de)serialization), `macros` (`dec!` literals), `maths` (sqrt/ln/exp for Phase 31 built-ins). [VERIFIED: docs.rs + Cargo.toml] |
| `chrono` | 0.4 (in `engine/Cargo.toml` with `serde` feature; latest 0.4.45) | `NaiveDate`, `NaiveTime`, `DateTime<FixedOffset>`, `DateTime<Tz>`, `TimeDelta`, `Months` | Already in deps. Proleptic Gregorian, year range covers FEEL's `[-999_999_999, 999_999_999]` (chrono supports `i32` year via `from_ymd_opt`; verify upper bound — see Pitfall 4). `checked_add_months` gives spec-correct Jan-31 + 1-month semantics. [VERIFIED: docs.rs + ctx7] |
| `chrono-tz` | 0.10.4 (NOT yet in deps — must be added) | IANA timezone database (`chrono_tz::Australia::Melbourne` etc.) | Required by DMN-09 for `@Australia/Melbourne` form. `LocalResult::single/earliest/latest` resolves DST gaps/overlaps. Pure Rust, embeds IANA tzdata at compile time (no system tzdata dependency). [VERIFIED: cargo search + ctx7] |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `bigdecimal` | 0.4.10 | Arbitrary-precision decimal | **DO NOT ADD in Phase 30.** Deferred fallback only — add in Phase 31 IF a TCK `string(round up(x, 6176))` vector proves scale 6176 must be preserved in string output. API differs from `rust_decimal` (no `Decimal::new(i64,u32)`; uses `BigDecimal::fromBigInt`). [VERIFIED: cargo search + ctx7 `/akubera/bigdecimal-rs`] |
| `proptest` | 1.11.0 (workspace dev-dep) | Property tests for lattice + temporal round-trip | Already a workspace dev-dependency. Use for round-trip `@""` parse/format and lattice conformance transitivity. [VERIFIED: root Cargo.toml] |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `rust_decimal` | `bigdecimal` | `bigdecimal` = arbitrary precision (handles scale 6176), but slower, no `maths` built-ins, API churn, and NOT needed by any inspected TCK value-domain vector. Keep `rust_decimal` for the value domain; revisit only for Phase 31 rounding built-ins. |
| `rust_decimal` | `decimal` crate (`decimal-rs`) | Less mature, fewer downloads, no supply-chain audit. Rejected. |
| `chrono::DateTime<Tz>` for IANA datetimes | `jid` / `tzfile` / hand-rolled | `chrono-tz` is the canonical chrono companion (same `chronotope` maintainer org), embeds IANA data, DST-aware. No real alternative. |
| `YearsMonthsDuration { months: i64 }` | `chrono::Duration` | `chrono::Duration` (alias `TimeDelta`) is fixed seconds — cannot represent "1 month" (28–31 days). A custom struct is mandatory. `chrono::Months` exists but is a calendar-add helper, not a duration value with normalization; wrap as `{ months: i64 }`. |

**Installation:**
```bash
# In engine/Cargo.toml — chrono-tz is the ONLY new dep Phase 30 must add.
# rust_decimal and chrono are already present.
cargo add chrono-tz --package engine
```

**Version verification (run during planning Wave 0):**
```bash
cargo search rust_decimal    # confirmed 1.42.1 (Cargo.toml pins 1.42)
cargo search chrono          # confirmed 0.4.45 (Cargo.toml pins 0.4)
cargo search chrono-tz       # confirmed 0.10.4 (to be added)
cargo search bigdecimal      # confirmed 0.4.10 (deferred — do NOT add in Phase 30)
```

## Package Legitimacy Audit

| Package | Registry | Age | Downloads | Source Repo | Verdict | Disposition |
|---------|----------|-----|-----------|-------------|---------|-------------|
| `rust_decimal` | crates.io | ~8 yrs | ~115M | github.com/paupino/rust-decimal | OK | Approved (already in Cargo.toml, supply-chain verified 2026-07-06) |
| `chrono` | crates.io | ~10 yrs | ~hundreds of M | github.com/chronotope/chrono | OK | Approved (already in Cargo.toml) |
| `chrono-tz` | crates.io | ~9 yrs | ~tens of M | github.com/chronotope/chrono-tz | OK | Approved — add to engine/Cargo.toml. Same maintainer org as chrono. |
| `bigdecimal` | crates.io | ~7 yrs | ~tens of M | github.com/akubera/bigdecimal-rs | OK (not added) | Approved but DEFERRED — do not install in Phase 30; only if Phase 31 proves scale-preservation need. |

**Packages removed due to SLOP verdict:** none
**Packages flagged as suspicious [SUS]:** none

*All four crates were confirmed via `cargo search` (registry existence) and ctx7/docs.rs (authoritative docs). chrono-tz is the only crate Phase 30 actually installs; the gsd-tools package-legitimacy gate should be run during planning Wave 0 to record the verdict.*

## Architecture Patterns

### System Architecture Diagram

```
                ┌──────────────────────────────────────────────────────────────┐
                │              engine::data_types::Value (extended)            │
                │  Bool | Integer | Float | String | Object | Array | Null     │
                │  + Number(Decimal)  ← DMN-08 (replaces Float in FEEL path)   │
                │  + Date(NaiveDate)  + Time(TimeValue)                        │
                │  + DateTime(DateTimeValue)  ← FixedOffset | IANA | Local     │
                │  + DaysTimeDuration(TimeDelta)                               │
                │  + YearsMonthsDuration{ months: i64 }                       │
                │  + Range{ start, end, start_incl, end_incl }                 │
                │  + Function{ ... }   (variant placeholder; body in Phase 31) │
                └──────────────┬───────────────────────────────────────────────┘
                               │
            ┌──────────────────┼──────────────────────────────┐
            ▼                  ▼                              ▼
   ┌─────────────────┐  ┌─────────────────────┐    ┌────────────────────────┐
   │  Rete path      │  │  FEEL type lattice  │    │  Temporal helpers      │
   │  (unchanged)    │  │  feel_type::Feel    │    │  feel::temporal        │
   │  matches on     │  │  Type { Number,     │    │  parse_at_literal(s)   │
   │  existing 7     │  │  String, Boolean,   │    │   -> Value             │
   │  variants; new  │  │  Date, Time,        │    │  format(value)         │
   │  variants hit   │  │  DateTime, Dt, YmD, │    │   -> String (round-trip│
   │  wildcards (NO  │  │  List, Context,     │    │   preserves IANA zone) │
   │  regression —   │  │  Range, Function,   │    │  add_months_zoned(     │
   │  see Pitfall 1) │  │  Any, Null }        │    │   dt, months)          │
   │                 │  │                     │    │                        │
   │                 │  │  conforms(t1,t2)    │    └────────────────────────┘
   │                 │  │  equivalent(t1,t2)  │
   │                 │  │  implicit_convert(  │
   │                 │  │   value, target)    │
   │                 │  │  resolve_type_ref(  │
   │                 │  │   "feel:number")    │
   │                 │  │   -> FeelType        │
   │                 │  └─────────────────────┘
   │                 │
   └─────────────────┘
                               ▲
                               │ resolves type_ref
            ┌──────────────────┴──────────────────────────────┐
            │  engine::dmn::model (Phase 29 IR — unchanged)    │
            │  ItemDefinition { type_ref: Option<String>,      │
            │                   allowed_values: ... ,          │
            │                   item_components: ... }         │
            │  InformationItem { type_ref: Option<String> }    │
            └──────────────────────────────────────────────────┘
```

Data flow for a FEEL value: a `@""` lexical string enters via `feel::temporal::parse_at_literal` → produces a `Value::Date/Time/DateTime/Duration`; the IR's `type_ref` string (`feel:number`, `feel:date and time`, …) is mapped to a `FeelType` lattice node via `resolve_type_ref`; lattice operations (`conforms`, `equivalent`, `implicit_convert`) are pure functions over `FeelType`/`Value` and are consumed by the Phase 31 evaluator. The Rete path (`engine::engine::*`, `validation.rs`, `rules.rs`) never constructs the new variants and matches them only through existing wildcards — zero behavioral change.

### Recommended Project Structure
```
engine/src/
├── data_types.rs          # Value enum extended additively (new variants)
├── dmn/
│   ├── mod.rs             # re-export feel types module
│   ├── feel/
│   │   ├── mod.rs         # pub mod types; pub mod temporal;
│   │   ├── types.rs       # FeelType enum + FeelTypeLattice: conforms/equivalent/implicit_convert/resolve_type_ref
│   │   └── temporal.rs    # parse_at_literal, format_value, add_months_zoned, YearsMonthsDuration, TimeValue, DateTimeValue
│   ├── model.rs           # Phase 29 IR — UNCHANGED (read-only consumer of type_ref)
│   ├── parser.rs          # UNCHANGED
│   └── exporter.rs        # UNCHANGED
```

### Pattern 1: Additive `Value` extension without `#[non_exhaustive]`
**What:** Add new variants to the existing `pub enum Value` without forking. Do NOT mark `Value` `#[non_exhaustive]` — it is a crate-internal type and exhaustive match is the safety mechanism we WANT at the consumer sites.
**When to use:** Always for this phase.
**Example:**
```rust
// Source: engine/src/data_types.rs (existing) + Phase 30 additions
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    // --- existing (unchanged) ---
    Bool(bool),
    Integer(i64),
    Float(f64),          // retained for the Rete f64 path; FEEL uses Number below
    String(String),
    DateTime(chrono::NaiveDateTime), // LEGACY Rete variant — KEEP; do NOT repurpose
    Object(HashMap<String, Value>),
    Array(Vec<Value>),
    Null,
    // --- Phase 30 FEEL additions (additive) ---
    Number(rust_decimal::Decimal),
    Date(chrono::NaiveDate),
    Time(crate::dmn::feel::temporal::TimeValue),
    DateTimeTz(crate::dmn::feel::temporal::DateTimeValue), // distinct from legacy DateTime
    DaysTimeDuration(chrono::TimeDelta),
    YearsMonthsDuration(crate::dmn::feel::temporal::YearsMonthsDuration),
    Range(Box<crate::dmn::feel::types::FeelRange>),
    Function(crate::dmn::feel::types::FeelFunction),
}
```

### Pattern 2: FEEL type lattice as a method on `FeelType`
**What:** Represent the lattice as an enum + free functions, not a trait graph (FEEL's lattice is small and closed).
**When to use:** Phase 30 lattice implementation.
**Example:**
```rust
// Source: DMN 1.5 spec §10.3.2.9 (verbatim rules, see Code Examples)
pub enum FeelType {
    Null, Any,
    Number, String, Boolean,
    Date, Time, DateTime,
    DaysTimeDuration, YearsMonthsDuration,
    List(Box<FeelType>),
    Context(Vec<(String, FeelType)>),
    Range(Box<FeelType>),
    Function { params: Vec<FeelType>, ret: Box<FeelType> },
}

/// §10.3.2.9.1 — equivalence (≡). Transitive.
pub fn equivalent(a: &FeelType, b: &FeelType) -> bool { /* match arms */ }

/// §10.3.2.9.2 — conformance (<:). Includes equivalence. Transitive.
/// Null <: T <: Any for every T.
pub fn conforms(a: &FeelType, b: &FeelType) -> bool { /* match arms */ }

/// §10.3.2.9.4 — the THREE implicit conversions. Returns None if mandatory but impossible.
pub fn implicit_convert(v: Value, target: &FeelType, ctx: ConvertCtx) -> Option<Value> { /* ... */ }

/// Map IR type_ref strings ("feel:number", "feel:date and time") to FeelType nodes.
pub fn resolve_type_ref(s: &str) -> Option<FeelType> { /* ... */ }
```

### Anti-Patterns to Avoid
- **Repurposing the legacy `Value::DateTime(NaiveDateTime)` variant for FEEL date-time.** The Rete path and `testing/mod.rs` already consume it with `NaiveDateTime` semantics (no offset, no IANA zone). FEEL date-time must carry offset/IANA/local — use a NEW `Value::DateTimeTz(DateTimeValue)` variant. Repurposing would silently change Rete behavior (regression).
- **Using `Value::Float(f64)` for FEEL numbers.** FEEL `0.1 + 0.2` must equal `0.3`; `f64` gives `0.30000000000000004`. Always use `Value::Number(Decimal)` for the FEEL path. The legacy `Float` variant is retained ONLY for the Rete path.
- **Hand-rolling a ` Months` duration as `i64` seconds.** Months are 28–31 days; `YearsMonthsDuration` must store total months, not seconds. Apply via `NaiveDate::checked_add_months`.
- **Adding `#[non_exhaustive]` to `Value`.** It would disable the exhaustive-match safety net that catches missing arms at consumer sites.
- **Treating `number ↔ string` as a lattice implicit conversion.** The spec defines only three implicit conversions (to-singleton-list, from-singleton-list, conforms-to). number↔string is done by the Phase 31 `string()` / `number()` built-ins, not by the lattice.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Decimal128 arithmetic | Custom fixed-point / `f64` | `rust_decimal::Decimal` | Decimal semantics (`0.1+0.2=0.3`), 28–29 digits, supply-chain verified, `maths` features for Phase 31. |
| ISO 8601 date/time parse + format | Regex hand-parser | `chrono` `NaiveDate::parse_from_str` / `DateTime::parse_from_rfc3339` + a thin `@""` wrapper | chrono handles leap years, calendar edge cases; only the `@`-prefix + IANA `@Zone/Name` suffix is FEEL-specific. |
| Month-aware duration arithmetic | `seconds + 30*86400` | `chrono::NaiveDate::checked_add_months(Months::new(n))` | Handles Jan 31 + 1 month → Feb 28/29 (spec: "last day of resulting month"). |
| IANA timezone resolution + DST | `tzdata` file parser | `chrono-tz` | Embeds IANA db; `LocalResult` handles DST gaps/overlaps. |
| Type lattice relations | Trait graph with associated types | Plain `enum FeelType` + `fn conforms/equivalent` | FEEL lattice is small and closed; a table/function is clearer, testable with property tests, and avoids Rust trait-system complexity. |
| `@""` lexical grammar | Hand-rolled tokenizer for the whole literal | `parse_at_literal` dispatches by first char after `@""` (`P`→duration, else date/time) to chrono parsers | The FEEL grammar (rule 65) is literally `"@" string-literal` where the string follows XML Schema lexical spaces — reuse chrono's XML Schema parsers. |

**Key insight:** Every FEEL temporal type maps to an existing chrono type or a trivial wrapper. The only genuinely custom type is `YearsMonthsDuration`, and even that is just `{ months: i64 }` plus a `checked_add_months` call.

## Runtime State Inventory

> Phase 30 is an additive code extension (new `Value` variants + new modules), not a rename/migration. This section is included for completeness.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no database schema or persisted records reference FEEL value variants yet (Phase 31+ will persist evaluated values). | None |
| Live service config | None — no running services depend on the new `Value` variants. | None |
| OS-registered state | None | None |
| Secrets/env vars | None | None |
| Build artifacts | `engine/target/` will recompile on `cargo add chrono-tz`. No stale artifacts carrying old names. | `cargo build` after adding dep. |

## Common Pitfalls

### Pitfall 1: Wildcard match arms silently swallow new FEEL variants (REGRESSION RISK)
**What goes wrong:** `validation.rs::validate_field_value` (lines 290–297) ends with `_ => {}`; `rules.rs` (lines 313, 321, 352) uses `_ => Value::Null`; `data_types.rs` `DynamicCast` impls use `_ => None`. When the new FEEL variants are added, they fall through these wildcards with NO compiler error.
**Why it happens:** The existing code used wildcards because `Value` was closed and the author didn't need to handle every variant. Exhaustive match is only enforced where there is NO wildcard.
**How to avoid:** (a) Audit every `match` on `Value` (grep `Value::` in `engine/src` and `api/src`). (b) For the FEEL path, NEVER route FEEL values through `validation.rs`/`rules.rs` — they are Rete-only. (c) Add a unit test that constructs one of each new variant and asserts the Rete path does not panic (regression guard). (d) Consider replacing the `validation.rs` wildcard with an explicit arm listing (low-risk, optional) so the compiler warns on future additions. (e) `testing/mod.rs::ValueSnapshot::from_value` (lines 82–98) is exhaustive (NO wildcard) — the compiler WILL force a new arm there; plan to add `ValueSnapshot` variants or map new FEEL types to a string fallback.
**Warning signs:** `cargo build` succeeds with zero changes to `validation.rs`/`rules.rs` after adding variants — that is the smell, not a success.

### Pitfall 2: `rust_decimal` scale overflow in Phase 31 rounding built-ins
**What goes wrong:** `round up(5.5, 6176)` — `rust_decimal` cannot represent scale 6176 (max 28). A naive `Decimal::round_dp(6176)` panics or saturates.
**Why it happens:** Decimal128's scale range is `[−6111, 6176]`; `rust_decimal`'s is `[0, 28]` (29 in some cases).
**How to avoid:** Phase 30 only stores the value domain; the rounding built-ins are Phase 31. In Phase 31, implement `round up/down/half-up/half-down` with a scale cap: if scale > 28, the numeric *value* is unchanged (5.5 stays 5.5) and the TCK `017_a` expected `5.5` matches under numeric equality. If a TCK vector demands the *string* form `5.5000…0` (6176 zeros), route that one built-in through a `bigdecimal` helper. **Do NOT change `Value::Number`'s backing type** — keep `rust_decimal`.
**Warning signs:** A Phase 31 TCK `string(round up(x, N))` test for large N fails on trailing-zero count.

### Pitfall 3: IANA timezone lost through arithmetic
**What goes wrong:** `@"2021-01-01T10:10:10@Australia/Melbourne" + @"P1M"` computes correctly but the result serializes as `2021-02-01T10:10:10+11:00` (fixed offset) instead of `…@Australia/Melbourne`, failing the TCK round-trip (expected `xsd:string` `2021-02-01T10:10:10@Australia/Melbourne`).
**Why it happens:** `DateTime<FixedOffset>` discards the IANA zone; adding a `TimeDelta`/`Months` and re-serializing emits the offset, not the zone name.
**How to avoid:** Model `DateTimeValue` as an enum: `Fixed(chrono::DateTime<FixedOffset>)` | `Iana(chrono::DateTime<chrono_tz::Tz>)` | `Local(chrono::NaiveDateTime)`. Carry the `chrono_tz::Tz` through arithmetic. Re-emit the `@Zone/Name` suffix in `format_value`. The TCK vector at `0100-arithmetic` line 607 confirms this is required.
**Warning signs:** TCK `0100-arithmetic` Australia/Melbourne cases fail with an offset mismatch.

### Pitfall 4: chrono year-range ceiling
**What goes wrong:** FEEL spec allows year `[-999_999_999, 999_999_999]`; `chrono::NaiveDate::from_ymd_opt` takes `i32` year (max `2_147_483_647`) — so the FEEL range fits, but only just, and downstream `DateTime` epoch math may overflow for extreme values.
**Why it happens:** chrono uses `i32` year; FEEL's 9-digit year is within `i32` range but `valuedt` (seconds since epoch) overflows `i64` for extreme years.
**How to avoid:** Accept the FEEL year range as documented; in `parse_at_literal`, return `Value::Null` for out-of-range years (spec: `valuedt^-1` returns null for illegal range). Do not attempt to support the full 9-digit year range in epoch arithmetic — the TCK vectors use 4-digit years (`2021-01-01T10:10:10`). Document this as a known limitation.
**Warning signs:** A TCK vector with a 5+ digit year fails.

### Pitfall 5: DST gap/overlap during duration arithmetic
**What goes wrong:** `@"2021-01-01T02:30:00@America/Sao_Paulo" + @"P1D"` lands in a DST spring-forward gap; `from_local_datetime` returns `LocalResult::None` and `.unwrap()` panics.
**Why it happens:** Adding `DaysTimeDuration` to a `DateTime<Tz>` via `*dt + delta` can produce a non-existent local time when the date crosses a DST boundary.
**How to avoid:** Use `tz.from_local_datetime(&naive_result).single()` and map `None`/`Ambiguous` to a spec-defined FEEL result (spec: local times "only sometimes comparable to UTC" — return null or the earlier/later per a documented policy). The TCK `0100-arithmetic` vectors avoid DST-gap edge cases for IANA zones (they use Melbourne which has no gap at the tested dates), but the implementation must not panic. Add a proptest that fuzzes `DateTime<Iana> + P1D` across DST boundaries.
**Warning signs:** `unwrap()` on `LocalResult` in the temporal module.

### Pitfall 6: `number ↔ string` mis-implemented as a lattice conversion
**What goes wrong:** The planner reads the roadmap's "number↔string" wording and implements an implicit `Number → String` conversion in the lattice, causing `1 + "2"` to evaluate to `3` (or `"12"`) instead of `null`.
**Why it happens:** The roadmap wording is imprecise; the spec (§10.3.2.9.4) defines only three implicit conversions and `number ↔ string` is NOT one of them.
**How to avoid:** Implement only `to-singleton-list`, `from-singleton-list`, and `conforms-to`. Number↔string is the Phase 31 `string()` / `number()` built-in functions, invoked explicitly. `1 + "2"` in FEEL yields `null` (type mismatch, no implicit conversion).
**Warning signs:** A FEEL expression mixing number and string operands produces a non-null result.

## Code Examples

### FEEL `@""` literal parsing (DMN 1.5 grammar rule 65 + §10.3.2.3)
```rust
// Source: DMN 1.5 plain-text spec, rule 65 ("at literal = '@', string literal")
//         + §10.3.2.3.3–3.7 (lexical spaces = XML Schema / XDM).
use chrono::{NaiveDate, NaiveTime, NaiveDateTime, DateTime, FixedOffset, TimeDelta};
use rust_decimal::Decimal;

/// Parse the string inside an @"..." literal (the '@' is stripped by the lexer).
/// Returns None (→ Value::Null) if the string does not match any FEEL temporal lexical space.
pub fn parse_at_literal(s: &str) -> Option<Value> {
    // Duration? starts with 'P' (positive) or '-P' (negative) — XDM dayTimeDuration / yearMonthDuration.
    if let Some(rest) = s.strip_prefix('-').or_else(|| s.strip_prefix("P").map(|_| s)) {
        if s.starts_with('P') || s.starts_with("-P") {
            return parse_duration(s); // dispatch: year-month if no 'T'/'D'/'H'/'M'<'S', else day-time
        }
    }
    // Time? no '-' in first 2 chars and contains ':' → XML Schema time.
    if !s.starts_with('-') && s.contains(':') && !s.contains('-') {
        return parse_time(s).map(Value::Time);
    }
    // Date or date-time? contains '-' → YYYY-MM-DD or YYYY-MM-DDTHH:mm:ss[zone].
    if s.contains('-') {
        // date-time if it contains 'T' (or '@' IANA zone marker after the time part).
        if s.contains('T') {
            return parse_date_time(s).map(Value::DateTimeTz);
        }
        return NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().map(Value::Date);
    }
    None
}
```

### YearsMonthsDuration + zoned datetime (DST-correct)
```rust
// Source: chrono docs (checked_add_months) + DMN 1.5 §10.3.2.3.7 (years-months duration
//         normalized so months ∈ [0,11]; valueymd(P1Y1M) = 13).
use chrono::{Months, TimeZone};
use chrono_tz::Australia::Melbourne;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YearsMonthsDuration { pub months: i64 } // total months; normalize on construct

/// Jan 31 + 1 month → Feb 28/29 (spec: last day of resulting month). chrono does this.
pub fn add_years_months_zoned(
    dt: DateTime<chrono_tz::Tz>, ymd: &YearsMonthsDuration,
) -> Option<DateTime<chrono_tz::Tz>> {
    let naive = dt.naive_local();
    let new_naive = naive.checked_add_months(Months::new(ymd.months.unsigned_abs() as u32))?;
    // Re-attach the zone via the tz, handling DST gaps/overlaps.
    dt.timezone().from_local_datetime(&new_naive).single()
        .or_else(|| dt.timezone().from_local_datetime(&new_naive).earliest())
}
```

### Type lattice conformance (DMN 1.5 §10.3.2.9.2 — verbatim rules)
```rust
// Source: DMN 1.5 spec §10.3.2.9.2, quoted verbatim from omg-dmn-taskforce/plain-text-spec.
//
// "Conformance includes equivalence. If T ≡ S then T <: S.
//  For every type T, Null <: T <: Any.
//  list<T> conforms to list<S> iff T conforms to S.
//  context<k1:T1..kn:Tn> conforms to context<l1:S1..lm:Sm> iff n ≥ m and
//    for every li:Si there is a unique kj:Tj with li=kj and Tj <: Si.
//  function (T1..Tn)->U conforms to (S1..Sm)->V iff n=m, Si <: Ti, and U <: V
//    (contravariant args, covariant return).
//  range<T> conforms to range<S> iff T conforms to S.
//  Conformance is transitive."
pub fn conforms(a: &FeelType, b: &FeelType) -> bool {
    match (a, b) {
        (_, FeelType::Any) => true,                 // T <: Any
        (FeelType::Null, _) => true,                // Null <: T
        (_, FeelType::Null) => false,
        (x, y) if equivalent(x, y) => true,         // equivalence implies conformance
        (FeelType::Date, FeelType::DateTime) => true, // §10.3.2.3.5: date ≡ datetime@UTC midnight
        (FeelType::List(t), FeelType::List(s)) => conforms(t, s),
        (FeelType::Range(t), FeelType::Range(s)) => conforms(t, s),
        (FeelType::Context(ae), FeelType::Context(be)) => context_conforms(ae, be),
        (FeelType::Function { params: ap, ret: ar },
         FeelType::Function { params: bp, ret: br }) =>
            ap.len() == bp.len()
            && ap.iter().zip(bp).all(|(s, t)| conforms(s, t)) // contravariant
            && conforms(ar, br),                            // covariant
        _ => false,
    }
}
```

### Implicit conversions (DMN 1.5 §10.3.2.9.4 — exactly three)
```rust
// Source: DMN 1.5 spec §10.3.2.9.4, verbatim.
//
// "to singleton list:  type T, target List<T>  → wrap in singleton list.
//  from singleton list: type List<T>, singleton, target T → unwrap first element.
//  conforms to:  type T1, target T2, T1 <: T2 → value unchanged; else null."
pub enum ConvertCtx { Filter, Invocation, Binding }

pub fn implicit_convert(v: Value, target: &FeelType, ctx: ConvertCtx) -> Option<Value> {
    match ctx {
        ConvertCtx::Filter => to_singleton_list(v, target), // T → List<T>
        ConvertCtx::Invocation => from_singleton_list(v, target), // List<T> singleton → T
        ConvertCtx::Binding => if conforms(&type_of(&v), target) { Some(v) } else { None },
    }
}
// NOTE: NO number→string, NO string→number. Those are Phase 31 string()/number() built-ins.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| FEEL number = "Java BigDecimal with MathContext DECIMAL128" | FEEL number = IEEE 754-2008 Decimal128 (scale range `[−6111, 6176]`, 34 sig digits) | DMN 1.6b1 (OMG issue DMN16-81, closed) | Clarifies the canonical reference; `rust_decimal` (28–29 digits) is a *practical subset* sufficient for all TCK value-domain vectors inspected. |
| `chrono` `Date::checked_add_months` | Stable `NaiveDate::checked_add_months(Months::new(n))` | chrono 0.4.x | No more `Date` deprecation; use `NaiveDate` + `Months`. |
| `chrono-tz` 0.8 | `chrono-tz` 0.10.4 | 2024–2025 | Updated IANA database; same `LocalResult` API. |

**Deprecated/outdated:**
- `chrono::Date` (deprecated) — use `NaiveDate`.
- `rust_decimal::Decimal::new(i64, u32)` — still valid; prefer `dec!` macro for literals.
- `chrono::Duration` — renamed to `TimeDelta` (alias retained); use `TimeDelta` in new code.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | No TCK value-domain vector requires >29 significant digits. | Standard Stack / DMN-08 | If a TCK vector (not inspected — e.g. in `0058-feel-number-function` or a level-2 folder I did not fully enumerate) has a 30+ digit literal, `rust_decimal` would fail that vector and `bigdecimal` becomes mandatory in Phase 30. Mitigation: planner adds a Wave-0 task to grep all TCK `xsd:decimal`/`xsd:double` expected values for length >29 and all DMN `<text>` numeric literals for >29 digits. |
| A2 | `chrono` `i32` year covers the FEEL `[-999_999_999, 999_999_999]` range for all TCK vectors. | Pitfall 4 | `i32::MAX` = 2_147_483_647 > 999_999_999 so the range fits; but if a TCK vector uses a 10-digit year, `from_ymd_opt` returns `None` → `Value::Null`. Mitigation: Wave-0 grep TCK for years >9999. |
| A3 | The TCK `017_a` (`round up(5.5, 6176)` → `5.5`) comparison is numeric, not scale-aware. | Pitfall 2 / DMN-08 | If the TCK runner compares decimal *scale* (trailing zeros), `rust_decimal` returning `5.5` (scale 1) vs expected `5.5` (scale 6176) would mismatch. The TCK XML writes `5.5` (no trailing zeros) which strongly suggests numeric comparison. This is a Phase 31 concern (rounding built-ins), not Phase 30. |
| A4 | `chrono-tz` 0.10.4 is MSRV-compatible with the project's `edition = "2021"` / rustc 1.94.1. | Standard Stack | `chrono-tz` 0.10 requires Rust ≥1.63 (per its Cargo.toml); rustc 1.94.1 satisfies this. Low risk. |
| A5 | `chrono-tz` embeds the IANA database at compile time (no system `tzdata` dependency). | Standard Stack | Confirmed from chrono-tz docs/README via ctx7. If false, deployment to Alpine/musl (Phase 21 Docker) would need `tzdata` installed — but the v4.0 Docker images already ship `tzdata`. |

**If this table is empty:** N/A — five assumptions logged, all with mitigation tasks for the planner.

## Open Questions (RESOLVED)

1. **Does any TCK vector demand >29 significant digits in a *value* (not a `scale` argument)?** — RESOLVED: Wave-0 `tck_precision_audit` gate in Plan 01 Task 2 confirms assumption A1 for the vendored subset (scans for >29-digit literals); if any hit is found, the plan documents the `bigdecimal` fallback before Phase 31.
   - What we know: `0100-arithmetic`, `1100-decimal`, `1141`–`1144`, `0002-input-data-number` inspected — max 8 fractional digits.
   - What's unclear: `0058-feel-number-function`, `0008-LX-arithmetic`, `0009-invocation-arithmetic`, `0061-feel-median`, `0063-feel-stddev` were not exhaustively scanned for >29-digit literals.
   - Recommendation: Wave-0 task — `grep -rhE 'xsd:(decimal|double)">-?[0-9]{30,}' TestCases/` and `grep -rhE '<text>[^<]*[0-9]{30,}' TestCases/` over the full TCK clone. If zero hits, A1 is confirmed and `rust_decimal` is locked.

2. **DST gap/overlap FEEL result policy** — RESOLVED: Plan 03 Task 1 implements a documented policy — return `null` for non-existent local times (`LocalResult::None`) and earliest for ambiguous (`LocalResult::Ambiguous`); Phase 31 confirms against TCK if a vector exercises it.
   - What we know: spec says local times without offset are "only sometimes comparable to UTC"; chrono-tz returns `LocalResult::None`/`Ambiguous`.
   - What's unclear: what does FEEL say the result of `@"2021-01-01T02:30:00@America/Sao_Paulo" + @"P1D"` is when the result lands in a DST gap? Return null? Earlier? Later?
   - Recommendation: Phase 30 picks a documented policy (return null for non-existent; earliest for ambiguous) and records it; Phase 31 confirms against TCK if a vector exercises it.

3. **`FeelFunction` variant payload in Phase 30** — RESOLVED: Plan 01 Task 1 adds `Value::Function(FeelFunction)` where `FeelFunction` is a placeholder struct carrying arity + param/return `FeelType`s ONLY (enough for lattice `conforms`/`equivalent`). The closure body is Phase 31.
   - What we know: Phase 31 defines `FeelExpression` (the AST) and lambda closures.
   - What's unclear: what should `Value::Function` carry in Phase 30 — just a placeholder for the type lattice, or a full closure?
   - Recommendation: Phase 30 adds `Value::Function(FeelFunction)` where `FeelFunction` is a placeholder struct carrying arity + param/return `FeelType`s ONLY (enough for lattice `conforms`/`equivalent`). The closure body is Phase 31.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `rustc` | All Rust code | ✓ | 1.94.1 | — |
| `cargo` | Build | ✓ | (workspace) | — |
| `rust_decimal` | DMN-08 numeric | ✓ (in Cargo.toml) | 1.42 (latest 1.42.1) | `bigdecimal` 0.4.10 (deferred) |
| `chrono` | DMN-09 temporal | ✓ (in Cargo.toml) | 0.4 (latest 0.4.45) | — |
| `chrono-tz` | DMN-09 IANA zones | ✗ (NOT in Cargo.toml) | 0.10.4 (latest) | None — must add |
| `bigdecimal` | DMN-08 fallback | ✗ (not added) | 0.4.10 | Only if A1 fails |
| `proptest` | Validation (property tests) | ✓ (workspace dev-dep) | 1.11.0 | — |
| OMG DMN TCK repo | DMN-08 validation vectors | ✓ (cloned to `/tmp/tck-check`) | master branch | — |

**Missing dependencies with no fallback:**
- `chrono-tz` — must be added to `engine/Cargo.toml` in Wave 0. No alternative provides IANA-zone DST-correct arithmetic for chrono.

**Missing dependencies with fallback:**
- `bigdecimal` — only needed if Wave-0 TCK scan (Open Question 1) finds a >29-digit value vector.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` 1.11.0 (workspace dev-dep) |
| Config file | `engine/Cargo.toml` `[dev-dependencies]` (proptest already present) |
| Quick run command | `cargo test -p engine --lib data_types:: dmn::feel::` |
| Full suite command | `cargo test -p engine` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DMN-08 | `0.1 + 0.2 = 0.3` in `Decimal` | unit | `cargo test -p engine --lib dmn::feel::numeric::test_decimal_add` | ❌ Wave 0 |
| DMN-08 | TCK numeric vectors pass (subset: `0100-arithmetic`, `1100-decimal`) | integration | `cargo test -p engine --test feel_tck_numeric` | ❌ Wave 0 (TCK XML checked into `engine/tests/fixtures/tck/`) |
| DMN-08 | No TCK value vector exceeds 29 significant digits (A1 confirmation) | smoke (Wave 0 gate) | `cargo test -p engine --test tck_precision_audit` | ❌ Wave 0 |
| DMN-09 | `@""` literals for all 5 types parse + round-trip | property | `cargo test -p engine --lib dmn::feel::temporal::tests -- --nocapture` + `cargo test -p engine --lib temporal_proptest` | ❌ Wave 0 |
| DMN-09 | `DateTime + YearsMonthsDuration` via `0100-arithmetic` vectors (Melbourne) | integration | `cargo test -p engine --test feel_tck_temporal` | ❌ Wave 0 |
| DMN-09 | Jan 31 + 1 month → Feb 28/29 (last-day-of-month) | unit | `cargo test -p engine --lib dmn::feel::temporal::test_month_edge` | ❌ Wave 0 |
| DMN-10 | `conforms` is reflexive + transitive; `Null <: T <: Any` | property | `cargo test -p engine --lib dmn::feel::types::conformance_proptest` | ❌ Wave 0 |
| DMN-10 | `equivalent` is reflexive/symmetric/transitive | property | `cargo test -p engine --lib dmn::feel::types::equivalence_proptest` | ❌ Wave 0 |
| DMN-10 | The 3 implicit conversions (to/from singleton list, conforms-to) | unit + property | `cargo test -p engine --lib dmn::feel::types::implicit_convert` | ❌ Wave 0 |
| DMN-10 | `1 + "2"` → null (NO number↔string implicit conversion) | unit | `cargo test -p engine --lib dmn::feel::types::test_no_number_string_conversion` | ❌ Wave 0 |
| Regression | Rete path unchanged — `validation.rs`/`rules.rs`/`testing/mod.rs` compile + existing tests pass | regression | `cargo test -p engine` (full suite) | ✅ (existing) |

### Sampling Rate
- **Per task commit:** `cargo test -p engine --lib data_types:: dmn::feel::`
- **Per wave merge:** `cargo test -p engine`
- **Phase gate:** Full suite green + TCK numeric + TCK temporal subset green before `/gsd-verify-work`.

### Wave 0 Gaps
- [ ] `engine/src/dmn/feel/mod.rs` — module wiring (`pub mod types; pub mod temporal;`)
- [ ] `engine/src/dmn/feel/types.rs` — `FeelType`, `conforms`, `equivalent`, `implicit_convert`, `resolve_type_ref` + property tests
- [ ] `engine/src/dmn/feel/temporal.rs` — `TimeValue`, `DateTimeValue`, `YearsMonthsDuration`, `parse_at_literal`, `format_value`, `add_months_zoned` + round-trip tests
- [ ] `engine/tests/fixtures/tck/` — vendored subset of TCK `0100-arithmetic`, `1100-feel-decimal-function`, `1141`–`1144` XML+DMN (license: OMG TCK is community/conformance — record provenance)
- [ ] `engine/tests/feel_tck_numeric.rs` — runs vendored TCK numeric vectors against `Value::Number`
- [ ] `engine/tests/feel_tck_temporal.rs` — runs `0100-arithmetic` temporal+duration vectors
- [ ] `engine/tests/tck_precision_audit.rs` — Wave-0 gate: scans vendored TCK for >29-digit values (confirms/invalidates A1)
- [ ] `cargo add chrono-tz --package engine` — the one dependency install

*(If no gaps: N/A — all infrastructure is new in Wave 0.)*

## Security Domain

> Phase 30 adds value-domain types and pure lattice functions. `security_enforcement` is enabled by default in `.planning/config.json` (no `security_enforcement: false` key present). However, this phase introduces no new input boundary, no network surface, no secrets, and no cryptography. The `@""` literal parser is the only new code that touches untrusted input (FEEL text from a `.dmn` file), but parsing is delegated to `chrono`/`rust_decimal` (battle-tested) and the parser is reached only via the Phase 29 XXE-hardened parser pipeline.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V2 Authentication | no | — (no auth surface) |
| V3 Session Management | no | — |
| V4 Access Control | no | — |
| V5 Input Validation | yes | `parse_at_literal` returns `None`→`Value::Null` on any parse failure (fail-closed); chrono/rust_decimal parsers reject malformed input. No `unwrap()` on `Option`/`Result` from untrusted input (see Pitfall 5). |
| V6 Cryptography | no | — |

### Known Threat Patterns for FEEL value-domain

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Malformed `@""` literal causing panic | Denial of Service | `parse_from_str` → `Option`/`Result`; map `None` to `Value::Null`; no `unwrap()` on parser output. Proptest fuzzes malformed strings. |
| Decimal overflow / division by zero | Denial of Service | `rust_decimal` division by zero → `Err`; Phase 30 value domain only stores results — the evaluator (Phase 31) maps `Err` → `Value::Null` (TCK `0078`: `1.0/0.0` → null). No NaN/Infinity in FEEL. |
| Giant numeric literal in `.dmn` | Denial of Service | `rust_decimal` parses to a fixed 96-bit mantissa; oversized literals error → null. No unbounded allocation. (Contrast: `bigdecimal` IS unbounded — another reason to prefer `rust_decimal` for the value domain.) |
| DST-gap panic during duration arithmetic | Denial of Service | `LocalResult::single()` → `Option`; `None`/`Ambiguous` → documented policy (null/earliest), never `unwrap()`. |

## Sources

### Primary (HIGH confidence)
- OMG DMN 1.5 plain-text spec (`omg-dmn-taskforce/plain-text-spec`, `10-expression-language-FEEL.adoc`) — §10.3.2.3 (temporal semantic domain), §10.3.2.9.1 (equivalence), §10.3.2.9.2 (conformance), §10.3.2.9.4 (the three implicit conversions), grammar rules 60/65. [CITED: github.com/omg-dmn-taskforce/plain-text-spec/blob/main/10-expression-language-FEEL.adoc]
- OMG DMN TCK repo (`dmn-tck/tck`, master) — test vectors inspected directly via sparse clone: `compliance-level-3/0100-arithmetic`, `1100-feel-decimal-function`, `1141`–`1144` (scale boundaries), `compliance-level-2/0002-input-data-number`. [CITED: github.com/dmn-tck/tck]
- `rust_decimal` docs (docs.rs) — 96-bit mantissa, 28–29 significant digits, comparison-to-bigdecimal note. [CITED: docs.rs/rust_decimal]
- `chrono` docs (docs.rs + ctx7 `/websites/rs_chrono_chrono`) — `NaiveDate::checked_add_months`, `Months`, `TimeDelta`, `LocalResult`. [CITED: docs.rs/chrono]
- `chrono-tz` docs (ctx7 `/chronotope/chrono-tz`) — `LocalResult::single/earliest/latest`, DST gap/overlap handling. [CITED: github.com/chronotope/chrono-tz]
- OMG issue DMN16-81 (closed) — FEEL number = IEEE 754-2008 Decimal128, scale range `[−6111, 6176]`. [CITED: issues.omg.org/issues/DMN16-81]
- Project source — `engine/src/data_types.rs` (current `Value` enum, 380 lines), `engine/Cargo.toml` (deps), `engine/src/dmn/model.rs` (`ItemDefinition.type_ref`), `engine/src/validation.rs`/`rules.rs`/`testing/mod.rs` (consumer match arms). [VERIFIED: codebase]

### Secondary (MEDIUM confidence)
- Camunda 8 FEEL language guide — `@""` literal format examples (cross-checked against spec grammar rule 65). [CITED: docs.camunda.io/feel/language-guide/feel-data-types]
- `cargo search` — confirmed latest versions: `rust_decimal 1.42.1`, `chrono 0.4.45`, `chrono-tz 0.10.4`, `bigdecimal 0.4.10`. [VERIFIED: registry]

### Tertiary (LOW confidence)
- None — all findings are spec-cited, registry-verified, or codebase-verified.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — `rust_decimal`/`chrono` already in deps and verified; `chrono-tz` version confirmed via `cargo search`; `bigdecimal` deferred with evidence.
- Architecture: HIGH — current `Value` enum and all consumer match arms read; additive pattern verified; regression risks enumerated.
- Numeric recommendation (DMN-08): HIGH — TCK vectors inspected directly; the one residual gap (full >29-digit scan) is a Wave-0 task, not an uncertainty about the recommendation direction.
- Temporal (DMN-09): HIGH — spec lexical spaces + chrono API + TCK round-trip format all cross-confirmed.
- Lattice (DMN-10): HIGH — spec §10.3.2.9 quoted verbatim; roadmap's "number↔string" corrected against the spec.
- Pitfalls: HIGH — each pitfall grounded in a specific codebase line or TCK vector.

**Research date:** 2026-07-07
**Valid until:** 2026-08-06 (30 days — stable domain; OMG DMN spec and chrono/rust_decimal APIs are slow-moving. Re-verify `chrono-tz` latest version and TCK repo HEAD before Phase 31 planning.)