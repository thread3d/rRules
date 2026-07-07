//! FEEL semantic value-domain module — sibling to [`crate::dmn::model`],
//! [`crate::dmn::parser`], and [`crate::dmn::exporter`].
//!
//! This module tree carries the FEEL (Friendly Enough Expression Language)
//! value domain that the Phase 31 evaluator will operate on:
//!
//! - [`types`] — the `FeelType` enum (the DMN 1.5 type lattice nodes) and the
//!   `FeelRange` / `FeelFunction` carrier structs used by
//!   [`crate::data_types::Value`]'s `Range` and `Function` variants. Lattice
//!   relations (`conforms`, `equivalent`, `implicit_convert`,
//!   `resolve_type_ref`) land in Plan 04.
//! - [`temporal`] — the five FEEL temporal carrier types: `TimeValue`,
//!   `DateTimeValue` (carrying fixed-offset, IANA-zone, or local forms), and
//!   `YearsMonthsDuration`. The `@""` parse/format and DST-correct duration
//!   arithmetic helpers land in Plan 03.
//! - [`numeric`] — the FEEL Decimal128 numeric helpers (`feel_numeric_eq`,
//!   `0.1 + 0.2 = 0.3`). Implemented in Plan 02.
//!
//! Plan 30-01 ships only the skeleton type declarations — enough for
//! [`crate::data_types::Value`] to carry the eight new FEEL variants and for
//! downstream plans to build on a typed foundation.

pub mod numeric;
pub mod temporal;
pub mod types;