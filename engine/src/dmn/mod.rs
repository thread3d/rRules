//! DMN 1.5 model, XML parser, and exporter — canonical IR for round-trip (Phase 29).
//!
//! This module tree is the parallel DMN evaluation surface: sibling to
//! [`crate::engine`] but NOT routed through [`crate::engine::EngineRegistry`].
//! DMN is stateless, side-effect-free, and three-valued; the Rete family is
//! forward-chaining, working-memory-mutating, and two-valued. The two paths do
//! not share an execution engine or an `Expression` AST.
//!
//! - [`model`] — the canonical DMN 1.5 in-memory IR. Sole store for every
//!   downstream phase (30–35 parse into or evaluate against it). Round-trip
//!   fidelity is structural from day one (Pitfall 11): IDs, descriptions,
//!   annotations, CDATA flags, and exact document element order are preserved.
//! - [`parser`] — `.dmn` XML → [`model`] via `quick-xml` `NsReader`. Namespace
//!   aware, XXE-hardened, CDATA-handled. Implemented in Plan 29-02.
//! - [`exporter`] — [`model`] → `.dmn` XML via `quick-xml::Writer`. Implemented
//!   in Plan 29-03.
//!
//! FEEL cell/expression text is carried as `String` here; Phase 31 parses it
//! into a distinct `FeelExpression` AST (NOT the rule-engine `Expression`).

pub mod exporter;
pub mod model;
pub mod parser;