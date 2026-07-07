//! Wave-0 DMN-08 precision-audit gate.
//!
//! Scans the vendored OMG DMN TCK subset under `engine/tests/fixtures/tck/`
//! and asserts that no value-domain numeric literal exceeds 29 significant
//! digits — confirming research assumption A1 (30-RESEARCH.md Open Question 1):
//! `rust_decimal::Decimal` (28–29 sig digits) is sufficient for the FEEL value
//! domain. If any >29-digit literal is found, the test MUST NOT silently pass:
//! it emits a clear failure listing each vector and prints the documented
//! `bigdecimal` fallback path. `Value::Number`'s backing type is NOT switched
//! here — that is Phase 31's call.
//!
//! No new deps — `std::fs` walking only.

use std::fs;
use std::path::{Path, PathBuf};

/// The vendored TCK subset directories that MUST be present (Plan 30-01 Task 2).
const EXPECTED_SUBDIRS: &[&str] = &[
    "0100-arithmetic",
    "1100-feel-decimal-function",
    "1141",
    "1142",
    "1143",
    "1144",
    "0002-input-data-number",
];

/// Threshold: rust_decimal holds 28–29 significant digits. Any value-domain
/// literal with >29 sig digits triggers the documented fallback path.
const SIG_DIGIT_THRESHOLD: usize = 29;

#[test]
fn tck_precision_audit() {
    let tck_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("tck");

    // --- Gate 1: the vendored subset must exist. ---
    assert!(
        tck_root.is_dir(),
        "TCK fixture directory not found: {}. Plan 30-01 Task 2 must vendor the OMG TCK subset first.",
        tck_root.display()
    );
    for sub in EXPECTED_SUBDIRS {
        let p = tck_root.join(sub);
        assert!(
            p.is_dir(),
            "Expected vendored TCK subdirectory missing: {}. Plan 30-01 Task 2 must vendor it.",
            p.display()
        );
    }

    // --- Gate 2: scan every XML file for >29-sig-digit numeric literals. ---
    let mut xml_files: Vec<PathBuf> = Vec::new();
    collect_xml_files(&tck_root, &mut xml_files);
    assert!(
        !xml_files.is_empty(),
        "No XML files found under {}; the vendored subset is empty.",
        tck_root.display()
    );

    let mut offenders: Vec<String> = Vec::new();
    for file in &xml_files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(e) => {
                offenders.push(format!("READ ERROR {}: {}", file.display(), e));
                continue;
            }
        };
        for lit in iter_numeric_literals(&content) {
            let sig = significant_digits(&lit.text);
            if sig > SIG_DIGIT_THRESHOLD {
                offenders.push(format!(
                    "{}: literal {:?} has {} significant digits (>{}); file={}",
                    lit.text,
                    lit.text,
                    sig,
                    SIG_DIGIT_THRESHOLD,
                    file.display()
                ));
            }
        }
    }

    if !offenders.is_empty() {
        eprintln!(
            "\n==== TCK PRECISION AUDIT: A1 REFUTED — {} value-domain literal(s) exceed {} sig digits ====",
            offenders.len(),
            SIG_DIGIT_THRESHOLD
        );
        for o in &offenders {
            eprintln!("  - {}", o);
        }
        eprintln!(
            "\nDocumented fallback path (30-RESEARCH.md Standard Stack / Pitfall 2):"
        );
        eprintln!(
            "  Add `bigdecimal` 0.4.10 localized to the Phase 31 rounding built-ins"
        );
        eprintln!("  (round up/down/half-up/half-down, floor, ceiling, decimal).");
        eprintln!("  `Value::Number` backing type stays `rust_decimal::Decimal` — the swap");
        eprintln!("  is localized because the numeric *value* (e.g. 5.5) is equal under FEEL");
        eprintln!("  numeric equality regardless of representational scale.");
        panic!(
            "TCK precision audit found {} value-domain literal(s) exceeding {} significant digits — \
             see stderr for the documented bigdecimal fallback path.",
            offenders.len(),
            SIG_DIGIT_THRESHOLD
        );
    }

    println!(
        "TCK precision audit: A1 confirmed — scanned {} XML file(s), no value-domain literal exceeds {} sig digits.",
        xml_files.len(),
        SIG_DIGIT_THRESHOLD
    );
}

