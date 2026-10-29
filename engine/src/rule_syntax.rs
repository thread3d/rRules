//! The text form of a rule — `when <fact>.<field> <op> <value> then log("…")`.
//!
//! `specs/rule-syntax.md` is the grammar; this is the implementation. The parser is deliberately a
//! **faithful surface for [`Condition`] and [`Action`]** and nothing more: everything it accepts has a direct
//! representation in the AST, so text and structure are two spellings of one object rather than two objects
//! that can drift.
//!
//! ```text
//! rule "high-status-customer"
//! when customer.status = "premium"
//! and customer.score >= 100
//! then log("High status customer detected")
//! ```

use std::collections::HashMap;

use crate::data_types::Value;
use crate::rules::{Action, ComparisonOp, Condition, LogLevel, Rule};

/// What went wrong, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSyntaxError {
    /// 1-based line number.
    pub line: usize,
    /// What the parser expected or rejected.
    pub message: String,
}

impl std::fmt::Display for RuleSyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for RuleSyntaxError {}

/// The longest body this parser will look at, mirroring the FEEL parser's token budget: a rules file is
/// data, and data must not be able to spend unbounded time in a parser.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// Parse one rule from its text form.
///
/// The returned rule's `id` equals its `name` — a placeholder, because the API imposes the document's
/// identity on every rule it stores (`write_executable_rule`). Nothing downstream may rely on it.
pub fn parse_rule_text(text: &str) -> Result<Rule, RuleSyntaxError> {
    if text.len() > MAX_BODY_BYTES {
        return Err(RuleSyntaxError {
            line: 0,
            message: format!("the rule body exceeds {MAX_BODY_BYTES} bytes"),
        });
    }

    let mut name: Option<String> = None;
    let mut condition: Option<Condition> = None;
    let mut joiner: Option<&'static str> = None;
    let mut action: Option<Action> = None;
    let mut salience = 0;
    let mut no_loop = false;
    let mut lock_on_active = false;

    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();

        // Blank lines and comments are ignored; a body is prose as much as code.
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        let (keyword, rest) = split_keyword(line);
        match keyword.as_str() {
            "rule" => {
                if name.is_some() {
                    return Err(err(
                        line_number,
                        "a rule has one name; this is the second `rule` line",
                    ));
                }
                name = Some(parse_title(rest, line_number)?);
            }
            "salience" => {
                salience = rest.trim().parse::<i32>().map_err(|_| {
                    err(
                        line_number,
                        "`salience` takes a whole number between -1000 and 1000",
                    )
                })?;
            }
            "no_loop" | "no-loop" => no_loop = parse_bool_flag(rest, line_number)?,
            "lock_on_active" => lock_on_active = parse_bool_flag(rest, line_number)?,
            "when" => {
                if condition.is_some() {
                    return Err(err(
                        line_number,
                        "a rule has one `when`; use `and`/`or` lines to add conditions",
                    ));
                }
                condition = Some(parse_expr(rest, line_number)?);
            }
            "and" | "or" => {
                let Some(existing) = condition.take() else {
                    return Err(err(line_number, format!("`{keyword}` before any `when`")));
                };
                let word: &'static str = if keyword == "and" { "and" } else { "or" };
                if let Some(previous) = joiner {
                    // The grammar has no precedence rule, and inventing one silently would make two
                    // authors' readings of the same body differ.
                    if previous != word {
                        return Err(err(
                            line_number,
                            "mixing `and` and `or` in one rule is ambiguous; group with `not(...)` instead",
                        ));
                    }
                }
                joiner = Some(word);
                let next = parse_expr(rest, line_number)?;
                condition = Some(match existing {
                    Condition::And(mut parts) if word == "and" => {
                        parts.push(next);
                        Condition::And(parts)
                    }
                    Condition::Or(mut parts) if word == "or" => {
                        parts.push(next);
                        Condition::Or(parts)
                    }
                    other => {
                        if word == "and" {
                            Condition::And(vec![other, next])
                        } else {
                            Condition::Or(vec![other, next])
                        }
                    }
                });
            }
            "then" => {
                if action.is_some() {
                    return Err(err(line_number, "a rule has one `then`"));
                }
                action = Some(parse_action(rest, line_number)?);
            }
            other => {
                return Err(err(
                    line_number,
                    format!(
                    "unknown line `{other}`: a rule body is `rule`, `when`, `and`/`or`, or `then`"
                ),
                ))
            }
        }
    }

    let name = name.ok_or_else(|| err(0, "no `rule \"name\"` line"))?;
    let condition =
        condition.ok_or_else(|| err(0, format!("`rule \"{name}\"` has no `when` line")))?;
    let action = action.ok_or_else(|| err(0, format!("`rule \"{name}\"` has no `then` line")))?;

    Ok(Rule {
        id: name.clone(),
        name,
        salience,
        no_loop,
        lock_on_active,
        agenda_group: None,
        condition,
        actions: vec![action],
    })
}

