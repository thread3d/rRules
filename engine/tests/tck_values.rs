//! Typed value parsing and deep comparison for OMG DMN TCK test-case XML.
//!
//! The TCK expresses expected results as nested XML (DMN 1.5 Clause 10.2 test
//! cases): `<value xsi:type="xsd:decimal">1</value>`, `<list><item>…</item></list>`,
//! and `<component name="k">…</component>` for contexts. This module parses that
//! subset into plain [`TckNode`] trees and converts them to engine
//! [`Value`]s so the sweep can compare an evaluator result against the expected
//! result **by structure** instead of by the first scalar it happens to find.
//!
//! Only the shapes that appear in the vendored corpus are supported; anything
//! unrecognised degrades to a typed scalar carrying its raw text, which makes
//! a mismatch visible rather than silently passing.

use engine::data_types::Value;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

/// A parsed value node from a TCK test XML file.
#[derive(Debug, Clone, PartialEq)]
pub enum TckNode {
    /// `xsi:nil="true"` — an explicit null.
    Null,
    /// `<value xsi:type="…">text</value>`; `xsi:type` is kept verbatim (for
    /// example `xsd:decimal`).
    Scalar {
        xsi_type: Option<String>,
        text: String,
    },
    /// `<list>` of `<item>` children.
    List(Vec<TckNode>),
    /// A context built from `<component name="k">` children.
    Context(Vec<(String, TckNode)>),
}

/// One `<testCase>` from a TCK test XML file.
#[derive(Debug, Clone)]
pub struct TckCase {
    pub id: String,
    pub description: Option<String>,
    /// `(input node name, value)` pairs.
    pub inputs: Vec<(String, TckNode)>,
    /// `(result node name, expected value)` pairs.
    pub expected: Vec<(String, TckNode)>,
    /// True when the case declares `errorResult="true"` (a null result is
    /// expected because the expression is invalid).
    pub expects_error: bool,
}

// =========================================================================
// XML subset parser
// =========================================================================

/// One tag in the flat token stream.
enum Tag {
    Open { name: String, attrs: Vec<(String, String)> },
    Close { name: String },
    Text(String),
}

fn tokenize(xml: &str) -> Vec<Tag> {
    let mut tags = Vec::new();
    let bytes = xml.as_bytes();
    let mut i = 0;

    while i < xml.len() {
        if bytes[i] == b'<' {
            // Comments and processing instructions are skipped wholesale.
            if xml[i..].starts_with("<!--") {
                match xml[i..].find("-->") {
                    Some(end) => {
                        i += end + 3;
                        continue;
                    }
                    None => break,
                }
            }
            if xml[i..].starts_with("<?") || xml[i..].starts_with("<!") {
                match xml[i..].find('>') {
                    Some(end) => {
                        i += end + 1;
                        continue;
                    }
                    None => break,
                }
            }

            let Some(end) = xml[i..].find('>') else { break };
            let raw = &xml[i + 1..i + end];
            i += end + 1;

            if let Some(name) = raw.strip_prefix('/') {
                tags.push(Tag::Close {
                    name: name.trim().to_string(),
                });
            } else {
                let self_closing = raw.ends_with('/');
                let body = raw.trim_end_matches('/').trim();
                let (name, attrs) = parse_tag_attrs(body);
                tags.push(Tag::Open {
                    name: name.clone(),
                    attrs,
                });
                if self_closing {
                    tags.push(Tag::Close { name });
                }
            }
        } else {
            let next = xml[i..].find('<').map(|o| i + o).unwrap_or(xml.len());
            tags.push(Tag::Text(xml[i..next].to_string()));
            i = next;
        }
    }

    tags
}

