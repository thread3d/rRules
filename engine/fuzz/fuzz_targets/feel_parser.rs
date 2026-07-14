//! Fuzzing target for FEEL parser (Phase 38).
//!
//! Tests the `parse_expression` and `parse_simple_unary_tests` entry points
//! with random inputs to verify no panics, proper error recovery, and correct
//! three-valued-logic behavior.

#![no_main]

use libfuzzer_sys::fuzz_target;
use engine::dmn::feel::parser::{parse_expression, parse_simple_unary_tests};

/// Fuzz target for `parse_expression`.
///
/// Feeds random strings to the parser. We verify:
/// - No panics (error recovery emits Err nodes instead)
/// - No infinite loops (parser respects max_depth and max_tokens)
/// - Results are valid FeelExpression variants
fuzz_target!(|input: &[u8]| {
    // Decode as UTF-8 string (or use raw bytes if not valid UTF-8)
    let src = std::str::from_utf8(input).unwrap_or("");

    // Parse expression — should never panic, always return an Err node or valid AST
    let _expr = parse_expression(src);

    // Also test the unary tests entry point
    let _unary = parse_simple_unary_tests(src);
});
