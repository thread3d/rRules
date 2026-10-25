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
    let mut s = input.trim().to_string();

    // Step 1: strip the XML declaration (the entire <?xml ... ?> block).
    while let Some(rest) = s.strip_prefix("<?xml") {
        if let Some(end) = rest.find("?>") {
            s = rest[end + 2..].trim_start().to_string();
        } else {
            break;
        }
    }

    // Step 2: drop comments and unwrap CDATA sections so `<![CDATA[x]]>` and
    // plain `x` compare equal (CDATA is only an escaping device).
    s = unwrap_cdata_and_strip_comments(&s);

    // Step 3: remove whitespace outside tags.
    s = collapse_whitespace_outside_tags(&s);

    // Step 4: canonicalize tags — sort attributes by key and rewrite self-closing
    // tags as open/close pairs so `<x/>` and `<x></x>` compare equal.
    s = canonicalize_tags(&s);

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

/// Rewrite every tag into canonical form: attributes sorted by name, whitespace
/// inside the tag collapsed to single spaces, and self-closing tags expanded to
/// open/close pairs.
///
/// This replaces the previous implementation, which looped forever on the first
/// tag that had no attributes (it never advanced the scan position) and rebuilt
/// tags without their closing `>`.
fn canonicalize_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;

    while let Some(lt) = rest.find('<') {
        // Text before the tag: copy verbatim.
        out.push_str(&rest[..lt]);
        let after = &rest[lt + 1..];

        // Declarations, doctypes, CDATA and comments are copied verbatim.
        if after.starts_with('!') || after.starts_with('?') {
            match after.find('>') {
                Some(gt) => {
                    out.push('<');
                    out.push_str(&after[..gt]);
                    out.push('>');
                    rest = &after[gt + 1..];
                }
                None => {
                    out.push('<');
                    out.push_str(after);
                    rest = "";
                }
            }
            continue;
        }

        // End tags: `<name>` (whitespace inside end tags is irrelevant).
        if let Some(name) = after.strip_prefix('/') {
            match name.find('>') {
                Some(gt) => {
                    out.push_str("</");
                    out.push_str(name[..gt].trim());
                    out.push('>');
                    rest = &name[gt + 1..];
                }
                None => {
                    out.push_str("</");
                    out.push_str(name);
                    rest = "";
                }
            }
            continue;
        }

        // Start or self-closing tag.
        match after.find('>') {
            Some(gt) => {
                let raw_tag = &after[..gt];
                let trimmed = raw_tag.trim_end();
                let self_closing = trimmed.ends_with('/');
                let body = if self_closing {
                    trimmed[..trimmed.len() - 1].trim_end()
                } else {
                    trimmed
                };

                match sort_attributes_in_single_tag(body) {
                    Some((name, attrs)) => {
                        out.push('<');
                        out.push_str(&name);
                        if !attrs.is_empty() {
                            out.push(' ');
                            out.push_str(&attrs.join(" "));
                        }
                        if self_closing {
                            out.push_str("></");
                            out.push_str(&name);
                            out.push('>');
                        } else {
                            out.push('>');
                        }
                    }
                    None => {
                        // Malformed/unrecognized tag shape — copy verbatim.
                        out.push('<');
                        out.push_str(raw_tag);
                        out.push('>');
                    }
                }
                rest = &after[gt + 1..];
            }
            None => {
                out.push('<');
                out.push_str(after);
                rest = "";
            }
        }
    }

    out.push_str(rest);
    out
}

/// Split a tag body (`name attr="v" other='w'`) into its name and a sorted list
/// of canonical `attr="value"` strings.
///
/// Returns `None` only when the body is empty or has no tag name.
fn sort_attributes_in_single_tag(tag_content: &str) -> Option<(String, Vec<String>)> {
    let tag_content = tag_content.trim();
    if tag_content.is_empty() {
        return None;
    }

    let name_end = tag_content
        .find(|c: char| c.is_whitespace())
        .unwrap_or(tag_content.len());
    let name = tag_content[..name_end].to_string();
    if name.is_empty() {
        return None;
    }

    let attrs_str = &tag_content[name_end..];
    let chars: Vec<char> = attrs_str.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut attrs: Vec<String> = Vec::new();

    while i < len {
        // Skip whitespace and any stray separators.
        if chars[i].is_whitespace() || chars[i] == '/' {
            i += 1;
            continue;
        }

        // Attribute name.
        let attr_start = i;
        while i < len && !chars[i].is_whitespace() && chars[i] != '=' {
            i += 1;
        }
        let attr_name: String = chars[attr_start..i].iter().collect();
        if attr_name.is_empty() {
            i += 1;
            continue;
        }

        // Optional `= value`.
        if i < len && chars[i] == '=' {
            i += 1;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            if i < len && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let val_start = i;
                while i < len && chars[i] != quote {
                    i += 1;
                }
                let value: String = chars[val_start..i].iter().collect();
                if i < len {
                    i += 1; // closing quote
                }
                attrs.push(format!("{}=\"{}\"", attr_name, value));
            } else {
                let val_start = i;
                while i < len && !chars[i].is_whitespace() {
                    i += 1;
                }
                let value: String = chars[val_start..i].iter().collect();
                attrs.push(format!("{}=\"{}\"", attr_name, value));
            }
        } else {
            // Valueless attribute.
            attrs.push(attr_name);
        }
    }

    attrs.sort();
    Some((name, attrs))
}

fn unwrap_cdata_and_strip_comments(input: &str) -> String {
    let mut result = input.to_string();

    // Unwrap `<![CDATA[content]]>` to `content`.
    const CDATA_OPEN: &str = "<![CDATA[";
    while let Some(start) = result.find(CDATA_OPEN) {
        if let Some(end) = result[start..].find("]]>") {
            let content_start = start + CDATA_OPEN.len();
            let content_end = start + end;
            let content = result[content_start..content_end].to_string();
            result.replace_range(start..content_end + 3, &content);
        } else {
            break;
        }
    }

    // Remove comments entirely.
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
        // Vector directories are discovered one level below the TCK root and
        // follow the upstream naming convention (`<vector>.dmn` +
        // `<vector>-test-NN.xml`) — see `engine/tests/fixtures/tck/`.
        let tmp = std::env::temp_dir().join("tck_test_valid");
        let _ = std::fs::remove_dir_all(&tmp);
        let vector_dir = tmp.join("0001-input-data-string");
        let _ = std::fs::create_dir_all(&vector_dir);
        std::fs::write(vector_dir.join("0001-input-data-string.dmn"), "<definitions/>").ok();
        std::fs::write(
            vector_dir.join("0001-input-data-string-test-01.xml"),
            "<testCases/>",
        )
        .ok();
        let vectors = discover_tck_vectors(&tmp);
        assert_eq!(vectors.len(), 1);
        assert!(vectors[0]
            .dmn_path
            .to_str()
            .unwrap()
            .ends_with("0001-input-data-string.dmn"));
        assert!(vectors[0].test_xml_path.to_str().unwrap().contains("-test-"));
        assert_eq!(vectors[0].dir_name, "0001-input-data-string");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discover_vectors_nested_dirs() {
        let tmp = std::env::temp_dir().join("tck_test_nested");
        let _ = std::fs::remove_dir_all(&tmp);
        let leaf = tmp.join("compliance-level-2").join("0001-input-data-string");
        let _ = std::fs::create_dir_all(&leaf);
        std::fs::write(leaf.join("0001-input-data-string.dmn"), "<definitions/>").ok();
        std::fs::write(
            leaf.join("0001-input-data-string-test-01.xml"),
            "<testCases/>",
        )
        .ok();

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
