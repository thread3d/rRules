//! FEEL built-in function library — `BuiltinFn` registry + ~70+ functions.
//!
//! Plan 31-07 (part 1: conversion/string/list/numeric families) and 31-08
//! (part 2: date-time/context/boolean/range/equality families) own the
//! real implementations and the `HashMap<&'static str, BuiltinFn>`
//! registry table (CONTEXT.md locked dispatch shape — chosen over a nested
//! `match` that scales poorly to ~70 entries, and over trait objects per
//! family that over-engineer a closed, flat function space).
//!
//! **Null propagation (general rule):** a null argument propagates to
//! `Value::Null` UNLESS the spec says otherwise — exceptions:
//! `count([null])` → `1`, `string(null)` → `null`, `is(a, b)` → NEVER null
//! (Pitfall 6: total structural identity), `all([])` → `true`, `any([])` →
//! `false`, `sum([])` → `null` per DMN 1.5 (31-RESEARCH.md Built-in
//! Function Library / Null-propagation behavior).
//!
//! *(skeleton — implemented in plans 31-07 + 31-08)*

/// A built-in function pointer. Null args propagate to `Value::Null`
/// unless the spec says otherwise (see module docs). The real registry
/// (`HashMap<&'static str, BuiltinFn>`, ~70+ entries) lands in 31-07/31-08.
///
/// 31-RESEARCH.md Code Examples / Built-in function registry shape.
// (skeleton — registry implemented in plans 31-07 + 31-08)
pub type BuiltinFn = fn(&[crate::data_types::Value]) -> crate::data_types::Value;