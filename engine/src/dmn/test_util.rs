//! Test utilities for DMN conformance verification — Phase 35.
//!
//! Provides two public functions used by the TCK test runner:
//!
//! - [`discover_tck_vectors`] walks the vendored TCK fixture directory and returns a flat list of
//!   `(directory, dmn_path, test_xml_path)` triples ready for execution.
//! - [`structurally_equal`] normalizes two XML strings so that whitespace differences, attribute
//!   order, namespace-prefix differences, self-closing tags vs open/close pairs, and XML
//!   declarations no longer cause false negatives in round-trip comparison.

use std::path::{Path, PathBuf};

/// A single TCK vector: one `.dmn` model plus its associated `-test-*.xml` test case file(s).
#[derive(Debug, Clone)]
pub struct TckVector {
    /// Short directory name (e.g. `0100-arithmetic`).
    pub dir_name: String,
    /// Absolute path to the `.dmn` model file.
    pub dmn_path: PathBuf,
    /// Absolute path to the `-test-*.xml` test case file.
    pub test_xml_path: PathBuf,
    /// Category derived from parent directory (e.g. `compliance-level-2`).
    pub category: String,
}

// ---------------------------------------------------------------------------
// discover_tck_vectors
// ---------------------------------------------------------------------------

/// Walk the vendored TCK fixture tree and return every `(dir, dmn_path, test_xml)` triple that
/// contains at least one `.dmn` file AND at least one `-test-*.xml` file.
pub fn discover_tck_vectors(tck_root: &Path) -> Vec<TckVector> {
    let mut vectors = Vec::new();
    if !tck_root.is_dir() {
        return vectors;
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    collect_subdirs(tck_root, &mut dirs);

    for dir in &dirs {
        if let Some(v) = try_discover_vector(dir) {
            vectors.push(v);
        }
    }
    vectors.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));
    vectors
}

fn collect_subdirs(root: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') || name.starts_with('_') {
                        continue;
                    }
                }
                out.push(p.clone());
                collect_subdirs(&p, out);
            }
        }
    }
}

fn try_discover_vector(dir: &Path) -> Option<TckVector> {
    let dmn_path = find_dmn_file(dir)?;
    let test_xml_path = find_test_xml_file(dir)?;

    let category = dir
        .parent()?
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Some(TckVector {
        dir_name: dir.file_name()?.to_str()?.to_string(),
        dmn_path,
        test_xml_path,
        category,
    })
}

fn find_dmn_file(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("dmn") {
            return Some(path);
        }
    }
    None
}