fn err(line: usize, message: impl Into<String>) -> RuleSyntaxError {
    RuleSyntaxError {
        line,
        message: message.into(),
    }
}

/// Split a line into its leading keyword and the rest.
fn split_keyword(line: &str) -> (String, &str) {
    let end = line
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(line.len());
    (line[..end].to_string(), &line[end..])
}

fn parse_bool_flag(rest: &str, line: usize) -> Result<bool, RuleSyntaxError> {
    match rest.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        "" => Ok(true),
        other => Err(err(
            line,
            format!("expected `true` or `false` after the flag, found `{other}`"),
        )),
    }
}

fn parse_title(rest: &str, line: usize) -> Result<String, RuleSyntaxError> {
    let trimmed = rest.trim();
    let (value, leftover) = parse_quoted(trimmed, line)?;
    if !leftover.trim().is_empty() {
        return Err(err(
            line,
            format!("unexpected `{}` after the rule name", leftover.trim()),
        ));
    }
    if value.is_empty() {
        return Err(err(line, "a rule needs a name"));
    }
    Ok(value)
}

/// A comparison, or a `not(...)` around one.
fn parse_expr(rest: &str, line: usize) -> Result<Condition, RuleSyntaxError> {
    let text = rest.trim();
    if let Some(inner) = text.strip_prefix("not") {
        let inner = inner.trim();
        let body = inner
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or_else(|| {
                err(
                    line,
                    "`not` takes a parenthesised condition, e.g. `not(a.b = 1)`",
                )
            })?;
        return Ok(Condition::Not(Box::new(parse_expr(body, line)?)));
    }

    // `fact.field op value` — the field may contain spaces only if the whole reference is quoted, which the
    // grammar does not allow; a name with a space is a naming decision for the model, not a parsing one.
    let (reference, rest) = split_once_whitespace(text)
        .ok_or_else(|| err(line, "expected `<fact>.<field> <operator> <value>`"))?;
    let (fact_type, field) = reference.split_once('.').ok_or_else(|| {
        err(
            line,
            format!("`{reference}` must name a field as `<fact>.<field>`"),
        )
    })?;
    if fact_type.is_empty() || field.is_empty() {
        return Err(err(
            line,
            format!("`{reference}` must name a field as `<fact>.<field>`"),
        ));
    }

    let (operator, rest) = split_once_whitespace(rest.trim())
        .ok_or_else(|| err(line, "expected a comparison operator after the field"))?;
    let op = match operator {
        "=" | "==" => ComparisonOp::Eq,
        "!=" | "<>" => ComparisonOp::Ne,
        ">" => ComparisonOp::Gt,
        ">=" => ComparisonOp::Ge,
        "<" => ComparisonOp::Lt,
        "<=" => ComparisonOp::Le,
        other => {
            return Err(err(
                line,
                format!("`{other}` is not a comparison operator (= != > >= < <=)"),
            ))
        }
    };

    let value = parse_literal(rest.trim(), line)?;
    Ok(Condition::Pattern {
        fact_type: fact_type.to_string(),
        field: field.to_string(),
        value,
        op,
    })
}

