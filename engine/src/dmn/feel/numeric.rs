//! FEEL Decimal128 numeric helpers.
//!
//! This module will host the FEEL numeric domain helpers — `feel_numeric_eq`
//! (null-propagating numeric equality) and the `0.1 + 0.2 = 0.3` proof — backed
//! by `rust_decimal::Decimal` (per 30-RESEARCH.md DMN-08 recommendation).
//!
//! **Implemented in Plan 02.** Plan 30-01 ships only the module wiring so the
//! `feel/` tree is complete.