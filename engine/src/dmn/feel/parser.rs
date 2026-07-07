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
//! *(Task 1 skeleton — lexer + parser implemented in Tasks 2 + 3)*