fn parse_action(rest: &str, line: usize) -> Result<Action, RuleSyntaxError> {
    let text = rest.trim();
    let (keyword, remainder) = split_keyword(text);
    match keyword.as_str() {
        "log" | "logmessage" => {
            let argument = remainder.trim();
            let inner = argument
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .ok_or_else(|| err(line, "`log` takes a quoted message, e.g. `log(\"fired\")`"))?;
            let (message, leftover) = parse_quoted(inner.trim(), line)?;
            if !leftover.trim().is_empty() {
                return Err(err(
                    line,
                    format!("unexpected `{}` after the message", leftover.trim()),
                ));
            }
            Ok(Action::LogMessage {
                level: LogLevel::Info,
                message,
            })
        }
        other => Err(err(
            line,
            format!(
                "`{other}` is not an action this syntax carries; only `log(\"…\")` is, because it is the \
                 one the interface produces (specs/rule-syntax.md)"
            ),
        )),
    }
}

fn parse_literal(text: &str, line: usize) -> Result<Value, RuleSyntaxError> {
    if text.is_empty() {
        return Err(err(line, "expected a value after the operator"));
    }
    if text.starts_with('"') {
        let (value, leftover) = parse_quoted(text, line)?;
        if !leftover.trim().is_empty() {
            return Err(err(
                line,
                format!("unexpected `{}` after the value", leftover.trim()),
            ));
        }
        return Ok(Value::String(value));
    }
    match text {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    if let Ok(integer) = text.parse::<i64>() {
        return Ok(Value::Integer(integer));
    }
    if let Ok(float) = text.parse::<f64>() {
        return Ok(Value::Float(float));
    }
    Err(err(
        line,
        format!("`{text}` is not a value: quote text, or write a number or true/false"),
    ))
}

/// A double-quoted string with `\"` and `\\` escapes, and the text after it.
fn parse_quoted(text: &str, line: usize) -> Result<(String, &str), RuleSyntaxError> {
    let mut chars = text.char_indices();
    let Some((_, opener)) = chars.next() else {
        return Err(err(line, "expected a quoted string"));
    };
    if opener != '"' {
        return Err(err(
            line,
            format!("expected a quoted string, found `{opener}`"),
        ));
    }

    let mut value = String::new();
    while let Some((index, character)) = chars.next() {
        match character {
            '\\' => match chars.next() {
                Some((_, escaped @ ('"' | '\\'))) => value.push(escaped),
                Some((_, other)) => {
                    return Err(err(
                        line,
                        format!(
                            "`\\{other}` is not an escape this syntax knows (use \\\" or \\\\)"
                        ),
                    ))
                }
                None => return Err(err(line, "a string cannot end with a lone `\\`")),
            },
            '"' => return Ok((value, &text[index + 1..])),
            other => value.push(other),
        }
    }
    Err(err(line, "unterminated string"))
}

fn split_once_whitespace(text: &str) -> Option<(&str, &str)> {
    let index = text.find(char::is_whitespace)?;
    Some((&text[..index], &text[index..]))
}

/// Render a rule as this syntax. The inverse of [`parse_rule_text`] for everything this grammar carries.
///
/// `specs/rule-syntax.md` requires `parse(render(rule)) == rule` for every rule the syntax can express, and
/// `engine/tests/rule_syntax.rs` holds that as a test over the rules the interface builds.
pub fn render_rule_text(rule: &Rule) -> String {
    let mut lines = vec![format!("rule {}", quote(&rule.name))];
    if rule.salience != 0 {
        lines.push(format!("salience {}", rule.salience));
    }
    if rule.no_loop {
        lines.push("no_loop true".to_string());
    }
    if rule.lock_on_active {
        lines.push("lock_on_active true".to_string());
    }
    match render_condition(&rule.condition) {
        Some(text) => lines.push(text),
        None => lines.push("when false".to_string()),
    }
    for action in &rule.actions {
        match action {
            Action::LogMessage { message, .. } => {
                lines.push(format!("then log({})", quote(message)))
            }
            // An action this syntax cannot spell is named rather than dropped: a rendering that silently
            // omits half a rule is the defect that made `dsl_body` untrustworthy in the first place.
            other => lines.push(format!(
                "// action not expressible in this syntax: {other:?}"
            )),
        }
    }
    lines.join("\n")
}

fn render_condition(condition: &Condition) -> Option<String> {
    match condition {
        Condition::True => Some("when true".to_string()),
        Condition::False => Some("when false".to_string()),
        Condition::Pattern {
            fact_type,
            field,
            value,
            op,
        } => Some(format!(
            "when {fact_type}.{field} {} {}",
            render_op(*op),
            render_value(value)
        )),
        Condition::Not(inner) => render_condition(inner).map(|text| {
            let body = text.strip_prefix("when ").unwrap_or(&text);
            format!("when not({body})")
        }),
        Condition::And(parts) => render_chain(parts, "and"),
        Condition::Or(parts) => render_chain(parts, "or"),
        // `Compare` has no surface: the grammar has no expressions, so a rule using it cannot be rendered
        // as text and the caller is told so by an empty rendering rather than a wrong one.
        Condition::Compare { .. } => None,
    }
}

fn render_chain(parts: &[Condition], joiner: &str) -> Option<String> {
    let mut rendered = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let text = render_condition(part)?;
        let body = text.strip_prefix("when ").unwrap_or(&text).to_string();
        rendered.push(if index == 0 {
            format!("when {body}")
        } else {
            format!("{joiner} {body}")
        });
    }
    Some(rendered.join("\n"))
}