/// Recursively collect `.xml` and `.dmn` files under `dir`. Both carry
/// numeric literals: the test-case XML holds `xsd:decimal`/`xsd:double`
/// expected values, and the `.dmn` files hold FEEL `<text>` expressions.
fn collect_xml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_xml_files(&p, out);
        } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if ext == "xml" || ext == "dmn" {
                out.push(p);
            }
        }
    }
}

/// A scanned numeric literal and the byte offset where it was found.
struct Literal {
    text: String,
}

/// Iterate over maximal numeric tokens in `s`: optional leading `-`, a run of
/// digits with an optional single `.` and optional exponent (`e`/`E` with
/// optional sign and digits). A token is only extracted when it is bounded on
/// BOTH sides by non-alphanumeric characters (or string boundaries) — this
/// excludes digit runs embedded in alphanumeric identifiers (e.g. the hex
/// hash suffix in TCK test-case IDs like `001_af177e63fc`, where `177e63` is
/// NOT a scientific-notation number). This is a deliberately conservative
/// scanner: it favours recall over precision so a 30-digit value is never
/// missed, while the boundary rule removes the common ID-fragment false
/// positive.
fn iter_numeric_literals(s: &str) -> Vec<Literal> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        // A numeric token starts at a digit, or at '-' followed by a digit.
        let starts_here = b.is_ascii_digit()
            || (b == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit());
        if !starts_here {
            i += 1;
            continue;
        }
        let start = i;
        // Left boundary: the char before the token must not be alphanumeric
        // (otherwise this digit run is the tail of an alphanumeric identifier).
        let left_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        // Scan the token.
        if bytes[i] == b'-' {
            i += 1;
        }
        // Integer part digits.
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        // Optional fractional part.
        if i < bytes.len() && bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()
        {
            i += 1; // consume '.'
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
        // Optional exponent.
        if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
            let j = i + 1;
            if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
                // only treat as exponent if a digit follows the sign
                if j + 1 < bytes.len() && bytes[j + 1].is_ascii_digit() {
                    i = j + 1;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            } else if j < bytes.len() && bytes[j].is_ascii_digit() {
                i = j;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
        }
        // Right boundary: the char after the token must not be alphanumeric.
        let right_ok = i >= bytes.len() || !bytes[i].is_ascii_alphanumeric();
        let text = &s[start..i];
        // Keep the token only if both boundaries are clean and it has a digit.
        if left_ok && right_ok && text.chars().any(|c| c.is_ascii_digit()) {
            out.push(Literal {
                text: text.to_string(),
            });
        }
    }
    out
}