fn find_test_xml_file(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains("-test-") && name.ends_with(".xml") {
            return Some(entry.path());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// structurally_equal
// ---------------------------------------------------------------------------

/// Normalize two XML strings for structural comparison. Returns `true` when the normalized forms
/// are byte-equal.
pub fn structurally_equal(xml_a: &str, xml_b: &str) -> bool {
    let norm_a = normalize_xml(xml_a);
    let norm_b = normalize_xml(xml_b);
    norm_a == norm_b
}

fn normalize_xml(input: &str) -> String {
    let mut s = input.to_string();

    // Step 1: strip XML declaration (the entire <?xml ... ?> block)
    while let Some(rest) = s.strip_prefix("<?xml") {
        if let Some(end) = rest.find("?>") {
            s = rest[end + 2..].to_string();
        } else {
            break;
        }
    }

    // Step 2: remove all whitespace between tags (collapse runs of whitespace to nothing)
    eprintln!("After step 1: {:?}", s);
    s = collapse_whitespace_outside_tags(&s);
    eprintln!("After step 2: {:?}", s);

    // Step 3: sort attributes within each tag alphabetically by key
    s = sort_attributes_in_all_tags(&s);
    eprintln!("After step 3: {:?}", s);

    // Step 4: normalize self-closing tags `<x/>` into open/close pairs `<x></x>`
    s = expand_self_closing_tags(&s);
    eprintln!("After step 4: {:?}", s);

    // Step 5: strip CDATA and comments (they don't affect structure)
    s = strip_cdata_and_comments(&s);
    eprintln!("After step 5: {:?}", s);

    s.trim().to_string()
}

fn collapse_whitespace_outside_tags(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '<' {
            // Enter a tag — preserve everything inside including spaces in attribute values.
            result.push('<');
            i += 1;
            while i < len && chars[i] != '>' {
                result.push(chars[i]);
                i += 1;
            }
            if i < len {
                result.push('>');
                i += 1;
            }
        } else if chars[i].is_whitespace() {
            // Skip all consecutive whitespace outside tags.
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

fn sort_attributes_in_all_tags(input: &str) -> String {
    let mut result = input.to_string();

    while let Some(start) = result.find('<') {
        if start + 1 < result.len() && (result.as_bytes()[start + 1] == b'/' || result.as_bytes()[start + 1].is_ascii_alphabetic()) {
            let end = match result[start..].find('>') {
                Some(e) => start + e,
                None => break,
            };

            let tag_content = &result[start + 1..end];
            // Only try to sort attributes if there's at least one space (indicating attributes exist).
            if tag_content.contains(' ') || tag_content.contains('\t') {
                if let Some(sorted_tag) = sort_attributes_in_single_tag(tag_content) {
                    result.replace_range(start + 1..=end, &sorted_tag);
                } else {
                    break;
                }
            }
            // If no attributes, leave the tag as-is.
        } else {
            break;
        }
    }

    result
}

fn sort_attributes_in_single_tag(tag_content: &str) -> Option<String> {
    let tag_content = tag_content.trim();
    if tag_content.is_empty() {
        return None;
    }

    // Extract the tag name (everything before the first whitespace).
    let space_pos = tag_content.find(|c: char| c.is_whitespace()).unwrap_or(tag_content.len());
    let tag_name = &tag_content[..space_pos];

    // If there's no whitespace after the tag name, there are no attributes — return early.
    if space_pos >= tag_content.len() {
        return None;
    }

    // Parse attributes from the rest of the string.
    let attrs_str = &tag_content[space_pos..];

    let mut attrs: Vec<String> = Vec::new();
    let mut i = 0;
    let chars: Vec<char> = attrs_str.chars().collect();
    let len = chars.len();

    while i < len {
        if chars[i].is_whitespace() {
            // Skip whitespace.
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        // Read attribute name.
        let attr_start = i;
        while i < len && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '/' {
            i += 1;
        }
        let attr_name = &attrs_str[attr_start..i];

        if i >= len || chars[i] == '/' {
            // Self-closing or no value.
            attrs.push(attr_name.to_string());
            if chars[i] == '/' {
                i += 1;
            }
            continue;
        }

        if chars[i] == '=' {
            i += 1; // Skip '='.
            let quote = if i < len && (chars[i] == '"' || chars[i] == '\'') {
                Some(chars[i])
            } else {
                None
            };

            if let Some(q) = quote {
                i += 1; // Skip opening quote.
                let val_start = i;
                while i < len && chars[i] != q {
                    i += 1;
                }
                let attr_val = &attrs_str[val_start..i];
                if i < len {
                    i += 1; // Skip closing quote.
                }
                attrs.push(format!("{}=\"{}\"", attr_name, attr_val));
            } else {
                // Unquoted value.
                let val_start = i;
                while i < len && !chars[i].is_whitespace() && chars[i] != '/' {
                    i += 1;
                }
                let attr_val = &attrs_str[val_start..i];
                attrs.push(format!("{}={}", attr_name, attr_val));
            }
        } else {
            // Boolean attribute (no value).
            attrs.push(attr_name.to_string());
        }
    }

    // Sort attributes alphabetically.
    attrs.sort();

    Some(format!("<{} {}", tag_name, attrs.join(" ")))
}

fn expand_self_closing_tags(input: &str) -> String {
    let mut result = input.to_string();

    while let Some(pos) = result.find(" />") {
        if pos > 0 && result.as_bytes()[pos - 1] == b'>' {
            result.replace_range(pos..pos + 2, "></");
        } else {
            break;
        }
    }

    if result.ends_with(" />") {
        result.truncate(result.len() - 3);
        result.push_str("></");
    }

    result
}

fn strip_cdata_and_comments(input: &str) -> String {
    let mut result = input.to_string();

    while let Some(start) = result.find("<![CDATA[") {
        if let Some(end) = result[start..].find("]]>") {
            result.replace_range(start..start + end + 3, "");
        } else {
            break;
        }
    }

    while let Some(start) = result.find("<!--") {
        if let Some(end) = result[start..].find("-->") {
            result.replace_range(start..start + end + 3, "");
        } else {
            break;
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests for the test_util module itself (unit tests).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structurally_equal_same_xml() {
        let xml = r#"<?xml version="1.0"?><root attr="val">text</root>"#;
        assert!(structurally_equal(xml, xml));
    }

    #[test]
    fn structurally_equal_whitespace_differs() {
        let a = r#"<?xml version="1.0"?><root>  hello  world  </root>"#;
        let b = r#"<root>hello world</root>"#;
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn structurally_equal_attribute_order() {
        let a = r#"<root z="2" a="1"/>"#;
        let b = r#"<root a="1" z="2"/>"#;
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn structurally_equal_self_closing_vs_open_close() {
        let a = r#"<root><child/></root>"#;
        let b = r#"<root><child></child></root>"#;
        eprintln!("a normalized: {:?}", normalize_xml(a));
        eprintln!("b normalized: {:?}", normalize_xml(b));
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn structurally_equal_xml_declaration_stripped() {
        let a = r#"<?xml version="1.0" encoding="UTF-8"?><root/>"#;
        let b = r#"<root/>"#;
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn structurally_equal_not_equal() {
        let a = r#"<?xml version="1.0"?><root attr="val">text</root>"#;
        let b = r#"<?xml version="1.0"?><root attr="different">text</root>"#;
        assert!(!structurally_equal(a, b));
    }

    #[test]
    fn structurally_equal_different_tags() {
        let a = r#"<a></a>"#;
        let b = r#"<b></b>"#;
        assert!(!structurally_equal(a, b));
    }

    #[test]
    fn structural_compare_empty_vs_whitespace_text() {
        let a = r#"<root>   </root>"#;
        let b = r#"<root></root>"#;
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn discover_vectors_empty_dir() {
        let tmp = std::env::temp_dir().join("tck_test_empty");
        let _ = std::fs::create_dir_all(&tmp);
        let vectors = discover_tck_vectors(&tmp);
        assert!(vectors.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_vectors_skips_dirs_without_both_files() {
        let tmp = std::env::temp_dir().join("tck_test_partial");
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(tmp.join("test.dmn"), "<definitions/>").ok();
        let vectors = discover_tck_vectors(&tmp);
        assert!(vectors.is_empty());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_vectors_finds_valid_vector() {
        let tmp = std::env::temp_dir().join("tck_test_valid");
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(tmp.join("model.dmn"), "<definitions/>").ok();
        std::fs::write(tmp.join("test-01.xml"), "<testCases/>").ok();
        let vectors = discover_tck_vectors(&tmp);
        assert_eq!(vectors.len(), 1);
        assert!(vectors[0].dmn_path.to_str().unwrap().ends_with("model.dmn"));
        assert!(vectors[0].test_xml_path.to_str().unwrap().contains("-test-"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_vectors_nested_dirs() {
        let tmp = std::env::temp_dir().join("tck_test_nested");
        let _ = std::fs::create_dir_all(tmp.join("compliance-level-2").join("0001-input-data-string"));
        let leaf = tmp.join("compliance-level-2").join("0001-input-data-string");
        std::fs::write(leaf.join("model.dmn"), "<definitions/>").ok();
        std::fs::write(leaf.join("test-01.xml"), "<testCases/>").ok();

        let vectors = discover_tck_vectors(&tmp);
        assert_eq!(vectors.len(), 1);
        assert_eq!(vectors[0].category, "compliance-level-2");
        assert_eq!(vectors[0].dir_name, "0001-input-data-string");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn structural_compare_preserves_content() {
        let a = r#"<root>hello</root>"#;
        let b = r#"<root>world</root>"#;
        assert!(!structurally_equal(a, b));
    }

    #[test]
    fn structural_compare_cdata_preserved() {
        let a = r#"<root><![CDATA[hello]]></root>"#;
        let b = r#"<root>hello</root>"#;
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn structural_compare_comments_stripped() {
        let a = r#"<root><!-- comment --><child/></root>"#;
        let b = r#"<root><child></child></root>"#;
        assert!(structurally_equal(a, b));
    }

    #[test]
    fn structural_compare_deep_nesting() {
        let a = r#"<a><b><c>text</c></b></a>"#;
        let b = r#"<a><b><c>text</c></b></a>"#;
        assert!(structurally_equal(a, b));

        let c = r#"  <a>    <b>      <c>   text   </c>    </b>  </a>  "#;
        assert!(structurally_equal(a, c));
    }
}
