//! FEEL lexer + hand-written recursive-descent + Pratt-precedence parser
//! (grammar rules 1–65).
//!
//! Plan 31-01 Task 2 ships the lexer (`Token`, `TokenSpan`, `Lexer`) and
//! Task 3 ships the recursive-descent + Pratt parser with dual entry
//! points (`parse_expression`, `parse_simple_unary_tests`), keyword-as-name
//! disambiguation, and error recovery producing a partial AST.
//!
//! **Precedence ladder** (31-RESEARCH.md Pattern 1, spec rule 2 + rule 4,
//! lowest → highest):
//!
//! ```text
//! or          (rule 47)   bp 10  left-assoc
//! and         (rule 48)   bp 20  left-assoc
//! comparison  (rule 49)   bp 30  (= != < <= > >= between in)
//! instance of (rule 51)   bp 35  (between comparison and arithmetic — A3)
//! additive    (rule 19/20) bp 40  (+ -)  left-assoc
//! mult        (rule 21/22) bp 50  (* /)  left-assoc
//! exponent    (rule 23)    bp 60  (**)   RIGHT-assoc  (Pitfall 2)
//! unary neg   (rule 24)    bp 70  (-)
//! postfix     (rule 43/50/38) bp 80  (. path, [filter], ( invocation))
//! primary     (rule 17/31/32/54/…) bp 90  (literal, name, ( expr ), list, …)
//! ```
//!
//! **Error recovery (CONTEXT.md user override):** on a parse error, emit a
//! `FeelExpression::Err(span, msg)` node at the error site, advance to
//! the next synchronization token (`,`, `)`, `]`, `}`, `then`, `else`,
//! `return`, `satisfies`, `in`, end-of-input), and continue. Never
//! `?`-propagate or panic on untrusted FEEL text (T-31-01-03 mitigation).
//!
//! **DoS guards** (T-31-01-01 / T-31-01-02): `max_tokens` cap on the lexer
//! (default 10_000); `max_depth` cap on the parser recursion (default 256).
//!
//! *(Task 2 implements the lexer; Task 3 implements the parser.)*
//!
//! Phase 37 adds:
//! - [`LexerTokenCache`] — lexer token caching to avoid redundant lexing

use super::ast::Span;
use rust_decimal::Decimal;

// Forward declaration for ParseEntry (defined in cache.rs)
#[allow(dead_code)]
type ParseEntry = crate::dmn::feel::cache::ParseEntry;

/// A FEEL lexical token (grammar rules 25, 33, 34, 35, 64, 65 + punctuators).
///
/// **Pitfall 3 (keyword-as-name):** the lexer does NOT have a `Keyword`
/// variant. Keyword-shaped strings (`for`, `if`, `some`, `every`, `in`,
/// `between`, `and`, `or`, `return`, `function`, `instance`, `of`, `not`,
/// `null`, `true`, `false`) all emit [`Token::Name`] — the parser
/// disambiguates by position (31-RESEARCH.md Pitfall 3). Booleans and
/// `null` are likewise contextually-resolved names, not distinct token
/// kinds.
///
/// Numeric literals construct [`Token::Number`] with a
/// `rust_decimal::Decimal` — **never `f64`** (prohibition 2). String
/// literals carry the decoded content (rule-64 escapes already expanded).
/// At-literals carry the inner string (temporal dispatch to
/// `feel::temporal::parse_at_literal` happens at evaluation time, not lex
/// time).
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A name (rule 25) — identifier chars including keyword-shaped strings.
    /// The parser disambiguates keyword-vs-name by position (Pitfall 3).
    Name(String),
    /// A numeric literal (rule 35) → `Decimal` (Decimal128, never `f64`).
    Number(Decimal),
    /// A string literal (rule 33) with rule-64 escapes already decoded.
    String(String),
    /// An `@"..."` at-literal (rule 65) — carries the inner string.
    AtLiteral(String),
    // Punctuators.
    /// `(` (rule 38/39 parameter list, rule 17 parenthesized expression).
    LParen,
    /// `)`.
    RParen,
    /// `[` (rule 50 filter, rule 54 list, rule 8 interval open-start).
    LBracket,
    /// `]`.
    RBracket,
    /// `{` (rule 57 context).
    LBrace,
    /// `}`.
    RBrace,
    /// `,` (rule 3 expression list, rule 44 multiple iteration contexts).
    Comma,
    /// `.` (rule 43 path, rule 8 interval `..` start, rule 18 QN separator).
    /// A standalone `.` (not part of `..`) is a path/number-dot token.
    Dot,
    /// `:` (rule 58 context entry key-separator, rule 56 formal param type
    /// annotation).
    Colon,
    /// `+` (rule 19 addition; also a name-part char per rule 25, but the
    /// lexer resolves `+` as this punctuator after whitespace/operator
    /// positions).
    Plus,
    /// `-` (rule 20 subtraction, rule 24 unary negation, rule 35 numeric
    /// literal sign — handled at lex time for literals, here for operator).
    Minus,
    /// `*` (rule 21 multiplication).
    Star,
    /// `/` (rule 22 division).
    Slash,
    /// `**` (rule 23 exponentiation — RIGHT-associative, Pitfall 2).
    DoubleStar,
    /// `=` (rule 49 equality — three-valued via `feel_equals`).
    Eq,
    /// `!=` (rule 49 inequality).
    NotEq,
    /// `<` (rule 49).
    Lt,
    /// `<=` (rule 49).
    Le,
    /// `>` (rule 49).
    Gt,
    /// `>=` (rule 49).
    Ge,
    /// `?` (rule 25 name-part char; also the filter predicate placeholder
    /// convention — the lexer emits `?` as its own token when it appears
    /// standalone, but inside a name it is part of a Name).
    Question,
    /// End of input — the parser's stop signal.
    Eof,
    /// An error marker token (T-31-01-01 DoS guard: emitted when
    /// `max_tokens` is exceeded, or when a numeric literal fails to parse
    /// as `Decimal`). Carries a message.
    LexError(String),
}

/// A token paired with its source [`Span`] (byte offset + length + line/col).
///
/// Every token carries a span so the parser can construct
/// `FeelExpression::Err(span, msg)` nodes for error recovery (CONTEXT.md
/// user override) and the Phase 34 editor can display per-cell errors.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenSpan {
    /// The token.
    pub token: Token,
    /// The source span of the token's first character.
    pub span: Span,
}

/// FEEL lexer — produces a `Vec<TokenSpan>` for the grammar's lexical forms.
///
/// Lexing rules (31-RESEARCH.md grammar-rule-to-parser-method table):
/// - Whitespace (rules 61–62): skip; track line/col.
/// - Numbers (rule 35): `["-"]? (digits ["." digits] | "." digits)` →
///   `Token::Number(Decimal)` via `Decimal::from_str_exact`. NEVER `f64`.
/// - Strings (rule 33): `"..."` with rule-64 escapes (`\'`, `\"`, `\\`,
///   `\n`, `\r`, `\t`, `\u{...}`). Emits the decoded content.
/// - At-literals (rule 65): `@"..."` → `Token::AtLiteral` carrying the
///   inner string (temporal dispatch at eval time, not lex time).
/// - Names (rule 25): identifier chars including `?`, letters, `_`, digits,
///   and the additional name symbols `.`/`/`/`-`/`'`/`+`/`*` BUT `.`/`+`/
///   `*`/`-` are ambiguous with operators — resolved by position: a name
///   *start* char (letter, `_`, `?`) begins a Name; subsequent name-part
///   chars extend it; a standalone `.`/`+`/`*`/`-` after whitespace or an
///   operator is a punct token. Keyword strings emit `Token::Name(keyword)`
///   — the parser disambiguates (Pitfall 3).
/// - Punctuators: `(`, `)`, `[`, `]`, `{`, `}`, `,`, `.`, `:`, `?`, `+`,
///   `-`, `*`, `/`, `**`, `=`, `!=`, `<`, `<=`, `>`, `>=`.
/// - DoS guard (T-31-01-01): if token count exceeds `max_tokens` (default
///   10_000), emit a single `Token::LexError` and stop.
pub struct Lexer<'a> {
    src: &'a [u8],
    cursor: usize,
    line: u32,
    col: u32,
    max_tokens: usize,
}