fn render_op(op: ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Eq => "=",
        ComparisonOp::Ne => "!=",
        ComparisonOp::Gt => ">",
        ComparisonOp::Ge => ">=",
        ComparisonOp::Lt => "<",
        ComparisonOp::Le => "<=",
    }
}

fn render_value(value: &Value) -> String {
    match value {
        Value::String(text) => quote(text),
        Value::Integer(integer) => integer.to_string(),
        Value::Float(float) => {
            // `1.0` must not render as `1`, or the rendering would claim an integer.
            if float.fract() == 0.0 {
                format!("{float:.1}")
            } else {
                float.to_string()
            }
        }
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(decimal) => decimal.to_string(),
        other => format!("{other:?}"),
    }
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The facts a rule reads, by fact type — used by the API to explain what a rule needs.
pub fn referenced_fact_types(rule: &Rule) -> Vec<String> {
    fn walk(condition: &Condition, found: &mut Vec<String>) {
        match condition {
            Condition::Pattern { fact_type, .. } => {
                if !found.iter().any(|existing| existing == fact_type) {
                    found.push(fact_type.clone());
                }
            }
            Condition::And(parts) | Condition::Or(parts) => {
                for part in parts {
                    walk(part, found);
                }
            }
            Condition::Not(inner) => walk(inner, found),
            Condition::Compare { left, right, .. } => {
                walk(left, found);
                walk(right, found);
            }
            Condition::True | Condition::False => {}
        }
    }

    let mut found = Vec::new();
    walk(&rule.condition, &mut found);
    found
}

/// Parse several rules from one body, separated by blank lines.
///
/// A rules *file* holds several; the grammar's `rule` line starts each one. Returns the rules it parsed and
/// the line each started on, so a caller can point at the one that failed.
pub fn parse_rules_text(text: &str) -> Result<Vec<Rule>, RuleSyntaxError> {
    let mut chunks: Vec<(usize, Vec<&str>)> = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        let starts_rule = trimmed.starts_with("rule ") || trimmed == "rule";
        if starts_rule || chunks.is_empty() {
            chunks.push((index + 1, Vec::new()));
        }
        if let Some((_, chunk)) = chunks.last_mut() {
            chunk.push(line);
        }
    }

    let mut rules = Vec::new();
    for (line_number, chunk) in chunks {
        if chunk.iter().all(|line| {
            let trimmed = line.trim();
            trimmed.is_empty() || trimmed.starts_with("//")
        }) {
            continue;
        }
        let body = chunk.join("\n");
        rules.push(parse_rule_text(&body).map_err(|error| RuleSyntaxError {
            line: error.line + line_number - 1,
            message: error.message,
        })?);
    }
    Ok(rules)
}

/// Convenience for tests and callers that want a map of the fact types a rule reads.
pub fn referenced_fact_types_in(rule: &Rule) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for fact_type in referenced_fact_types(rule) {
        *counts.entry(fact_type).or_insert(0) += 1;
    }
    counts
}
