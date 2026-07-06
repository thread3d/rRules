//! DMN 1.5 XML parser — Plan 29-02 (RED phase: failing test + stub API).
//!
//! The public API signatures are in place; the streaming namespace-aware
//! `quick-xml` `NsReader` implementation lands in the GREEN commit. The
//! `parser_smoke_red` test asserts a minimal valid DMN 1.5 document parses
//! to a non-empty IR — it FAILS against this stub (returns `Err`).
//!
//! ## XXE hardening contract (T-29-01 mitigation)
//!
//! The GREEN implementation MUST reject any `<!DOCTYPE>` / external-entity
//! declaration with [`crate::dmn::model::DmnParseError::XxeRejected`] and MUST
//! default an absent `hitPolicy` attribute to
//! [`crate::dmn::model::HitPolicy::Unique`] (DMN-04).

use crate::dmn::model::{DmnModel, DmnParseError};

/// Parse a DMN document from a string. Wraps [`parse_dmn_bytes`].
///
/// *(Stub — implemented in the GREEN commit.)*
pub fn parse_dmn(_input: &str) -> Result<DmnModel, DmnParseError> {
    Err(DmnParseError::MissingElement(
        "parser not yet implemented (RED)".into(),
    ))
}

/// Parse a DMN document from bytes (the real implementation, GREEN commit).
///
/// *(Stub — implemented in the GREEN commit.)*
pub fn parse_dmn_bytes(_input: &[u8]) -> Result<DmnModel, DmnParseError> {
    Err(DmnParseError::MissingElement(
        "parser not yet implemented (RED)".into(),
    ))
}

/// Parse a DMN document from a file path.
///
/// *(Stub — implemented in the GREEN commit.)*
pub fn parse_dmn_file(_path: &str) -> Result<DmnModel, DmnParseError> {
    Err(DmnParseError::MissingElement(
        "parser not yet implemented (RED)".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RED driver: a minimal valid DMN 1.5 document MUST parse to a non-empty
    /// IR. Against the stub this fails (parse_dmn returns `Err`). The GREEN
    /// implementation makes it pass.
    const MINIMAL_1_5: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20200524/MODEL/" id="_d1" name="Min" namespace="https://www.omg.org/spec/DMN/20200524/MODEL/">
  <itemDefinition id="_itemAge" name="Age" isCollection="false">
    <typeRef>feel:number</typeRef>
  </itemDefinition>
</definitions>"#;

    #[test]
    fn parser_smoke_red() {
        let model = parse_dmn(MINIMAL_1_5).expect("minimal DMN 1.5 should parse");
        assert!(
            !model.definitions.children.is_empty(),
            "definitions.children must not be empty"
        );
    }
}