impl<'a> Lexer<'a> {
    /// Construct a new lexer over `src` with the default `max_tokens`
    /// cap (10_000 — T-31-01-01 mitigation).
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            cursor: 0,
            line: 1,
            col: 1,
            max_tokens: 10_000,
        }
    }

    /// Construct a lexer with a custom `max_tokens` cap (used by the DoS
    /// guard test).
    pub fn with_max_tokens(src: &'a str, max_tokens: usize) -> Self {
        Self {
            src: src.as_bytes(),
            cursor: 0,
            line: 1,
            col: 1,
            max_tokens,
        }
    }

    /// Tokenize the source into a `Vec<TokenSpan>`.
    ///
    /// Walks `src` byte-by-byte, skipping whitespace (rules 61–62),
    /// emitting one [`TokenSpan`] per token. A trailing [`Token::Eof`] is
    /// always appended. On `max_tokens` overflow (T-31-01-01), a single
    /// [`Token::LexError`] is emitted and tokenization stops.
    pub fn tokenize(&mut self) -> Vec<TokenSpan> {
        let mut out = Vec::new();
        while self.cursor < self.src.len() {
            if out.len() >= self.max_tokens {
                // T-31-01-01 DoS guard: emit a LexError and stop.
                let span = self.current_span(0);
                out.push(TokenSpan {
                    token: Token::LexError(format!("max_tokens ({}) exceeded", self.max_tokens)),
                    span,
                });
                return out;
            }
            let b = self.src[self.cursor];
            // Whitespace (rules 61–62): skip, track line/col.
            if b == b' ' || b == b'\t' || b == b'\r' {
                self.advance_byte();
                continue;
            }
            if b == b'\n' {
                self.line += 1;
                self.col = 1;
                self.cursor += 1;
                continue;
            }
            let start = self.cursor;
            let span_line = self.line;
            let span_col = self.col;
            let tok = self.lex_one();
            let len = self.cursor - start;
            out.push(TokenSpan {
                token: tok,
                span: Span {
                    start,
                    len,
                    line: span_line,
                    col: span_col,
                },
            });
        }
        // Trailing Eof.
        out.push(TokenSpan {
            token: Token::Eof,
            span: Span {
                start: self.cursor,
                len: 0,
                line: self.line,
                col: self.col,
            },
        });
        out
    }

    /// Build a span of length `len` at the current cursor position.
    fn current_span(&self, len: usize) -> Span {
        Span {
            start: self.cursor,
            len,
            line: self.line,
            col: self.col,
        }
    }

    /// Advance one byte, updating line/col.
    fn advance_byte(&mut self) {
        self.cursor += 1;
        self.col += 1;
    }

    /// Lex a single token starting at `self.cursor`. Advances the cursor
    /// past the token. Assumes `self.cursor < self.src.len()` and the
    /// current byte is not whitespace.
    fn lex_one(&mut self) -> Token {
        let b = self.src[self.cursor];
        // String literal (rule 33): "..." with rule-64 escapes.
        if b == b'"' {
            return self.lex_string();
        }
        // At-literal (rule 65): @"...".
        if b == b'@' && self.peek_at(1) == Some(b'"') {
            self.cursor += 1;
            self.col += 1;
            let inner = self.lex_string_inner();
            return Token::AtLiteral(inner);
        }
        // Numeric literal (rule 35): digits or ".digits" (leading-dot
        // decimal). A leading `-` is part of the numeric literal ONLY when
        // it is not in operator position — handled at the parser level via
        // unary-neg, so the lexer emits `Minus` for `-` and lets the parser
        // disambiguate. A `.` followed by a digit is a leading-dot decimal
        // — UNLESS that `.` is the second dot of `..` (range separator,
        // rule 8): `1..10` must tokenize as `Number(1), Dot, Dot,
        // Number(10)`, not `Number(1), Dot, Number(0.10)`. The check
        // `prev != b'.'` prevents the second `.` of `..` from being
        // consumed as a leading-dot decimal when followed by digits.
        if b.is_ascii_digit()
            || (b == b'.'
                && self.peek_at(1).is_some_and(|c| c.is_ascii_digit())
                && !(self.cursor > 0 && self.src[self.cursor - 1] == b'.'))
        {
            return self.lex_number();
        }
        // Name (rule 25): starts with a letter, `_`, or `?`.
        if b == b'?' || b == b'_' || b.is_ascii_alphabetic() || b.is_ascii_digit() {
            return self.lex_name();
        }
        // Punctuators.
        self.lex_punct()
    }

    /// Lex a `"..."` string literal (rule 33) with rule-64 escapes.
    /// The opening `"` is at `self.cursor`. Returns the decoded inner
    /// content. The closing `"` is consumed.
    fn lex_string(&mut self) -> Token {
        Token::String(self.lex_string_inner())
    }

    /// Lex the inner content of a `"..."` string (used by both `lex_string`
    /// and the at-literal inner string). The opening `"` is at `self.cursor`
    /// on entry; the closing `"` is consumed.
    fn lex_string_inner(&mut self) -> String {
        // Consume the opening quote.
        self.cursor += 1;
        self.col += 1;
        let mut out = String::new();
        while self.cursor < self.src.len() {
            let c = self.src[self.cursor];
            if c == b'"' {
                // Closing quote.
                self.cursor += 1;
                self.col += 1;
                return out;
            }
            if c == b'\\' {
                // Rule-64 escape sequence.
                self.cursor += 1;
                self.col += 1;
                if self.cursor >= self.src.len() {
                    // Unterminated escape — emit nothing, stop.
                    return out;
                }
                let esc = self.src[self.cursor];
                self.cursor += 1;
                self.col += 1;
                match esc {
                    b'\'' => out.push('\''),
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        // \u{XXXX} — Unicode code point escape.
                        if self.peek_at(0) == Some(b'{') {
                            self.cursor += 1;
                            self.col += 1;
                            let mut hex = String::new();
                            while self.cursor < self.src.len() && self.src[self.cursor] != b'}' {
                                hex.push(self.src[self.cursor] as char);
                                self.cursor += 1;
                                self.col += 1;
                            }
                            if self.cursor < self.src.len() && self.src[self.cursor] == b'}' {
                                self.cursor += 1;
                                self.col += 1;
                            }
                            if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(code) {
                                    out.push(ch);
                                }
                            }
                        }
                    }
                    other => {
                        // Unknown escape: keep the backslash as well as the
                        // character. FEEL lists the valid escapes, but models
                        // (and the TCK) pass regular expressions through string
                        // literals — `"\s"` must reach `split` as backslash-s,
                        // not as `s`.
                        out.push('\\');
                        out.push(other as char);
                    }
                }
                continue;
            }
            // Regular char. Handle ASCII fast; for multi-byte UTF-8, read
            // the full char from the source slice to avoid corrupting it.
            if c < 0x80 {
                out.push(c as char);
                self.cursor += 1;
                self.col += 1;
            } else {
                let ch_len = utf8_len(c).min(self.src.len() - self.cursor);
                if let Ok(s) = std::str::from_utf8(&self.src[self.cursor..self.cursor + ch_len]) {
                    if let Some(ch) = s.chars().next() {
                        out.push(ch);
                    }
                }
                self.cursor += ch_len;
                self.col += 1;
            }
        }
        out
    }

    /// Lex a numeric literal (rule 35) into `Token::Number(Decimal)`.
    /// The first byte at `self.cursor` is a digit or `.`. NEVER `f64`.
    fn lex_number(&mut self) -> Token {
        let start = self.cursor;
        // Optional integer digits.
        while self.cursor < self.src.len() && self.src[self.cursor].is_ascii_digit() {
            self.cursor += 1;
            self.col += 1;
        }
        // Optional fractional part: "." followed by digits.
        if self.peek_at(0) == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            self.cursor += 1;
            self.col += 1;
            while self.cursor < self.src.len() && self.src[self.cursor].is_ascii_digit() {
                self.cursor += 1;
                self.col += 1;
            }
        }
        // Optional exponent (FEEL rule 31): `e`/`E`, an optional sign, digits.
        // Without this the lexer stopped at the mantissa and the exponent was
        // silently dropped, so `1.23e4` evaluated to `1.23`.
        if matches!(self.peek_at(0), Some(b'e') | Some(b'E')) {
            let mut lookahead = 1;
            if matches!(self.peek_at(lookahead), Some(b'+') | Some(b'-')) {
                lookahead += 1;
            }
            if self.peek_at(lookahead).is_some_and(|c| c.is_ascii_digit()) {
                self.cursor += lookahead;
                self.col += lookahead as u32;
                while self.cursor < self.src.len() && self.src[self.cursor].is_ascii_digit() {
                    self.cursor += 1;
                    self.col += 1;
                }
            }
        }

        let text = std::str::from_utf8(&self.src[start..self.cursor]).unwrap_or("");
        match parse_numeric_literal(text) {
            Some(d) => Token::Number(d),
            None => Token::LexError(format!("invalid numeric literal: {:?}", text)),
        }
    }

    /// Lex a name (rule 25). The first byte is a letter, `_`, `?`, or digit.
    ///
    /// Name-part characters here are alphanumerics, `_` and `?` only. FEEL also
    /// permits `+ - * / . '` inside a name, but whether `Date-Time2` is one name
    /// or a subtraction is a question about the *model*, not the text: `a*b` is a
    /// product in every real model. Those runs are re-merged after lexing against
    /// the names the model declares (`Parser::merge_declared_name_tokens`), which
    /// keeps the lexer context-free and its token cache model-independent.
    fn lex_name(&mut self) -> Token {
        let start = self.cursor;
        self.cursor += 1;
        self.col += 1;
        while self.cursor < self.src.len() {
            let c = self.src[self.cursor];
            if c.is_ascii_alphanumeric() || c == b'_' || c == b'?' {
                self.cursor += 1;
                self.col += 1;
                continue;
            }
            break;
        }
        let text = std::str::from_utf8(&self.src[start..self.cursor]).unwrap_or("");
        Token::Name(text.to_string())
    }

    /// Lex a punctuator / operator starting at `self.cursor`. Handles `**`,
    /// `<=`, `>=`, `!=`, and single-char punctuators.
    fn lex_punct(&mut self) -> Token {
        let b = self.src[self.cursor];
        let tok = match b {
            b'(' => {
                self.advance_byte();
                Token::LParen
            }
            b')' => {
                self.advance_byte();
                Token::RParen
            }
            b'[' => {
                self.advance_byte();
                Token::LBracket
            }
            b']' => {
                self.advance_byte();
                Token::RBracket
            }
            b'{' => {
                self.advance_byte();
                Token::LBrace
            }
            b'}' => {
                self.advance_byte();
                Token::RBrace
            }
            b',' => {
                self.advance_byte();
                Token::Comma
            }
            b':' => {
                self.advance_byte();
                Token::Colon
            }
            b'?' => {
                self.advance_byte();
                Token::Question
            }
            b'+' => {
                self.advance_byte();
                Token::Plus
            }
            b'-' => {
                self.advance_byte();
                Token::Minus
            }
            b'*' => {
                if self.peek_at(1) == Some(b'*') {
                    self.cursor += 2;
                    self.col += 2;
                    Token::DoubleStar
                } else {
                    self.advance_byte();
                    Token::Star
                }
            }
            b'/' => {
                self.advance_byte();
                Token::Slash
            }
            b'=' => {
                self.advance_byte();
                Token::Eq
            }
            b'!' => {
                if self.peek_at(1) == Some(b'=') {
                    self.cursor += 2;
                    self.col += 2;
                    Token::NotEq
                } else {
                    self.advance_byte();
                    Token::LexError("unexpected character: '!'".to_string())
                }
            }
            b'<' => {
                if self.peek_at(1) == Some(b'=') {
                    self.cursor += 2;
                    self.col += 2;
                    Token::Le
                } else {
                    self.advance_byte();
                    Token::Lt
                }
            }
            b'>' => {
                if self.peek_at(1) == Some(b'=') {
                    self.cursor += 2;
                    self.col += 2;
                    Token::Ge
                } else {
                    self.advance_byte();
                    Token::Gt
                }
            }
            b'.' => {
                // `..` is the interval separator (rule 8) — emit two Dot
                // tokens so the parser can pair them. A single `.` is a
                // path/QN separator or a leading-dot decimal (handled in
                // lex_one before reaching here).
                self.advance_byte();
                Token::Dot
            }
            other => {
                self.advance_byte();
                Token::LexError(format!("unexpected character: {:?}", other as char))
            }
        };
        tok
    }

    /// Peek at the byte `offset` positions ahead of the cursor (0 = next
    /// byte after the current one). Returns `None` past end-of-input.
    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.cursor + offset).copied()
    }
}

/// Return the UTF-8 byte length of the char whose first byte is `b`.
/// Used to advance past multi-byte chars in string literals.
fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        // Invalid continuation byte — treat as 1 to avoid infinite loop.
        1
    }
}

#[cfg(test)]
mod lexer_tests {
    use super::*;
    use rust_decimal::dec;

    /// lex("1 + 2.5") yields tokens [Number, Plus, Number, Eof] with correct
    /// spans (behavior test 1).
    #[test]
    fn lex_number_plus_number() {
        let toks = Lexer::new("1 + 2.5").tokenize();
        assert!(
            toks.len() >= 4,
            "expected at least 4 tokens, got {}",
            toks.len()
        );
        assert_eq!(toks[0].token, Token::Number(dec!(1)));
        assert_eq!(toks[1].token, Token::Plus);
        assert_eq!(toks[2].token, Token::Number(dec!(2.5)));
        // Trailing Eof.
        assert_eq!(toks.last().unwrap().token, Token::Eof);
        // Span of the first token covers "1" at offset 0, length 1.
        assert_eq!(toks[0].span.start, 0);
        assert_eq!(toks[0].span.len, 1);
        assert_eq!(toks[0].span.line, 1);
        assert_eq!(toks[0].span.col, 1);
    }