/// Count significant digits of a decimal literal string.
///
/// Rules (30-RESEARCH.md Open Question 1):
/// - Ignore the leading sign.
/// - Drop the decimal point, then strip leading zeros (placeholders before the
///   first nonzero digit, in either the integer or fractional part).
/// - Strip trailing zeros ONLY when a decimal point was present (those zeros are
///   scale padding — the value is unchanged; `rust_decimal` holds it). For an
///   integer literal, trailing zeros are significant (e.g. `1000` = 4 sig digs).
/// - A positive exponent that grows the integer magnitude beyond
///   `rust_decimal`'s max (~7.92e28, ~29 integer digits) is flagged as an
///   overflow by returning the effective integer-digit count.
fn significant_digits(lit: &str) -> usize {
    // Strip leading sign.
    let s = lit.trim_start_matches('-').trim_start_matches('+');
    // Split mantissa / exponent.
    let (mant, exp) = match s.find(|c| c == 'e' || c == 'E') {
        Some(idx) => (&s[..idx], Some(&s[idx + 1..])),
        None => (s, None),
    };
    let has_decimal = mant.contains('.');
    // All mantissa digits, decimal point removed.
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    // Strip leading zeros (placeholders).
    let lead_stripped = digits.trim_start_matches('0');
    let mantissa_digits: usize = if has_decimal {
        // Trailing zeros are scale padding when a decimal point is present.
        let trimmed = lead_stripped.trim_end_matches('0');
        trimmed.chars().filter(|c| c.is_ascii_digit()).count()
    } else {
        // Integer literal: trailing zeros are significant.
        lead_stripped.chars().filter(|c| c.is_ascii_digit()).count()
    };

    // Exponent magnitude check: a large positive exponent grows the integer
    // magnitude; if it exceeds rust_decimal's ~29-integer-digit ceiling, flag it.
    if let Some(e) = exp {
        let e_signed: i64 = e.parse::<i64>().unwrap_or(0);
        if e_signed > 0 {
            let int_part = mant.split('.').next().unwrap_or("");
            let int_digits = int_part.trim_start_matches('0').chars().filter(|c| c.is_ascii_digit()).count();
            let effective_int = (int_digits as i64) + e_signed;
            if effective_int > SIG_DIGIT_THRESHOLD as i64 {
                return effective_int as usize;
            }
        }
    }

    mantissa_digits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sig_digits_basic() {
        assert_eq!(significant_digits("5.5"), 2);
        assert_eq!(significant_digits("65.123456"), 8);
        assert_eq!(significant_digits("0.0001"), 1);
        assert_eq!(significant_digits("1000"), 4);
        assert_eq!(significant_digits("-3.14"), 3);
        assert_eq!(significant_digits("1.000"), 1); // trailing fractional zeros are scale padding
        assert_eq!(significant_digits("0"), 0);
        assert_eq!(significant_digits("0.0"), 0);
    }

    #[test]
    fn sig_digits_large() {
        // A 30-digit integer literal exceeds rust_decimal.
        let big = "123456789012345678901234567890";
        assert_eq!(significant_digits(big), 30);
        // A 30-sig-digit decimal with trailing fractional scale padding.
        let big_scale = "1.123456789012345678901234567890000000";
        assert_eq!(significant_digits(big_scale), 30);
        // A value that overflows rust_decimal via a large positive exponent.
        assert_eq!(significant_digits("1e30"), 31);
    }

    #[test]
    fn iter_literals_finds_numbers() {
        let s = r#"<expectedValue type="xsd:decimal">5.5</expectedValue><text>1 + 2</text>"#;
        let lits: Vec<String> = iter_numeric_literals(s).into_iter().map(|l| l.text).collect();
        assert!(lits.contains(&"5.5".to_string()));
        assert!(lits.contains(&"1".to_string()));
        assert!(lits.contains(&"2".to_string()));
    }

    #[test]
    fn iter_literals_excludes_id_fragments() {
        // TCK test-case IDs carry hex hash suffixes (e.g. `001_af177e63fc`).
        // The `177e63` inside is NOT a scientific-notation number.
        let s = r#"<testCase id="001_af177e63fc"><resultNode name="feel-decimal-function_010_4b46e59c8b">"#;
        let lits: Vec<String> = iter_numeric_literals(s).into_iter().map(|l| l.text).collect();
        // `001` is bounded by `"` and `_` — extracted (harmless: 3 sig digits).
        assert!(lits.contains(&"001".to_string()));
        // `177e63` / `46e59` are embedded in alphanumeric IDs — must NOT appear.
        assert!(
            !lits.iter().any(|l| l.contains('e') || l.contains('E')),
            "no exponent tokens should be extracted from alphanumeric IDs, got {:?}",
            lits
        );
    }
}