fn parse_tag_attrs(body: &str) -> (String, Vec<(String, String)>) {
    let name_end = body.find(char::is_whitespace).unwrap_or(body.len());
    let name = body[..name_end].to_string();
    let mut attrs = Vec::new();
    let rest = &body[name_end..];
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_whitespace() || chars[i] == '/' {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if key.is_empty() {
            i += 1;
            continue;
        }
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let val_start = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                let value: String = chars[val_start..i].iter().collect();
                if i < chars.len() {
                    i += 1;
                }
                attrs.push((key, value));
            } else {
                let val_start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                let value: String = chars[val_start..i].iter().collect();
                attrs.push((key, value));
            }
        } else {
            attrs.push((key, String::new()));
        }
    }

    (name, attrs)
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k == name || k.ends_with(&format!(":{name}")))
        .map(|(_, v)| v.as_str())
}

// =========================================================================
// Test-case parsing
// =========================================================================

/// Index of the `</name>` tag matching the `<name>` tag at `open`.
fn find_close(tags: &[Tag], open: usize, name: &str) -> usize {
    let mut depth = 0usize;
    for (k, tag) in tags.iter().enumerate().skip(open) {
        match tag {
            Tag::Open { name: n, .. } if n == name => depth += 1,
            Tag::Close { name: n } if n == name => {
                depth -= 1;
                if depth == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    tags.len()
}

/// Parse every `<testCase>` in a TCK test XML document.
pub fn parse_test_cases(xml: &str) -> Vec<TckCase> {
    let tags = tokenize(xml);
    let mut cases = Vec::new();
    let mut i = 0;

    while i < tags.len() {
        let Tag::Open { name, attrs } = &tags[i] else {
            i += 1;
            continue;
        };
        if name != "testCase" {
            i += 1;
            continue;
        }

        let id = attr(attrs, "id").unwrap_or("").to_string();
        let close = find_close(&tags, i, "testCase");
        let body = &tags[i + 1..close.min(tags.len())];

        let mut case = TckCase {
            id,
            description: None,
            inputs: Vec::new(),
            expected: Vec::new(),
            expects_error: false,
        };

        let mut j = 0;
        while j < body.len() {
            let Tag::Open { name, attrs } = &body[j] else {
                j += 1;
                continue;
            };
            match name.as_str() {
                "inputNode" | "inputValues" => {
                    let node_name = attr(attrs, "name").unwrap_or("").to_string();
                    let end = find_close(body, j, name);
                    let value = parse_container(&body[j + 1..end.min(body.len())]);
                    if !node_name.is_empty() {
                        case.inputs.push((node_name, value));
                    }
                    j = end + 1;
                }
                "resultNode" => {
                    let node_name = attr(attrs, "name").unwrap_or("").to_string();
                    if attr(attrs, "errorResult").is_some_and(|v| v == "true") {
                        case.expects_error = true;
                    }
                    let end = find_close(body, j, name);
                    let value = parse_container(&body[j + 1..end.min(body.len())]);
                    case.expected.push((node_name, value));
                    j = end + 1;
                }
                "description" => {
                    if let Some(Tag::Text(text)) = body.get(j + 1) {
                        case.description = Some(text.trim().to_string());
                    }
                    j += 1;
                }
                _ => j += 1,
            }
        }

        cases.push(case);
        i = close + 1;
    }

    cases
}

/// Skip whitespace-only text and the TCK's structural wrapper tags.
fn skip_wrappers(tags: &[Tag], mut i: usize) -> usize {
    loop {
        match tags.get(i) {
            Some(Tag::Text(t)) if t.trim().is_empty() => i += 1,
            Some(Tag::Open { name, .. })
                if matches!(
                    name.as_str(),
                    "expected" | "inputNode" | "inputValues" | "resultNode"
                ) =>
            {
                i += 1
            }
            _ => return i,
        }
    }
}

/// Parse the value found at `idx` (a value-bearing tag).
fn parse_node(tags: &[Tag], idx: usize) -> TckNode {
    match tags.get(idx) {
        Some(Tag::Open { name, attrs }) if name == "value" => {
            if attr(attrs, "nil").is_some_and(|v| v == "true") {
                return TckNode::Null;
            }
            let text = match tags.get(idx + 1) {
                Some(Tag::Text(t)) => t.trim().to_string(),
                _ => String::new(),
            };
            TckNode::Scalar {
                xsi_type: attr(attrs, "type").map(|t| t.to_string()),
                text,
            }
        }
        Some(Tag::Open { name, .. }) if name == "list" => {
            let close = find_close(tags, idx, "list");
            let mut items = Vec::new();
            let mut k = idx + 1;
            while k < close {
                if let Tag::Open { name: n, .. } = &tags[k] {
                    if n == "item" {
                        let item_close = find_close(tags, k, "item");
                        items.push(parse_container(&tags[k + 1..item_close.min(tags.len())]));
                        k = item_close + 1;
                        continue;
                    }
                }
                k += 1;
            }
            TckNode::List(items)
        }
        Some(Tag::Open { name, .. }) if name == "context" || name == "relation" => {
            let close = find_close(tags, idx, name);
            parse_container(&tags[idx + 1..close.min(tags.len())])
        }
        _ => TckNode::Null,
    }
}

/// Parse a container body: either a single value/list, or a run of
/// `<component name="…">` entries (which form a context).
fn parse_container(body: &[Tag]) -> TckNode {
    let k = skip_wrappers(body, 0);

    if matches!(
        body.get(k),
        Some(Tag::Open { name, .. }) if name == "component" || name == "entry"
    ) {
        let mut entries = Vec::new();
        let mut j = k;
        while j < body.len() {
            if let Tag::Open { name, attrs } = &body[j] {
                if name == "component" || name == "entry" {
                    let key = attr(attrs, "name").unwrap_or("").to_string();
                    let close = find_close(body, j, name);
                    entries.push((key, parse_container(&body[j + 1..close.min(body.len())])));
                    j = close + 1;
                    continue;
                }
            }
            j += 1;
        }
        return TckNode::Context(entries);
    }

    parse_node(body, k)
}

// =========================================================================
// Value conversion + comparison
// =========================================================================

/// Convert a parsed TCK node into an engine [`Value`] for comparison.
pub fn to_value(node: &TckNode) -> Value {
    match node {
        TckNode::Null => Value::Null,
        TckNode::Scalar { xsi_type, text } => scalar_to_value(xsi_type.as_deref(), text),
        TckNode::List(items) => Value::Array(items.iter().map(to_value).collect()),
        TckNode::Context(entries) => {
            let mut map = HashMap::new();
            for (k, v) in entries {
                map.insert(k.clone(), to_value(v));
            }
            Value::Object(map)
        }
    }
}

fn scalar_to_value(xsi_type: Option<&str>, text: &str) -> Value {
    let text = text.trim();
    let short = xsi_type.map(|t| t.rsplit(':').next().unwrap_or(t)).unwrap_or("");

    match short {
        "string" => Value::String(text.to_string()),
        "boolean" | "bool" => match text {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::Null,
        },
        "decimal" | "double" | "float" | "integer" | "int" | "long" => match Decimal::from_str(text) {
            Ok(d) => Value::Number(d),
            Err(_) => Value::String(text.to_string()),
        },
        "date" => match chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            Ok(d) => Value::Date(d),
            Err(_) => Value::String(text.to_string()),
        },
        _ => {
            // No type information: infer from the text.
            if text.eq_ignore_ascii_case("null") || text.is_empty() {
                return Value::Null;
            }
            if text == "true" {
                return Value::Bool(true);
            }
            if text == "false" {
                return Value::Bool(false);
            }
            match Decimal::from_str(text) {
                Ok(d) => Value::Number(d),
                Err(_) => Value::String(text.to_string()),
            }
        }
    }
}

/// Structural equality with the tolerance a conformance suite needs: numbers
/// compare by value (not scale), strings compare exactly, lists/contexts
/// compare element-wise.
pub fn deep_equal(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Null, Value::Null) => true,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Integer(a), Value::Integer(b)) => a == b,
        (Value::Number(a), Value::Integer(b)) => *a == Decimal::from(*b),
        (Value::Integer(a), Value::Number(b)) => Decimal::from(*a) == *b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Date(a), Value::Date(b)) => a == b,
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| deep_equal(x, y))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|other| deep_equal(v, other)))
        }
        // Lists of one element may appear where a scalar is expected (and vice
        // versa) because DMN flattens singleton lists in results.
        (Value::Array(a), other) if a.len() == 1 => deep_equal(&a[0], other),
        (other, Value::Array(b)) if b.len() == 1 => deep_equal(other, &b[0]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<testCases xmlns="http://www.omg.org/spec/DMN/20160719/testcase">
  <testCase id="case_001">
    <description>sorted list</description>
    <inputNode name="listA">
      <list>
        <item><value xsi:type="xsd:decimal">3</value></item>
        <item><value xsi:type="xsd:decimal">1</value></item>
      </list>
    </inputNode>
    <resultNode name="sort1" type="decision">
      <expected>
        <list>
          <item><value xsi:type="xsd:decimal">1</value></item>
          <item><value xsi:type="xsd:decimal">3</value></item>
        </list>
      </expected>
    </resultNode>
  </testCase>
  <testCase id="case_002">
    <resultNode name="err" type="decision" errorResult="true">
      <expected>
        <value xsi:nil="true"></value>
      </expected>
    </resultNode>
  </testCase>
</testCases>"#;

    #[test]
    fn parses_inputs_and_expected_lists() {
        let cases = parse_test_cases(SAMPLE);
        assert_eq!(cases.len(), 2);

        let first = &cases[0];
        assert_eq!(first.id, "case_001");
        assert_eq!(first.description.as_deref(), Some("sorted list"));
        assert_eq!(first.inputs.len(), 1);
        assert_eq!(first.inputs[0].0, "listA");
        let value = to_value(&first.inputs[0].1);
        match value {
            Value::Array(items) => assert_eq!(items.len(), 2),
            other => panic!("expected array input, got {other:?}"),
        }
        assert_eq!(first.expected.len(), 1);
        assert_eq!(first.expected[0].0, "sort1");
    }

    #[test]
    fn parses_error_case() {
        let cases = parse_test_cases(SAMPLE);
        assert!(cases[1].expects_error);
        assert_eq!(to_value(&cases[1].expected[0].1), Value::Null);
    }

    #[test]
    fn deep_equal_compares_structure() {
        let a = to_value(&TckNode::List(vec![
            TckNode::Scalar {
                xsi_type: Some("xsd:decimal".into()),
                text: "1.0".into(),
            },
        ]));
        let b = to_value(&TckNode::List(vec![TckNode::Scalar {
            xsi_type: Some("xsd:decimal".into()),
            text: "1".into(),
        }]));
        assert!(deep_equal(&a, &b), "1.0 and 1 compare equal by value");

        let c = to_value(&TckNode::List(vec![
            TckNode::Scalar {
                xsi_type: Some("xsd:decimal".into()),
                text: "2".into(),
            },
        ]));
        assert!(!deep_equal(&a, &c), "different element values must differ");
    }

    #[test]
    fn parses_context_components() {
        let xml = r#"<testCase id="c">
          <resultNode name="r" type="decision">
            <expected><list><item>
              <component name="key"><value xsi:type="xsd:string">a</value></component>
              <component name="value"><value xsi:type="xsd:decimal">1</value></component>
            </item></list></expected>
          </resultNode>
        </testCase>"#;
        let cases = parse_test_cases(xml);
        assert_eq!(cases.len(), 1);
        let node = to_value(&cases[0].expected[0].1);
        match node {
            Value::Array(items) => {
                assert_eq!(items.len(), 1);
                match &items[0] {
                    Value::Object(map) => {
                        assert_eq!(map.get("key"), Some(&Value::String("a".into())));
                        assert_eq!(
                            map.get("value"),
                            Some(&Value::Number(Decimal::from(1)))
                        );
                    }
                    other => panic!("expected context item, got {other:?}"),
                }
            }
            other => panic!("expected list of contexts, got {other:?}"),
        }
    }
}