    /// lex(r#""hello \"world\"""#) yields a String token with the escaped
    /// quote preserved per rule 64 (behavior test 2).
    #[test]
    fn lex_string_with_escaped_quote() {
        let toks = Lexer::new(r#""hello \"world\"""#).tokenize();
        assert_eq!(toks[0].token, Token::String("hello \"world\"".to_string()));
    }

    /// lex("@\"2021-01-31\"") yields an AtLiteral token (behavior test 3).
    #[test]
    fn lex_at_literal() {
        let toks = Lexer::new(r#"@"2021-01-31""#).tokenize();
        assert_eq!(toks[0].token, Token::AtLiteral("2021-01-31".to_string()));
    }

    /// lex("for x in list return x") yields [Name("for"), Name("x"),
    /// Name("in"), Name("list"), Name("return"), Name("x")] — keyword
    /// strings emit Name tokens (Pitfall 3: disambiguation is the parser's
    /// job, not the lexer's) (behavior test 4).
    #[test]
    fn lex_keywords_emit_name_tokens() {
        let toks = Lexer::new("for x in list return x").tokenize();
        let names: Vec<&Token> = toks.iter().map(|ts| &ts.token).collect();
        assert_eq!(
            names,
            vec![
                &Token::Name("for".to_string()),
                &Token::Name("x".to_string()),
                &Token::Name("in".to_string()),
                &Token::Name("list".to_string()),
                &Token::Name("return".to_string()),
                &Token::Name("x".to_string()),
                &Token::Eof,
            ]
        );
    }

    /// lex("null true false") yields Name("null"), Name("true"), Name("false")
    /// — booleans/null are contextually-resolved names, not distinct token
    /// kinds (parser resolves them) (behavior test 5).
    #[test]
    fn lex_null_true_false_are_names() {
        let toks = Lexer::new("null true false").tokenize();
        assert_eq!(toks[0].token, Token::Name("null".to_string()));
        assert_eq!(toks[1].token, Token::Name("true".to_string()));
        assert_eq!(toks[2].token, Token::Name("false".to_string()));
    }

    /// lex("a.b.c") yields [Name("a"), Dot, Name("b"), Dot, Name("c")] —
    /// the lexer does NOT pre-merge qualified names (behavior test 6).
    #[test]
    fn lex_qualified_name_not_merged() {
        let toks = Lexer::new("a.b.c").tokenize();
        assert_eq!(toks[0].token, Token::Name("a".to_string()));
        assert_eq!(toks[1].token, Token::Dot);
        assert_eq!(toks[2].token, Token::Name("b".to_string()));
        assert_eq!(toks[3].token, Token::Dot);
        assert_eq!(toks[4].token, Token::Name("c".to_string()));
    }

    /// lex with a token count exceeding MAX_TOKENS (e.g. 10_000) yields an
    /// Err token (DoS guard per research Security Domain V5) (behavior
    /// test 7).
    #[test]
    fn lex_max_tokens_dos_guard() {
        // A string of 5 single-char names separated by spaces yields 9
        // tokens per "a a a a a" — set max_tokens low (4) so the cap trips.
        let mut lexer = Lexer::with_max_tokens("a a a a a", 4);
        let toks = lexer.tokenize();
        assert!(
            toks.iter().any(|ts| matches!(ts.token, Token::LexError(_))),
            "expected a LexError token when max_tokens is exceeded, got {:?}",
            toks
        );
    }

    /// lex("**" yields DoubleStar (right-associative exponentiation,
    /// Pitfall 2).
    #[test]
    fn lex_double_star() {
        let toks = Lexer::new("2 ** 3").tokenize();
        assert_eq!(toks[1].token, Token::DoubleStar);
    }

    /// lex("<=" / ">=" / "!=" comparison operators.
    #[test]
    fn lex_comparison_operators() {
        let toks = Lexer::new("a <= b >= c != d").tokenize();
        assert_eq!(toks[1].token, Token::Le);
        assert_eq!(toks[3].token, Token::Ge);
        assert_eq!(toks[5].token, Token::NotEq);
    }

    /// lex a numeric literal ".5" (leading-dot decimal per rule 35).
    #[test]
    fn lex_leading_dot_decimal() {
        let toks = Lexer::new(".5").tokenize();
        assert_eq!(toks[0].token, Token::Number(dec!(0.5)));
    }

    /// lex never produces `f64` — numeric tokens are `Decimal`.
    #[test]
    fn lex_number_is_decimal_not_f64() {
        let toks = Lexer::new("0.1 + 0.2").tokenize();
        // Both Number tokens must be Decimal (the Token::Number variant
        // carries Decimal by construction; this test guards against a
        // future refactor adding an f64 variant).
        match &toks[0].token {
            Token::Number(d) => assert_eq!(*d, dec!(0.1)),
            other => panic!("expected Token::Number, got {:?}", other),
        }
    }

    /// lex rule-64 escape sequences: \n, \t, \\, \u{...}.
    #[test]
    fn lex_string_escapes() {
        let toks = Lexer::new(r#""a\nb\tc\\d\u{0041}""#).tokenize();
        assert_eq!(toks[0].token, Token::String("a\nb\tc\\dA".to_string()));
    }

    /// lex tracks line and column across newlines.
    #[test]
    fn lex_line_col_tracking() {
        let toks = Lexer::new("a\nb").tokenize();
        assert_eq!(toks[0].span.line, 1);
        assert_eq!(toks[0].span.col, 1);
        assert_eq!(toks[1].span.line, 2);
        assert_eq!(toks[1].span.col, 1);
    }
}

// ======================================================================
// Lexer token cache (Phase 37 optimization: avoid redundant lexing).
// ======================================================================
//
// The lexer produces a Vec<TokenSpan> that is often reused:
// - Multiple evalutations of the same FEEL cell (cached in FeelAstCache)
// - Incremental parsing scenarios (user typing, editor undo/redo)
//
// We cache the token stream keyed by source text + entry point kind.
// The same source always yields identical tokens — lexing is deterministic.

use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Canonical cache key: `(entry-point kind, trimmed content)`.
///
/// This mirrors `cell_key` in the AST cache but for tokens instead of ASTs.
pub fn token_cache_key(src: &str, entry: ParseEntry) -> String {
    let trimmed = src.trim();
    format!("{:?}:{}", entry, trimmed)
}

/// Default bounded capacity for the lexer token cache (PERF-05). Matches
/// the AST cache cap; the two caches are populated in lockstep (every AST
/// cache miss performs one token cache lookup), so they stay size-aligned
/// in normal per-model operation.
const DEFAULT_MAX_TOKEN_CACHE_ENTRIES: usize = 10_000;

/// Token cache keyed by `(ParseEntry, source_text)` pairs.
///
/// Used to avoid redundant lexing when the same FEEL text is parsed
/// multiple times. The cache stores `Arc<Vec<TokenSpan>>` so token lists
/// can be shared without cloning.
///
/// **PERF-05 — bounded eviction:** the map is bounded by
/// [`DEFAULT_MAX_TOKEN_CACHE_ENTRIES`] via FIFO eviction (insertion-order,
/// tracked by a `Mutex<VecDeque<String>>`). FIFO keeps the hot read path
/// lock-free — the `Mutex` is touched only on a miss/insert. See
/// [`crate::dmn::feel::cache::FeelAstCache`] for the full rationale (the two
/// caches share the same eviction strategy and per-model scoping argument).
#[derive(Debug)]
pub struct LexerTokenCache {
    map: DashMap<String, Arc<Vec<TokenSpan>>>,
    /// Insertion order for FIFO eviction when `map.len()` exceeds `max_entries`.
    order: Mutex<VecDeque<String>>,
    /// Bounded capacity (PERF-05 backstop).
    max_entries: usize,
}

impl LexerTokenCache {
    /// Construct an empty lexer token cache with the default bounded capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_MAX_TOKEN_CACHE_ENTRIES)
    }

    /// Construct an empty lexer token cache with a custom bounded capacity.
    pub fn with_capacity(max_entries: usize) -> Self {
        Self {
            map: DashMap::new(),
            order: Mutex::new(VecDeque::new()),
            max_entries,
        }
    }

    /// Look up or lex `src` under the given entry-point kind. Returns the
    /// cached (or freshly lexed) token stream as an `Arc<Vec<TokenSpan>>`.
    ///
    /// **Contract:** `src` must be post-CDATA-strip (same as AST cache).
    pub fn get_or_lex(&self, src: &str, entry: ParseEntry) -> Arc<Vec<TokenSpan>> {
        let key = token_cache_key(src, entry);
        let tokens = match self.map.entry(key.clone()) {
            dashmap::mapref::entry::Entry::Occupied(o) => o.get().clone(),
            dashmap::mapref::entry::Entry::Vacant(v) => {
                let mut lexer = Lexer::new(src);
                let lexed = Arc::new(lexer.tokenize());
                v.insert(lexed.clone());
                if let Ok(mut ord) = self.order.lock() {
                    ord.push_back(key);
                }
                lexed
            }
        };
        if self.map.len() > self.max_entries {
            self.evict_oldest();
        }
        tokens
    }

    /// FIFO eviction: drop the oldest-inserted keys until `map.len()` is back
    /// under `max_entries`. Duplicate keys (from concurrent misses) are
    /// harmless — `map.remove` is idempotent on a missing key.
    fn evict_oldest(&self) {
        let to_remove: Vec<String> = {
            let mut ord = match self.order.lock() {
                Ok(o) => o,
                Err(_) => return,
            };
            let mut keys = Vec::new();
            while self.map.len() > self.max_entries {
                match ord.pop_front() {
                    Some(k) => keys.push(k),
                    None => break,
                }
            }
            keys
        };
        for k in &to_remove {
            self.map.remove(k);
        }
    }

    /// Number of distinct (entry-point, content) pairs currently cached.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// True iff the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clear all cached tokens (useful for test isolation or reload scenarios).
    pub fn clear(&self) {
        self.map.clear();
        if let Ok(mut ord) = self.order.lock() {
            ord.clear();
        }
    }
}

impl Default for LexerTokenCache {
    fn default() -> Self {
        Self::new()
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod lexer_cache_tests {
    use super::*;

    /// Same source text under same entry point returns identical token stream.
    #[test]
    fn get_or_lex_returns_same_tokens_for_same_source() {
        let cache = LexerTokenCache::new();
        let src = "1 + 2 * 3";

        let a = cache.get_or_lex(src, ParseEntry::Expression);
        let b = cache.get_or_lex(src, ParseEntry::Expression);

        assert_eq!(a.len(), b.len(), "same source yields same token count");
        for (ta, tb) in a.iter().zip(b.iter()) {
            assert_eq!(
                ta.token, tb.token,
                "token at position {} differs",
                ta.span.start
            );
            assert_eq!(
                ta.span, tb.span,
                "span at position {} differs",
                ta.span.start
            );
        }
    }

    /// Different entry points for same source return IDENTICAL token streams
    /// (lexer doesn't care about parsing context).
    #[test]
    fn different_entry_points_produce_identical_token_streams() {
        let cache = LexerTokenCache::new();
        let src = "x"; // simple name

        let expr_tokens = cache.get_or_lex(src, ParseEntry::Expression);
        let unary_tokens = cache.get_or_lex(src, ParseEntry::SimpleUnaryTests);

        assert_eq!(
            expr_tokens.len(),
            unary_tokens.len(),
            "lexer produces same token count regardless of entry point"
        );
        for (ta, tb) in expr_tokens.iter().zip(unary_tokens.iter()) {
            assert_eq!(
                ta.token, tb.token,
                "token at position {} identical regardless of entry point",
                ta.span.start
            );
        }
        // Same source = same cache key (key is just (entry, content), so different
        // entry points yield DIFFERENT entries even if tokens are the same).
        assert_eq!(
            cache.len(),
            2,
            "different entry points create separate cache entries"
        );
    }

    /// Whitespace normalization works in token cache key.
    #[test]
    fn whitespace_is_normalized_in_token_cache_key() {
        let cache = LexerTokenCache::new();

        let a = cache.get_or_lex("  x + y  ", ParseEntry::Expression);
        let b = cache.get_or_lex("x + y", ParseEntry::Expression);

        assert_eq!(a.len(), b.len());
    }

    /// Cache len reflects distinct (entry, content) pairs.
    #[test]
    fn cache_len_tracks_distinct_entries() {
        let cache = LexerTokenCache::new();

        cache.get_or_lex("1 + 2", ParseEntry::Expression);
        cache.get_or_lex("3 * 4", ParseEntry::Expression);
        cache.get_or_lex("1 + 2", ParseEntry::Expression); // duplicate

        assert_eq!(cache.len(), 2, "duplicate entries don't grow cache");
    }

    /// clear() empties the cache.
    #[test]
    fn clear_empties_cache() {
        let cache = LexerTokenCache::new();
        cache.get_or_lex("1 + 2", ParseEntry::Expression);
        assert_eq!(cache.len(), 1);

        cache.clear();
        assert!(cache.is_empty(), "clear empties the cache");
    }
}

// ======================================================================
// Recursive-descent + Pratt parser (grammar rules 1–65) — Task 3.
// ======================================================================
//
// The parser consumes the lexer's `Vec<TokenSpan>` and produces a
// `FeelExpression`. Two public entry points:
//   - `parse_expression(src)` (grammar rule 1) — output cells / general FEEL.
//   - `parse_simple_unary_tests(src)` (grammar rule 15) — input cells.
//
// **Error recovery (CONTEXT.md user override):** on a parse error, emit a
// `FeelExpression::Err(span, msg)` node at the error site, advance to the
// next synchronization token (`,`, `)`, `]`, `}`, `then`, `else`,
// `return`, `satisfies`, `in`, end-of-input), and continue. Never `?`-
// propagate or panic on untrusted FEEL text (T-31-01-03 mitigation).
//
// **DoS guard (T-31-01-02):** `max_depth` cap (default 256) on the
// recursive-descent stack; past the cap emit
// `FeelExpression::Err(span, "max recursion depth exceeded")`.

use super::ast::{BinOpKind, FeelExpression, QuantKind};
use super::types::FeelType;
use crate::data_types::Value;

/// The default maximum recursion depth for the parser (T-31-01-02).
const DEFAULT_MAX_DEPTH: u32 = 256;

/// The recursive-descent + Pratt parser over `Vec<TokenSpan>`.
/// Re-merge tokens that form a name the model declares.
///
/// The lexer emits `Date-Time2` as `Date`, `-`, `Time2` and `values in a list` as
/// four name tokens. A declared name may contain spaces and the name symbols
/// `+ - * / . '`, so this pass joins the longest declared name that starts at each
/// position, replacing the run with a single `Name` token.
fn merge_declared_name_tokens(tokens: Vec<TokenSpan>, known_names: &[String]) -> Vec<TokenSpan> {
    if known_names.is_empty() {
        return tokens;
    }
    let declarations: Vec<Vec<String>> = known_names
        .iter()
        .filter(|name| name.contains(' ') || name.chars().any(|c| "+-*/.\'".contains(c)))
        .map(|name| name.split_whitespace().map(str::to_string).collect())
        .collect();
    if declarations.is_empty() {
        return tokens;
    }

    let mut merged: Vec<TokenSpan> = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while index < tokens.len() {
        if !matches!(tokens[index].token, Token::Name(_)) {
            merged.push(tokens[index].clone());
            index += 1;
            continue;
        }
        // Build the candidate text token by token, tracking the longest declared
        // name that matches.
        let mut best: Option<(usize, String)> = None;
        let mut candidate = String::new();
        let mut cursor = index;
        let mut expect_word = true;
        while cursor < tokens.len() {
            match (&tokens[cursor].token, expect_word) {
                (Token::Name(word), true) => {
                    // Words join with a space; a symbol joins them without one
                    // (`Date` + `-` + `Time2` is `Date-Time2`, not `Date- Time2`).
                    if !candidate.is_empty() && !candidate.ends_with(['-', '+', '*', '/', '.']) {
                        candidate.push(' ');
                    }
                    candidate.push_str(word);
                    expect_word = false;
                    cursor += 1;
                }
                // A name symbol continues the same word with no space.
                (Token::Minus, false)
                | (Token::Plus, false)
                | (Token::Star, false)
                | (Token::Slash, false)
                | (Token::Dot, false) => {
                    let Some(symbol) = name_symbol(&tokens[cursor].token) else {
                        break;
                    };
                    candidate.push(symbol);
                    expect_word = true;
                    cursor += 1;
                }
                _ => break,
            }
            if declarations
                .iter()
                .any(|declaration| *declaration == candidate.split(' ').collect::<Vec<_>>())
            {
                best = Some((cursor, candidate.clone()));
            }
        }
        match best {
            Some((end, name)) => {
                let mut span = tokens[index].clone();
                span.token = Token::Name(name);
                merged.push(span);
                index = end;
            }
            None => {
                merged.push(tokens[index].clone());
                index += 1;
            }
        }
    }
    merged
}

/// The character a token contributes when it is part of a name.
fn name_symbol(token: &Token) -> Option<char> {
    match token {
        Token::Minus => Some('-'),
        Token::Plus => Some('+'),
        Token::Star => Some('*'),
        Token::Slash => Some('/'),
        Token::Dot => Some('.'),
        _ => None,
    }
}

/// Properties whose name contains a space.
fn is_multiword_property(name: &str) -> bool {
    matches!(name, "time offset")
}

/// Parse a FEEL numeric literal, including scientific notation.
///
/// `rust_decimal` parses a plain decimal exactly; the exponent is applied here by
/// repeated multiplication (bounded to the decimal scale) so `1.23e4` is `12300`
/// and `1.23e-4` is `0.000123`.
fn parse_numeric_literal(text: &str) -> Option<Decimal> {
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], text[index + 1..].parse::<i32>().ok()?),
        None => (text, 0),
    };
    let mut value = Decimal::from_str_exact(mantissa).ok()?;
    let ten = Decimal::from(10);
    // `rust_decimal` holds at most 28 fractional digits, so a larger magnitude
    // exponent cannot be represented either way.
    for _ in 0..exponent.unsigned_abs().min(28) {
        value = if exponent >= 0 {
            value.checked_mul(ten)?
        } else {
            value.checked_div(ten)?
        };
    }
    Some(value)
}

struct Parser {
    tokens: Vec<TokenSpan>,
    pos: usize,
    depth: u32,
    max_depth: u32,
    /// Names the model declares (inputs, decisions, variables). FEEL names may
    /// contain spaces, so a name like `Monthly Salary` has to be recognised as
    /// one name rather than two adjacent identifiers.
    known_names: Vec<String>,
    /// True while parsing the upper bound of a range, where a `[` closes the
    /// range (`[1..10[`) instead of starting a filter.
    in_range_bound: bool,
}

impl Parser {
    fn new(tokens: Vec<TokenSpan>) -> Self {
        Self::with_known_names(tokens, Vec::new())
    }

