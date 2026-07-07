//! Explicit FEEL coercions used by operators and built-in functions.
//!
//! Distinct from the Phase 30 lattice `implicit_convert`
//! (`feel::types::implicit_convert` — exactly the three §10.3.2.9.4
//! conversions: to-singleton-list, from-singleton-list, conforms-to). This
//! module carries the **explicit** coercions the operators and built-ins
//! perform, e.g. number/integer cross-type promotion in arithmetic,
//! singleton-list unwrapping for `+`/`-` on a list of one, etc.
//!
//! **Prohibition (carried from Phase 30 Pitfall 6):** there is NO
//! number↔string conversion here — that is the explicit `string()` /
//! `number()` built-in (31-07), NOT an operator coercion. `1 + "2"` yields
//! `Value::Null`.
//!
//! *(skeleton — implemented in plan 31-03 alongside the operators that
//! consume it)*