    fn with_known_names(tokens: Vec<TokenSpan>, known_names: Vec<String>) -> Self {
        let tokens = merge_declared_name_tokens(tokens, &known_names);
        Self {
            tokens,
            pos: 0,
            depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
            known_names,
            in_range_bound: false,
        }
    }

    /// The current token (or `Token::Eof` past end).
    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .map(|ts| &ts.token)
            .unwrap_or(&Token::Eof)
    }

    /// The current token's span (or a zero span at end).
    fn peek_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map(|ts| ts.span)
            .unwrap_or_else(|| Span::zero(0, 1, 1))
    }

    /// Advance and return the consumed token.
    fn advance(&mut self) -> Option<TokenSpan> {
        let ts = self.tokens.get(self.pos).cloned();
        if ts.is_some() {
            self.pos += 1;
        }
        ts
    }

    /// True if the current token is a `Name` with the given keyword text.
    fn is_keyword(&self, kw: &str) -> bool {
        matches!(self.peek(), Token::Name(s) if s == kw)
    }

    /// Consume a keyword `Name`; returns true if consumed.
    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.is_keyword(kw) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Emit an `Err` node at the current position.
    fn err_here(&self, msg: &str) -> FeelExpression {
        FeelExpression::Err(self.peek_span(), msg.to_string())
    }

    /// Advance to the next synchronization token (error recovery,
    /// CONTEXT.md user override — not fail-fast).
    fn sync_to_boundary(&mut self) {
        while !matches!(
            self.peek(),
            Token::Eof | Token::Comma | Token::RParen | Token::RBracket | Token::RBrace
        ) {
            if let Token::Name(s) = self.peek() {
                if matches!(s.as_str(), "then" | "else" | "return" | "satisfies") {
                    return;
                }
            }
            self.advance();
        }
    }

    /// DoS depth guard (T-31-01-02): returns false if max depth exceeded.
    fn enter(&mut self) -> bool {
        self.depth += 1;
        self.depth <= self.max_depth
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// True if the current position is `instance of` (two-token lookahead).
    fn is_instance_of(&self) -> bool {
        if !self.is_keyword("instance") {
            return false;
        }
        match self.tokens.get(self.pos + 1) {
            Some(ts) => matches!(&ts.token, Token::Name(s) if s == "of"),
            None => false,
        }
    }

    // ================================================================
    // Pratt-precedence loop (grammar rules 2, 19–23, 47–49, 51).
    // ================================================================

    /// The main Pratt loop. Parses a textual expression at `min_bp` or
    /// higher. Binding powers:
    ///   or=10, and=20, comparison/between/in=30, instance of=35,
    ///   additive=40, mult=50, **=60 (RIGHT-assoc),
    ///   unary=70, postfix=80, primary=90.
    fn parse_expr(&mut self, min_bp: u8) -> FeelExpression {
        if !self.enter() {
            return self.err_here("max recursion depth exceeded");
        }
        let mut lhs = self.parse_unary();
        loop {
            // --- Special: `between` (bp 30, ternary: a between b and c)
            if self.is_keyword("between") && 30 >= min_bp {
                self.advance(); // consume "between"
                let lower = self.parse_expr(31); // bp>20 → `and` not consumed
                if matches!(lower, FeelExpression::Err(_, _)) {
                    self.leave();
                    return lower;
                }
                if !self.eat_keyword("and") {
                    self.leave();
                    self.sync_to_boundary();
                    return self.err_here("expected 'and' in 'between' expression");
                }
                let upper = self.parse_expr(31);
                if matches!(upper, FeelExpression::Err(_, _)) {
                    self.leave();
                    return upper;
                }
                lhs = FeelExpression::BinOp {
                    op: BinOpKind::Between,
                    lhs: Box::new(lhs),
                    rhs: Box::new(FeelExpression::List(vec![lower, upper])),
                };
                continue;
            }

            // --- Special: `instance of` (bp 35, A3 verification)
            if self.is_instance_of() && 35 >= min_bp {
                self.advance(); // "instance"
                self.advance(); // "of"
                let ty = self.parse_type_ref();
                lhs = FeelExpression::InstanceOf {
                    expr: Box::new(lhs),
                    ty,
                };
                continue;
            }

            // --- Special: `in` with a parenthesised unary-test list (DMN 1.5 §10.3.1.2)
            //
            // Out of line on purpose: the locals this needs (checkpoints, parsed tests) would
            // otherwise inflate `parse_expr`'s frame for *every* recursion level, and the frame size is
            // what bounds how deep the DoS guard can recurse before the stack does (FEEL-012).
            if self.is_keyword("in") && 30 >= min_bp {
                if let Some(with_tests) = self.parse_in_unary_tests(lhs.clone()) {
                    lhs = with_tests;
                    continue;
                }
            }

            // --- Regular binary operators
            let (op, bp, rassoc) = match self.classify_binary() {
                Some(c) => c,
                None => break,
            };
            if bp < min_bp {
                break;
            }
            self.advance(); // consume operator
            let next_min = if rassoc { bp } else { bp + 1 };
            let rhs = self.parse_expr(next_min);
            // Error recovery: propagate Err nodes from the RHS (error
            // recovery model — don't wrap errors in more AST nodes).
            if matches!(rhs, FeelExpression::Err(_, _)) {
                self.leave();
                return rhs;
            }
            lhs = FeelExpression::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        self.leave();
        lhs
    }

    /// Classify the current token as a binary operator with binding power
    /// and associativity. Returns `None` for non-operators (including
    /// `between` and `instance`, which are handled specially in `parse_expr`).
    fn classify_binary(&self) -> Option<(BinOpKind, u8, bool)> {
        match self.peek() {
            Token::Plus => Some((BinOpKind::Add, 40, false)),
            Token::Minus => Some((BinOpKind::Sub, 40, false)),
            Token::Star => Some((BinOpKind::Mul, 50, false)),
            Token::Slash => Some((BinOpKind::Div, 50, false)),
            // `**` is left-associative here: the TCK evaluates `3 ** 4 ** 5` as
            // `(3 ** 4) ** 5` (`0100-arithmetic`, `exponent_003`).
            Token::DoubleStar => Some((BinOpKind::Pow, 60, false)),
            Token::Eq => Some((BinOpKind::Eq, 30, false)),
            Token::NotEq => Some((BinOpKind::NotEq, 30, false)),
            Token::Lt => Some((BinOpKind::Lt, 30, false)),
            Token::Le => Some((BinOpKind::Le, 30, false)),
            Token::Gt => Some((BinOpKind::Gt, 30, false)),
            Token::Ge => Some((BinOpKind::Ge, 30, false)),
            Token::Name(s) => match s.as_str() {
                "or" => Some((BinOpKind::Or, 10, false)),
                "and" => Some((BinOpKind::And, 20, false)),
                "in" => Some((BinOpKind::In, 30, false)),
                _ => None, // `between`/`instance` handled in parse_expr
            },
            _ => None,
        }
    }

    // ================================================================
    // Prefix / postfix / primary
    // ================================================================

    /// Parse unary prefix operators (rule 24: `-expr`).
    fn parse_unary(&mut self) -> FeelExpression {
        match self.peek() {
            Token::Minus => {
                self.advance();
                let operand = self.parse_unary();
                FeelExpression::Neg(Box::new(operand))
            }
            _ => self.parse_postfix(),
        }
    }

    /// Parse postfix operators: `.path` (rule 43), `[filter]` (rule 50),
    /// `(invocation)` (rule 38). Processed left-to-right so `a.b[i]`
    /// naturally yields `Filter(Path(a, b), i)` (A2: path < filter <
    /// invocation).
    fn parse_postfix(&mut self) -> FeelExpression {
        let mut base = self.parse_primary();
        // Don't apply postfix operators to Err nodes — propagate the error
        // (error recovery model: don't wrap errors in more AST nodes).
        if matches!(base, FeelExpression::Err(_, _)) {
            return base;
        }
        loop {
            match self.peek() {
                Token::Dot => {
                    // Check for `..` (range operator)
                    let next_is_dot = self
                        .tokens
                        .get(self.pos + 1)
                        .map(|ts| matches!(ts.token, Token::Dot))
                        .unwrap_or(false);
                    if next_is_dot {
                        self.advance(); // first .
                        self.advance(); // second .
                        let end = self.parse_range_bound();
                        base = FeelExpression::Range {
                            start: Box::new(base),
                            end: Box::new(end),
                            start_inclusive: true,
                            end_inclusive: true,
                            point: false,
                        };
                        continue;
                    }
                    // Regular path: `target.name`. A property name may contain
                    // spaces (`dateValue.time offset`), so a run of words that
                    // forms a known property name is one path segment.
                    self.advance();
                    let mut name = match self.advance() {
                        Some(TokenSpan {
                            token: Token::Name(n),
                            ..
                        }) => n,
                        _ => {
                            self.sync_to_boundary();
                            return self.err_here("expected name after '.'");
                        }
                    };
                    while let Token::Name(next) = self.peek().clone() {
                        let merged = format!("{name} {next}");
                        if is_multiword_property(&merged) {
                            name = merged;
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    base = FeelExpression::Path {
                        target: Box::new(base),
                        name,
                    };
                }
                Token::LBracket => {
                    // Inside a range bound, or directly after a range literal, a
                    // `[` closes the range (`[1..10[`, `]1..10[`) — it does not
                    // index or filter anything.
                    if self.in_range_bound || matches!(base, FeelExpression::Range { .. }) {
                        return base;
                    }
                    self.advance();
                    let pred = self.parse_expr(0);
                    if !matches!(self.peek(), Token::RBracket) {
                        self.sync_to_boundary();
                        return self.err_here("expected ']' after filter predicate");
                    }
                    self.advance();
                    base = FeelExpression::Filter {
                        target: Box::new(base),
                        predicate: Box::new(pred),
                    };
                }
                Token::LParen => {
                    self.advance();
                    let (args, named) = self.parse_arg_list();
                    if !matches!(self.peek(), Token::RParen) {
                        self.sync_to_boundary();
                        return self.err_here("expected ')' after function arguments");
                    }
                    self.advance();
                    base = FeelExpression::FunctionInvocation {
                        callee: Box::new(base),
                        args,
                        named,
                    };
                }
                _ => break,
            }
        }
        base
    }

    /// Parse comma-separated arguments inside `(...)`. Returns
    /// `(positional_args, Option<named_args>)`.
    fn parse_arg_list(&mut self) -> (Vec<FeelExpression>, Option<Vec<(String, FeelExpression)>>) {
        let mut args = Vec::new();
        let mut named: Option<Vec<(String, FeelExpression)>> = None;
        if matches!(self.peek(), Token::RParen) {
            return (args, named);
        }
        loop {
            // Check for a named argument. FEEL parameter names can contain
            // spaces (`decimal separator: ","`, `grouping separator: "."`), so a
            // name is a run of adjacent `Name` tokens terminated by `:`.
            let mut lookahead = self.pos;
            let mut words: Vec<String> = Vec::new();
            while let Some(TokenSpan {
                token: Token::Name(word),
                ..
            }) = self.tokens.get(lookahead)
            {
                words.push(word.clone());
                lookahead += 1;
            }
            let is_named = !words.is_empty()
                && matches!(
                    self.tokens.get(lookahead).map(|span| &span.token),
                    Some(Token::Colon)
                );
            if is_named {
                let name = words.join(" ");
                self.pos = lookahead + 1; // consume the names and the ':'
                let val = self.parse_expr(0);
                named.get_or_insert_with(Vec::new).push((name, val));
            } else {
                let arg = self.parse_expr(0);
                args.push(arg);
            }
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        (args, named)
    }

    /// Parse a primary expression (rules 17, 25, 31–32, 54–57).
    fn parse_primary(&mut self) -> FeelExpression {
        let tok = self.peek().clone();
        match tok {
            Token::Number(n) => {
                self.advance();
                FeelExpression::Literal(Value::Number(n))
            }
            Token::String(s) => {
                self.advance();
                FeelExpression::Literal(Value::String(s))
            }
            Token::AtLiteral(s) => {
                self.advance();
                FeelExpression::AtLiteral(s)
            }
            Token::LParen => {
                self.advance();
                // `(=v)` is a singleton range, not a parenthesised `..` range:
                // its inclusivity is already correct and the `)` is not a bound.
                let singleton_range = matches!(self.peek(), Token::Eq);
                let inner = self.parse_expr(0);
                // Propagate Err nodes (e.g. depth-guard errors) without
                // wrapping them in a "expected ')'" error.
                if matches!(inner, FeelExpression::Err(_, _)) {
                    return inner;
                }
                // If inner is a Range from parse_postfix's `..` handling,
                // fix `start_inclusive` to `false` (because `(` = exclusive
                // start) and `end_inclusive` from the closing bracket
                // (`)` → exclusive, `]` → inclusive). This handles the
                // `(a..b)` and `(a..b]` range syntaxes (rule 8).
                if singleton_range && matches!(inner, FeelExpression::Range { .. }) {
                    if !matches!(self.peek(), Token::RParen) {
                        self.sync_to_boundary();
                        return self.err_here("expected ')' after singleton range");
                    }
                    self.advance();
                    return inner;
                }
                if let FeelExpression::Range { start, end, .. } = inner {
                    let end_inclusive = matches!(self.peek(), Token::RBracket);
                    if !matches!(self.peek(), Token::RParen | Token::RBracket) {
                        self.sync_to_boundary();
                        return self.err_here("expected ')' or ']' after range");
                    }
                    self.advance(); // consume ) or ]
                    return FeelExpression::Range {
                        start,
                        end,
                        start_inclusive: false,
                        end_inclusive,
                        point: false,
                    };
                }
                if !matches!(self.peek(), Token::RParen) {
                    self.sync_to_boundary();
                    return self.err_here("expected ')' after parenthesized expression");
                }
                self.advance();
                inner
            }
            Token::LBracket => self.parse_list_or_range(),
            // `]a..b]` — the alternative exclusive-start bracket.
            Token::RBracket => {
                self.advance();
                let inner = self.parse_expr(0);
                if matches!(inner, FeelExpression::Err(_, _)) {
                    return inner;
                }
                let FeelExpression::Range { start, end, .. } = inner else {
                    self.sync_to_boundary();
                    return self.err_here("expected a range after ']'");
                };
                let end_inclusive = matches!(self.peek(), Token::RBracket);
                if !matches!(
                    self.peek(),
                    Token::RBracket | Token::LBracket | Token::RParen
                ) {
                    self.sync_to_boundary();
                    return self.err_here("expected ']', '[' or ')' after range");
                }
                self.advance();
                FeelExpression::Range {
                    start,
                    end,
                    start_inclusive: false,
                    end_inclusive,
                    point: false,
                }
            }
            // `= value` — a singleton range (the same as `[value..value]`).
            //
            // A bound may not itself be a singleton range. `parse_range_bound` re-enters
            // this rule, so without the guard `= = = = ...` recursed once per `=` and
            // CLONED the subtree into both `start` and `end` at every level: twenty
            // equals signs parsed into an AST of 2^20 nodes and about 170 MB, and
            // twenty-four exhausted multi-gigabyte memory. A fuzz target found it as an
            // out-of-memory abort; `feels_dos_...` tests hold the line.
            Token::Eq if self.in_range_bound => {
                self.sync_to_boundary();
                self.err_here("a range bound cannot be a singleton range")
            }
            Token::Eq => {
                self.advance();
                let value = self.parse_range_bound();
                FeelExpression::Range {
                    start: Box::new(value.clone()),
                    end: Box::new(value),
                    start_inclusive: true,
                    end_inclusive: true,
                    point: true,
                }
            }
            Token::LBrace => self.parse_context(),
            Token::Name(s) => {
                match s.as_str() {
                    "true" => {
                        self.advance();
                        FeelExpression::Literal(Value::Bool(true))
                    }
                    "false" => {
                        self.advance();
                        FeelExpression::Literal(Value::Bool(false))
                    }
                    "null" => {
                        self.advance();
                        FeelExpression::Literal(Value::Null)
                    }
                    "for" => self.parse_for(),
                    "if" => self.parse_if(),
                    "some" => self.parse_quantified(QuantKind::Some),
                    "every" => self.parse_quantified(QuantKind::Every),
                    "function" => self.parse_function_definition(),
                    // Keyword-as-name (Pitfall 3): any other keyword-shaped
                    // string is a Name reference. Disambiguation by position
                    // (the Pratt loop decides if `or`/`and`/`between`/`in`/
                    // `instance` are operators vs names).
                    _ => {
                        if let Some(declared) = self.merge_declared_name(&s) {
                            // `merge_declared_name` leaves the position on the
                            // last word of the name; consume it.
                            self.advance();
                            return FeelExpression::Name(declared);
                        }
                        let name = self.merge_multiword_name(&s);
                        self.advance();
                        FeelExpression::Name(name)
                    }
                }
            }
            Token::LexError(msg) => {
                self.advance();
                FeelExpression::Err(self.peek_span(), format!("lex error: {}", msg))
            }
            Token::Eof => self.err_here("unexpected end of input"),
            other => self.err_here(&format!("unexpected token: {:?}", other)),
        }
    }

    /// Parse a range bound: a `[` after it closes the range, so the postfix loop
    /// must not treat it as a filter.
    fn parse_range_bound(&mut self) -> FeelExpression {
        let previous = self.in_range_bound;
        self.in_range_bound = true;
        let bound = self.parse_expr(0);
        self.in_range_bound = previous;
        bound
    }

    /// Join the words of a name the model declares (`Monthly Salary`,
    /// `values in a list`) into one `Name` node.
    ///
    /// The longest declared name that starts at the current position wins; the
    /// parser position is left on the last consumed word so the caller's
    /// `advance()` consumes it. Returns `None` when no declared multi-word name
    /// matches here, leaving the ordinary single-name path to run.
    fn merge_declared_name(&mut self, first: &str) -> Option<String> {
        if self.known_names.is_empty() {
            return None;
        }
        let mut words = vec![first.to_string()];
        let mut best: Option<(String, usize)> = None;
        let mut probe = self.pos + 1;
        loop {
            let candidate = words.join(" ");
            if self.known_names.contains(&candidate) {
                best = Some((candidate.clone(), words.len()));
            }
            let next = match self.tokens.get(probe) {
                Some(TokenSpan {
                    token: Token::Name(word),
                    ..
                }) => word.clone(),
                _ => break,
            };
            words.push(next);
            probe += 1;
        }
        let (name, word_count) = best?;
        if word_count < 2 {
            return None;
        }
        self.pos += word_count - 1;
        Some(name)
    }

    /// Join a multi-word built-in function name whose words the lexer emitted
    /// as separate `Name` tokens.
    ///
    /// FEEL names may contain spaces (`upper case`, `string join`,
    /// `list contains`, `get value`, `date and time`, `round half up`, …) and
    /// the registry is keyed by those full names. Starting from the current
    /// `Name` token, greedily consume following `Name` tokens while the joined
    /// text remains a prefix of a registered multi-word built-in, and accept
    /// the merged name only when it exactly equals one of them. Anything else
    /// (e.g. `a and contains(x)`, `date and foo`) is left unmerged, so operator
    /// and variable names keep their single-token meaning.
    ///
    /// On success the parser position is left on the last consumed word so the
    /// caller's [`Parser::advance`] consumes it and the following token is the
    /// `(` of the invocation.
    fn merge_multiword_name(&mut self, first: &str) -> String {
        let candidates = super::builtins::multi_word_builtin_names();
        if candidates.is_empty() || !candidates.iter().any(|c| c.starts_with(first)) {
            return first.to_string();
        }

        let mut joined = first.to_string();
        let mut probe = self.pos + 1;

        while let Some(TokenSpan {
            token: Token::Name(word),
            ..
        }) = self.tokens.get(probe)
        {
            let candidate = format!("{joined} {word}");
            let is_prefix = candidates.iter().any(|c| c.starts_with(&candidate));
            if !is_prefix {
                break;
            }
            joined = candidate;
            probe += 1;
        }

        if probe > self.pos + 1 && candidates.contains(&joined.as_str()) {
            self.pos = probe - 1;
            joined
        } else {
            first.to_string()
        }
    }

    /// Parse `[e1, e2, ...]` (list, rule 54) or `[a..b]` (bracketed range).
    ///
    /// `parse_postfix` greedily consumes `..` as a postfix range operator
    /// (for standalone `a..b` in `for`/`some`/`every` iteration sources),
    /// so `parse_expr(0)` for the first element may already return a
    /// [`FeelExpression::Range`]. When that happens, this function unwraps
    /// it and fixes `end_inclusive` from the closing bracket (`]` →
    /// inclusive, `)` → exclusive). `start_inclusive` stays `true`
    /// because `[` is always inclusive (this function is only called from
    /// the `LBracket` arm of `parse_primary`).
    fn parse_list_or_range(&mut self) -> FeelExpression {
        self.advance(); // consume [
        if matches!(self.peek(), Token::RBracket) {
            self.advance();
            return FeelExpression::List(vec![]);
        }
        let first = self.parse_expr(0);

        // If parse_postfix already consumed `..` and produced a Range,
        // unwrap it and fix end_inclusivity from the closing bracket.
        // `start_inclusive` is always `true` (opening `[` is inclusive).
        if let FeelExpression::Range {
            start,
            end,
            start_inclusive,
            ..
        } = first
        {
            // FEEL writes an exclusive end as `[` (`[1..10[`) and an inclusive
            // one as `]`; `)` is also accepted for `[1..10)`.
            let end_inclusive = matches!(self.peek(), Token::RBracket);
            if !matches!(
                self.peek(),
                Token::RBracket | Token::RParen | Token::LBracket
            ) {
                self.sync_to_boundary();
                return self.err_here("expected ']', '[' or ')' after range");
            }
            self.advance(); // consume closing bracket
            return FeelExpression::Range {
                start,
                end,
                start_inclusive,
                end_inclusive,
                point: false,
            };
        }

        // Check for range: `..` after first element (fallback for when
        // parse_postfix did not consume `..`, e.g. if the first element
        // is a complex expression that stopped before the `..`).
        let next_is_dot = self
            .tokens
            .get(self.pos)
            .map(|ts| matches!(ts.token, Token::Dot))
            .unwrap_or(false);
        let next_next_is_dot = self
            .tokens
            .get(self.pos + 1)
            .map(|ts| matches!(ts.token, Token::Dot))
            .unwrap_or(false);
        if next_is_dot && next_next_is_dot {
            self.advance(); // first .
            self.advance(); // second .
            let end = self.parse_range_bound();
            let end_inclusive = matches!(self.peek(), Token::RBracket);
            if !matches!(
                self.peek(),
                Token::RBracket | Token::RParen | Token::LBracket
            ) {
                self.sync_to_boundary();
            }
            self.advance(); // consume ] or )
            return FeelExpression::Range {
                start: Box::new(first),
                end: Box::new(end),
                start_inclusive: true,
                end_inclusive,
                point: false,
            };
        }
        // It's a list
        let mut elems = vec![first];
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            elems.push(self.parse_expr(0));
        }
        if !matches!(self.peek(), Token::RBracket) {
            self.sync_to_boundary();
            return self.err_here("expected ']' after list elements");
        }
        self.advance(); // consume ]
        FeelExpression::List(elems)
    }

    /// Parse `{ k: v, ... }` (context, rule 57). Keys may be keyword-shaped
    /// strings (Pitfall 3: `for`, `if`, `in` are valid context keys).
    fn parse_context(&mut self) -> FeelExpression {
        self.advance(); // consume {
        let mut entries = Vec::new();
        if matches!(self.peek(), Token::RBrace) {
            self.advance();
            return FeelExpression::Context(entries);
        }
        loop {
            // A context key is a name or a string. FEEL names may contain
            // spaces (`{foo bar: "foo"}`), so a run of adjacent name tokens up to
            // the `:` is one key.
            let key = match self.peek().clone() {
                Token::String(value) => {
                    self.advance();
                    value
                }
                Token::Name(first) => {
                    self.advance();
                    let mut key = first;
                    loop {
                        match self.peek().clone() {
                            // Another word continues the key with a space.
                            Token::Name(next) => {
                                key.push(' ');
                                key.push_str(&next);
                                self.advance();
                            }
                            // A name symbol joins without one (`{foo+bar: 1}`).
                            symbol
                                if matches!(
                                    symbol,
                                    Token::Minus | Token::Plus | Token::Star | Token::Slash
                                ) =>
                            {
                                let Some(character) = name_symbol(&symbol) else {
                                    break;
                                };
                                let after =
                                    self.tokens.get(self.pos + 1).map(|span| span.token.clone());
                                let Some(Token::Name(next)) = after else {
                                    break;
                                };
                                key.push(character);
                                key.push_str(&next);
                                self.advance();
                                self.advance();
                            }
                            _ => break,
                        }
                    }
                    key
                }
                _ => {
                    self.sync_to_boundary();
                    return self.err_here("expected context key (name or string)");
                }
            };
            if !matches!(self.peek(), Token::Colon) {
                self.sync_to_boundary();
                return self.err_here("expected ':' after context key");
            }
            self.advance(); // consume :
            let val = self.parse_expr(0);
            if entries.iter().any(|(existing, _)| *existing == key) {
                // A duplicate key makes the context invalid (`0057-feel-context`,
                // `decision008`), so this is a parse error rather than last-wins.
                self.sync_to_boundary();
                return self.err_here("duplicate key in context");
            }
            entries.push((key, val));
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        if !matches!(self.peek(), Token::RBrace) {
            self.sync_to_boundary();
            return self.err_here("expected '}' after context entries");
        }
        self.advance(); // consume }
        FeelExpression::Context(entries)
    }

    /// Parse `function(params) [external] body` (rule 55).
    fn parse_function_definition(&mut self) -> FeelExpression {
        self.advance(); // consume "function"
        if !matches!(self.peek(), Token::LParen) {
            self.sync_to_boundary();
            return self.err_here("expected '(' after 'function'");
        }
        self.advance(); // consume (
        let mut params = Vec::new();
        if !matches!(self.peek(), Token::RParen) {
            loop {
                let name = match self.advance() {
                    Some(TokenSpan {
                        token: Token::Name(n),
                        ..
                    }) => n,
                    _ => {
                        self.sync_to_boundary();
                        return self.err_here("expected parameter name");
                    }
                };
                let ty = if matches!(self.peek(), Token::Colon) {
                    self.advance();
                    Some(self.parse_type_ref())
                } else {
                    None
                };
                params.push((name, ty));
                if matches!(self.peek(), Token::Comma) {
                    self.advance();
                } else {
                    break;
                }
            }
        }
        if !matches!(self.peek(), Token::RParen) {
            self.sync_to_boundary();
            return self.err_here("expected ')' after parameter list");
        }
        self.advance(); // consume )
        let external = self.eat_keyword("external");
        let body = self.parse_expr(0);
        FeelExpression::FunctionDefinition {
            params,
            body: Box::new(body),
            external,
        }
    }

    /// Parse `for x in iter, y in iter2 return body` (rule 44).
    fn parse_for(&mut self) -> FeelExpression {
        self.advance(); // consume "for"
        let mut iters = Vec::new();
        loop {
            let var = match self.advance() {
                Some(TokenSpan {
                    token: Token::Name(n),
                    ..
                }) => n,
                _ => {
                    self.sync_to_boundary();
                    return self.err_here("expected iteration variable name");
                }
            };
            if !self.eat_keyword("in") {
                self.sync_to_boundary();
                return self.err_here("expected 'in' after iteration variable");
            }
            let source = self.parse_expr(0);
            iters.push((var, source));
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        if !self.eat_keyword("return") {
            self.sync_to_boundary();
            return self.err_here("expected 'return' in for expression");
        }
        let body = self.parse_expr(0);
        FeelExpression::For {
            iters,
            body: Box::new(body),
        }
    }

    /// Parse `if cond then a else b` (rule 45).
    fn parse_if(&mut self) -> FeelExpression {
        self.advance(); // consume "if"
        let cond = self.parse_expr(0);
        if !self.eat_keyword("then") {
            self.sync_to_boundary();
            return self.err_here("expected 'then' in if expression");
        }
        let then = self.parse_expr(0);
        if !self.eat_keyword("else") {
            self.sync_to_boundary();
            return self.err_here("expected 'else' in if expression");
        }
        let else_ = self.parse_expr(0);
        FeelExpression::If {
            cond: Box::new(cond),
            then: Box::new(then),
            else_: Box::new(else_),
        }
    }

    /// Parse `some/every x in list satisfies pred` (rule 46).
    fn parse_quantified(&mut self, kind: QuantKind) -> FeelExpression {
        self.advance(); // consume "some" or "every"
        let mut iters = Vec::new();
        loop {
            let var = match self.advance() {
                Some(TokenSpan {
                    token: Token::Name(n),
                    ..
                }) => n,
                _ => {
                    self.sync_to_boundary();
                    return self.err_here("expected iteration variable name");
                }
            };
            if !self.eat_keyword("in") {
                self.sync_to_boundary();
                return self.err_here("expected 'in' after iteration variable");
            }
            let source = self.parse_expr(0);
            iters.push((var, source));
            if matches!(self.peek(), Token::Comma) {
                self.advance();
            } else {
                break;
            }
        }
        if !self.eat_keyword("satisfies") {
            self.sync_to_boundary();
            return self.err_here("expected 'satisfies' in quantified expression");
        }
        let satisfies = self.parse_expr(0);
        FeelExpression::Quantified {
            kind,
            iters,
            satisfies: Box::new(satisfies),
        }
    }

    /// Parse a FEEL type reference for `instance of T` (rule 51).
    /// Handles simple types, multi-word types (`date and time`,
    /// `days and time duration`, `years and months duration`), and
    /// parametric types (`list<T>`, etc. — simplified to `Any` element
    /// type for now; 31-04 extends this).
    fn parse_type_ref(&mut self) -> FeelType {
        let name = match self.advance() {
            Some(TokenSpan {
                token: Token::Name(n),
                ..
            }) => n,
            _ => return FeelType::Any,
        };
        // Multi-word types: check for `and` following the first word
        if self.is_keyword("and") {
            match name.as_str() {
                "date" => {
                    self.advance(); // and
                    self.eat_keyword("time");
                    return FeelType::DateTime;
                }
                "days" => {
                    self.advance(); // and
                    self.eat_keyword("time");
                    self.eat_keyword("duration");
                    return FeelType::DaysTimeDuration;
                }
                "years" => {
                    self.advance(); // and
                    self.eat_keyword("months");
                    self.eat_keyword("duration");
                    return FeelType::YearsMonthsDuration;
                }
                _ => {} // fall through to simple type
            }
        }
        match name.as_str() {
            "number" => FeelType::Number,
            "string" => FeelType::String,
            "boolean" => FeelType::Boolean,
            "null" => FeelType::Null,
            "date" => FeelType::Date,
            "time" => FeelType::Time,
            "Any" => FeelType::Any,
            "list" => FeelType::List(Box::new(FeelType::Any)),
            "context" => FeelType::Context(vec![]),
            "function" => FeelType::Function {
                params: vec![],
                ret: Box::new(FeelType::Any),
            },
            "range" => FeelType::Range(Box::new(FeelType::Any)),
            _ => FeelType::Any,
        }
    }

    // ================================================================
    // SimpleUnaryTests (rule 15) helpers
    // ================================================================

    /// Parse a single positive unary test (rule 11/12).
    /// Leading comparison operators (`<`, `<=`, `>`, `>=`, `=`, `!=`)
    /// produce `BinOp(op, Name("?"), endpoint)` — the `?` placeholder
    /// is the input-cell marker (evaluator resolves it, 31-02).
    fn parse_positive_unary_test(&mut self) -> FeelExpression {
        let placeholder = FeelExpression::Name("?".to_string());
        let op = match self.peek() {
            Token::Lt => Some(BinOpKind::Lt),
            Token::Le => Some(BinOpKind::Le),
            Token::Gt => Some(BinOpKind::Gt),
            Token::Ge => Some(BinOpKind::Ge),
            Token::Eq => Some(BinOpKind::Eq),
            Token::NotEq => Some(BinOpKind::NotEq),
            _ => None,
        };
        if let Some(op) = op {
            self.advance();
            let rhs = self.parse_expr(31);
            return FeelExpression::BinOp {
                op,
                lhs: Box::new(placeholder),
                rhs: Box::new(rhs),
            };
        }
        self.parse_expr(0)
    }

    /// `lhs in (…)` where the parentheses hold a unary-test list, as `Some(expr)`; `None` when the
    /// parentheses hold an expression (a range, an arithmetic group), which the normal path parses.
    #[inline(never)]
    fn parse_in_unary_tests(&mut self, lhs: FeelExpression) -> Option<FeelExpression> {
        if !self.in_list_is_unary_tests() {
            return None;
        }
        let negated = matches!(
            self.tokens.get(self.pos + 1).map(|span| &span.token),
            Some(Token::Name(name)) if name == "not"
        );
        self.advance(); // "in"
        if negated {
            self.advance(); // "not"
        }
        self.advance(); // "("
        let tests = self.parse_positive_unary_tests();
        if matches!(tests, FeelExpression::Err(_, _)) || !matches!(self.peek(), Token::RParen) {
            return None;
        }
        self.advance(); // ")"
        let rhs = if negated {
            FeelExpression::Neg(Box::new(tests))
        } else {
            tests
        };
        Some(FeelExpression::BinOp {
            op: BinOpKind::In,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }

    /// Is the token at the cursor an `in` whose parenthesised right-hand side is a **unary-test
    /// list** rather than an expression?
    ///
    /// It is one when the parentheses open with a comparison operator (`< 5`), or when the contents
    /// separate positive unary tests with a top-level comma (`1, 2, 3`), or when it is `not(...)`.
    /// Anything else — `(1..10)`, `(a + b)` — is an expression and must take the normal path, because
    /// only that path gives a range its parentheses' inclusivity.
    fn in_list_is_unary_tests(&self) -> bool {
        let mut index = self.pos + 1;
        let negated = matches!(
            self.tokens.get(index).map(|span| &span.token),
            Some(Token::Name(name)) if name == "not"
        );
        if negated {
            index += 1;
        }
        if !matches!(
            self.tokens.get(index).map(|span| &span.token),
            Some(Token::LParen)
        ) {
            return false;
        }
        index += 1;

        if matches!(
            self.tokens.get(index).map(|span| &span.token),
            Some(Token::Lt | Token::Le | Token::Gt | Token::Ge | Token::Eq | Token::NotEq)
        ) {
            return true;
        }

        // A top-level comma: a list of tests. Nested commas belong to a call or a list literal.
        let mut depth = 0usize;
        while let Some(span) = self.tokens.get(index) {
            match span.token {
                Token::LParen | Token::LBracket | Token::LBrace => depth += 1,
                Token::RParen if depth == 0 => return false,
                Token::RParen | Token::RBracket | Token::RBrace => depth = depth.saturating_sub(1),
                Token::Comma if depth == 0 => return true,
                _ => {}
            }
            index += 1;
        }
        false
    }

    /// Parse comma-separated positive unary tests (rule 13), desugared
    /// to a nested `Or` chain (left-folded).
    fn parse_positive_unary_tests(&mut self) -> FeelExpression {
        let first = self.parse_positive_unary_test();
        if !matches!(self.peek(), Token::Comma) {
            return first;
        }
        let mut result = first;
        while matches!(self.peek(), Token::Comma) {
            self.advance();
            let next = self.parse_positive_unary_test();
            result = FeelExpression::BinOp {
                op: BinOpKind::Or,
                lhs: Box::new(result),
                rhs: Box::new(next),
            };
        }
        result
    }
}

// ---- Public entry points ----

/// Parse a FEEL `Expression` (grammar rule 1) from `src`.
///
/// Dispatches on the first token: `[`/`{`/`function` → boxed expression
/// (rule 53/54/55/57); `for`/`if`/`some`/`every` → textual control form
/// (rules 44/45/46); else → textual expression via the Pratt loop (rule 2).
///
/// **Error recovery:** on a parse error, returns a `FeelExpression::Err`
/// node — never panics on untrusted input (T-31-01-03 mitigation).
///
/// **Lexer cache (Phase 37):** optional `cache` parameter for token caching.
/// When provided, the lexer's output tokens are cached and reused across
/// identical source texts. For the first parse of a given text, tokens are
/// produced normally and stored in the cache; subsequent parses return
/// cached tokens with zero lexical overhead.
pub fn parse_expression(src: &str) -> FeelExpression {
    parse_expression_with_cache(src, None)
}

/// Parse a FEEL `Expression`, resolving names the model declares.
///
/// `known_names` lets the parser recognise a name that contains spaces (DMN
/// allows them, and models use them: `Monthly Salary`, `values in a list`).
/// Without it such a name parses as two adjacent names, which is a syntax error.
pub fn parse_expression_with_names(src: &str, known_names: &[String]) -> FeelExpression {
    let mut lexer = Lexer::new(src);
    let tokens = lexer.tokenize();
    let mut parser = Parser::with_known_names(tokens, known_names.to_vec());
    parser.parse_expr(0)
}

/// Parse a FEEL `Expression` (grammar rule 1) from `src` using an optional lexer cache.
///
/// When `cache` is `Some(&LexerTokenCache)`, the lexer's token stream is cached
/// and reused across identical source texts. This eliminates redundant lexing
/// for expressions parsed multiple times (e.g., in iterative evaluation scenarios).
pub fn parse_expression_with_cache(src: &str, cache: Option<&LexerTokenCache>) -> FeelExpression {
    let tokens = if let Some(cache) = cache {
        // Use cached tokens when available.
        cache.get_or_lex(src, ParseEntry::Expression)
    } else {
        // Fall back to direct lexing.
        let mut lexer = Lexer::new(src);
        Arc::new(lexer.tokenize())
    };
    let mut p = Parser::new(tokens.as_ref().to_vec());
    p.parse_expr(0)
}

/// Parse a FEEL `SimpleUnaryTests` (grammar rule 15) from `src`.
///
/// Handles `-` (empty/any → sentinel `Literal(Bool(true))` "always true"
/// node), `not(tests)` → `Neg(positiveUnaryTests)`, and a comma-separated
/// Or-chain of positive unary tests (rule 14 → rule 13 → expression).
pub fn parse_simple_unary_tests(src: &str) -> FeelExpression {
    parse_simple_unary_tests_with_cache(src, None)
}

/// Parse a FEEL `SimpleUnaryTests` (grammar rule 15) from `src` using an optional lexer cache.
pub fn parse_simple_unary_tests_with_cache(
    src: &str,
    cache: Option<&LexerTokenCache>,
) -> FeelExpression {
    let tokens = if let Some(cache) = cache {
        cache.get_or_lex(src, ParseEntry::SimpleUnaryTests)
    } else {
        let mut lexer = Lexer::new(src);
        Arc::new(lexer.tokenize())
    };
    let mut p = Parser::new(tokens.as_ref().to_vec());

    // Lone `-` means "always true" (any input passes). The lexer appends
    // a trailing `Eof`, so we check: first token is `Minus` and the next
    // is `Eof` (or absent).
    if matches!(p.peek(), Token::Minus) {
        let second_is_eof = p
            .tokens
            .get(1)
            .map(|ts| matches!(ts.token, Token::Eof))
            .unwrap_or(true);
        if second_is_eof {
            return FeelExpression::Literal(Value::Bool(true));
        }
    }

    // `not` form (rule 15: `["not"] positiveUnaryTests`).
    let negated = p.eat_keyword("not");
    let had_paren = negated && matches!(p.peek(), Token::LParen);
    if had_paren {
        p.advance(); // consume (
    }

    let result = p.parse_positive_unary_tests();

    if had_paren && matches!(p.peek(), Token::RParen) {
        p.advance();
    }

    if negated {
        FeelExpression::Neg(Box::new(result))
    } else {
        result
    }
}

#[cfg(test)]
mod parser_tests {
    use super::*;
    use crate::data_types::Value;
    use crate::dmn::feel::ast::{BinOpKind, FeelExpression, QuantKind};

    /// Helper: assert a `FeelExpression` matches a literal number.
    fn assert_num(expr: &FeelExpression, n: i64) {
        match expr {
            FeelExpression::Literal(Value::Number(d)) => assert_eq!(*d, Decimal::from(n)),
            other => panic!("expected Literal(Number({})), got {:?}", n, other),
        }
    }

    /// A run of `=` used to be exponential: every `=` built a singleton range whose
    /// bound re-entered the same rule and cloned the subtree into `start` and `end`.
    /// Twenty-four equals signs exhausted the memory of a fuzzer run; these cases pin
    /// the behaviour at "an error, quickly, with a small tree".
    #[test]
    fn singleton_range_does_not_nest() {
        for n in [2usize, 8, 24, 64, 1_000] {
            let src = "=".repeat(n);
            let started = std::time::Instant::now();
            let expr = parse_expression(&src);
            let elapsed = started.elapsed();
            assert!(
                elapsed < std::time::Duration::from_millis(500),
                "{} equals signs took {:?}",
                n,
                elapsed
            );
            let rendered = format!("{expr:?}");
            assert!(
                rendered.len() < 2_000,
                "{} equals signs produced a {} byte tree",
                n,
                rendered.len()
            );
        }
    }

    /// The exact input a fuzz target reduced the blow-up to (an out-of-memory abort at
    /// 2 GB). It parses to an error without a large allocation.
    #[test]
    fn fuzz_artifact_singleton_range_oom() {
        let src = "[==============\u{0b}=====[=A]==";
        let started = std::time::Instant::now();
        let expr = parse_expression(src);
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        assert!(format!("{expr:?}").len() < 4_000);
    }

    /// A single `= value` is still a singleton range (the behaviour the nesting guard
    /// must not remove).
    #[test]
    fn singleton_range_still_parses() {
        match parse_expression("= 5") {
            FeelExpression::Range {
                start, end, point, ..
            } => {
                assert!(point, "expected a point range");
                assert_eq!(format!("{start:?}"), format!("{end:?}"));
            }
            other => panic!("expected a singleton range, got {other:?}"),
        }
    }

    /// Test: parse_expression("1 + 2 * 3") yields BinOp(Add, 1, BinOp(Mul, 2, 3)) (precedence).
    #[test]
    fn parse_precedence_add_mul() {
        let expr = parse_expression("1 + 2 * 3");
        match expr {
            FeelExpression::BinOp {
                op: BinOpKind::Add,
                lhs,
                rhs,
            } => {
                assert_num(&lhs, 1);
                match rhs.as_ref() {
                    FeelExpression::BinOp {
                        op: BinOpKind::Mul, ..
                    } => {}
                    other => panic!("expected BinOp(Mul) on rhs, got {:?}", other),
                }
            }
            other => panic!("expected BinOp(Add), got {:?}", other),
        }
    }

    /// Test: parse_expression("2 ** 3 ** 2") yields BinOp(Pow, 2, BinOp(Pow, 3, 2)) (right-assoc, Pitfall 2).
    #[test]
    fn parse_pow_left_associative() {
        // `2 ** 3 ** 2` groups as `(2 ** 3) ** 2`: the TCK's `3 ** 4 ** 5` is
        // `(3 ** 4) ** 5` (`0100-arithmetic`, `exponent_003`).
        let expr = parse_expression("2 ** 3 ** 2");
        match expr {
            FeelExpression::BinOp {
                op: BinOpKind::Pow,
                lhs,
                ..
            } => match lhs.as_ref() {
                FeelExpression::BinOp {
                    op: BinOpKind::Pow, ..
                } => {}
                other => panic!("expected BinOp(Pow) on lhs (left-assoc), got {:?}", other),
            },
            other => panic!("expected BinOp(Pow) at the root, got {:?}", other),
        }
    }

    #[test]
    fn parse_or_and_precedence() {
        let expr = parse_expression("a or b and c");
        match expr {
            FeelExpression::BinOp {
                op: BinOpKind::Or,
                lhs,
                rhs,
            } => {
                match lhs.as_ref() {
                    FeelExpression::Name(n) => assert_eq!(n, "a"),
                    other => panic!("expected Name(a) on lhs, got {:?}", other),
                }
                match rhs.as_ref() {
                    FeelExpression::BinOp {
                        op: BinOpKind::And, ..
                    } => {}
                    other => panic!("expected BinOp(And) on rhs, got {:?}", other),
                }
            }
            other => panic!("expected BinOp(Or), got {:?}", other),
        }
    }

    /// Test: parse_expression("a.b[i]") yields Filter(Path(a, b), i) (path < filter, A2 verification).
    #[test]
    fn parse_path_filter_precedence_a2() {
        let expr = parse_expression("a.b[i]");
        match expr {
            FeelExpression::Filter { target, predicate } => {
                match target.as_ref() {
                    FeelExpression::Path {
                        target: inner,
                        name,
                    } => {
                        match inner.as_ref() {
                            FeelExpression::Name(n) => assert_eq!(n, "a"),
                            other => panic!("expected Name(a) inside Path, got {:?}", other),
                        }
                        assert_eq!(name, "b");
                    }
                    other => panic!("expected Path(a, b) as filter target, got {:?}", other),
                }
                match predicate.as_ref() {
                    FeelExpression::Name(n) => assert_eq!(n, "i"),
                    other => panic!("expected Name(i) as predicate, got {:?}", other),
                }
            }
            other => panic!("expected Filter, got {:?}", other),
        }
    }

    /// Test: parse_expression("1 + 2 instance of number") yields InstanceOf(BinOp(Add, 1, 2), Number) (A3 verification).
    #[test]
    fn parse_instance_of_precedence_a3() {
        let expr = parse_expression("1 + 2 instance of number");
        match expr {
            FeelExpression::InstanceOf { expr: inner, ty } => {
                match inner.as_ref() {
                    FeelExpression::BinOp {
                        op: BinOpKind::Add, ..
                    } => {}
                    other => panic!("expected BinOp(Add) inside InstanceOf, got {:?}", other),
                }
                assert_eq!(ty, crate::dmn::feel::types::FeelType::Number);
            }
            other => panic!("expected InstanceOf, got {:?}", other),
        }
    }

    /// Test: parse_expression("{ for: 1, if: 2 }") yields Context([("for", 1), ("if", 2)]) (keyword-as-name, Pitfall 3).
    #[test]
    fn parse_context_keyword_keys_pitfall3() {
        let expr = parse_expression("{ for: 1, if: 2 }");
        match expr {
            FeelExpression::Context(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].0, "for");
                assert_eq!(entries[1].0, "if");
                assert_num(&entries[0].1, 1);
                assert_num(&entries[1].1, 2);
            }
            other => panic!("expected Context, got {:?}", other),
        }
    }

    /// Test: parse_expression("function(for, in, between) for + in + between") parses (keyword param names + keyword body names).
    #[test]
    fn parse_function_keyword_params_and_body() {
        let expr = parse_expression("function(for, in, between) for + in + between");
        match expr {
            FeelExpression::FunctionDefinition {
                params, external, ..
            } => {
                assert_eq!(params.len(), 3);
                assert_eq!(params[0].0, "for");
                assert_eq!(params[1].0, "in");
                assert_eq!(params[2].0, "between");
                assert!(!external, "non-external function definition");
            }
            other => panic!("expected FunctionDefinition, got {:?}", other),
        }
    }

    /// Test: parse_simple_unary_tests("-") yields a sentinel "always true" node (empty unary tests).
    #[test]
    fn parse_unary_tests_dash_sentinel() {
        let expr = parse_simple_unary_tests("-");
        // The sentinel is a Literal(Bool(true)) — "always true" for any input.
        match expr {
            FeelExpression::Literal(Value::Bool(true)) => {}
            other => panic!(
                "expected Literal(Bool(true)) sentinel for '-', got {:?}",
                other
            ),
        }
    }

    /// Test: parse_simple_unary_tests("not(< 3)") yields a negation node.
    #[test]
    fn parse_unary_tests_not_form() {
        let expr = parse_simple_unary_tests("not(< 3)");
        // `not(...)` yields a Neg node over the inner tests.
        match expr {
            FeelExpression::Neg(inner) => {
                // Inner is the positive unary test `< 3` — represented as a
                // BinOp(Lt, input_marker, 3) or similar. Just assert it's
                // not an Err.
                assert!(!matches!(inner.as_ref(), FeelExpression::Err(_, _)));
            }
            other => panic!("expected Neg, got {:?}", other),
        }
    }

    /// Test: parse_simple_unary_tests("1, 2, 3") yields an Or-chain of positive unary tests (comma list).
    #[test]
    fn parse_unary_tests_comma_list() {
        let expr = parse_simple_unary_tests("1, 2, 3");
        // Comma-separated positive unary tests desugar to an Or-chain.
        // Either a nested BinOp(Or) chain or a single literal — assert no Err.
        assert!(
            !matches!(expr, FeelExpression::Err(_, _)),
            "expected non-Err, got {:?}",
            expr
        );
    }

    /// Test: parse_expression("1 + ") yields a FeelExpression::Err(span, msg) node (error recovery, not panic).
    #[test]
    fn parse_error_recovery_emits_err_node() {
        let expr = parse_expression("1 + ");
        match expr {
            FeelExpression::Err(_, msg) => {
                assert!(!msg.is_empty(), "error message should be non-empty");
            }
            other => panic!("expected Err node, got {:?}", other),
        }
    }

    /// Test: parse_expression with deeply nested ((((...)))) beyond MAX_DEPTH yields an Err node (DoS guard).
    #[test]
    fn parse_max_depth_dos_guard() {
        let deep = "(".repeat(300) + &")".repeat(300);
        let expr = parse_expression(&deep);
        match expr {
            FeelExpression::Err(_, msg) => {
                assert!(
                    msg.contains("depth") || msg.contains("recursion") || msg.contains("max"),
                    "expected depth-related error, got: {}",
                    msg
                );
            }
            other => panic!("expected Err node for deep nesting, got {:?}", other),
        }
    }

    /// Test: parse_expression("for x in 1..10 return x * 2") yields For with iteration context range.
    #[test]
    fn parse_for_expression() {
        let expr = parse_expression("for x in 1..10 return x * 2");
        match expr {
            FeelExpression::For { iters, .. } => {
                assert_eq!(iters.len(), 1);
                assert_eq!(iters[0].0, "x");
            }
            other => panic!("expected For, got {:?}", other),
        }
    }

    /// Test: parse_expression("some x in [1,2,3] satisfies x > 1") yields Quantified(Some, ...).
    #[test]
    fn parse_some_quantified() {
        let expr = parse_expression("some x in [1,2,3] satisfies x > 1");
        match expr {
            FeelExpression::Quantified {
                kind: QuantKind::Some,
                ..
            } => {}
            other => panic!("expected Quantified(Some), got {:?}", other),
        }
    }

    /// Test: parse_expression("5 instance of number") yields InstanceOf(Literal(5), Number).
    #[test]
    fn parse_instance_of_number() {
        let expr = parse_expression("5 instance of number");
        match expr {
            FeelExpression::InstanceOf { ty, .. } => {
                assert_eq!(ty, crate::dmn::feel::types::FeelType::Number);
            }
            other => panic!("expected InstanceOf, got {:?}", other),
        }
    }

    /// Test: parse_expression("[1,2,3]") yields List; parse_expression("{a: 1, b: 2}") yields Context.
    #[test]
    fn parse_list_and_context() {
        let list_expr = parse_expression("[1,2,3]");
        match list_expr {
            FeelExpression::List(elems) => assert_eq!(elems.len(), 3),
            other => panic!("expected List, got {:?}", other),
        }
        let ctx_expr = parse_expression("{a: 1, b: 2}");
        match ctx_expr {
            FeelExpression::Context(entries) => assert_eq!(entries.len(), 2),
            other => panic!("expected Context, got {:?}", other),
        }
    }

    /// Test: parse_expression("function(a, b) a + b") yields FunctionDefinition (non-external).
    #[test]
    fn parse_function_definition_non_external() {
        let expr = parse_expression("function(a, b) a + b");
        match expr {
            FeelExpression::FunctionDefinition {
                params, external, ..
            } => {
                assert_eq!(params.len(), 2);
                assert!(!external);
            }
            other => panic!("expected FunctionDefinition, got {:?}", other),
        }
    }

    /// Test: parse_expression("function(a, b) external a + b") yields FunctionDefinition with external=true (recognized, NOT rejected at parse time).
    #[test]
    fn parse_function_definition_external_recognized() {
        let expr = parse_expression("function(a, b) external a + b");
        match expr {
            FeelExpression::FunctionDefinition {
                params, external, ..
            } => {
                assert_eq!(params.len(), 2);
                assert!(external, "external flag should be recognized at parse time");
            }
            other => panic!("expected FunctionDefinition, got {:?}", other),
        }
    }

    /// Test: parse_expression("a between b and c") yields Between desugaring node (BinOp(Between, a, ...)) — the parser does NOT greedily consume `and` as the boolean operator.
    #[test]
    fn parse_between_desugaring() {
        let expr = parse_expression("a between b and c");
        match expr {
            FeelExpression::BinOp {
                op: BinOpKind::Between,
                ..
            } => {}
            other => panic!("expected BinOp(Between, ...), got {:?}", other),
        }
    }

    /// Test: parse_expression("not(null)") parses `not` as a function invocation (not a keyword here).
    #[test]
    fn parse_not_as_invocation() {
        let expr = parse_expression("not(null)");
        match expr {
            FeelExpression::FunctionInvocation { .. } => {}
            other => panic!("expected FunctionInvocation, got {:?}", other),
        }
    }
}
