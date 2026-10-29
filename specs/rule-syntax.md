# Rule syntax — the text form of a rule

**Status:** shipped in 9.0.82 — `engine::rule_syntax::parse_rule_text` and `render_rule_text`, with
`engine/tests/rule_syntax.rs` holding the round trip. `POST /v1/rules` compiles a `dsl_body` that parses, so a
rule can be authored as text, as a form, or as JSON. This document is the grammar and the reasoning; where it
and the parser disagree, the parser is wrong.

## Why a syntax at all

Two things in rRules are called "a rule": a **document** (`POST /v1/rules`) and a rule the **engine
executes** (`specs/rule-sources.md`). Documents carry a `dsl_body`; for several releases that body was a
pseudo-DSL that **nothing parsed** — the interface stored it and no engine saw it, so a rule created through
the product could never fire.

The interface now builds a structured rule and renders the body from it (`frontend/src/api/rule-builder.ts`),
so the two cannot disagree. This grammar makes the text *authorable* as well: the body a person types can be
compiled into the same `Rule` the interface produces, and then the ordering stops mattering — text and form
are two surfaces on one object, not two objects.

## Design rule

**The grammar is a faithful surface for `engine::rules::{Condition, Action}` and nothing more.** It adds no
semantics of its own: everything it accepts has a direct representation in the AST, and everything the AST
expresses that the grammar cannot (arbitrary nesting, all five action kinds) is simply a smaller surface —
not a different meaning. When the two could drift, the AST wins and the grammar grows to fit it.

## Grammar

```
program     := line* EOF
line        := blank | comment | header | condition | extra-condition | action
blank       := WS*
comment     := WS* "//" any*
header      := WS* "rule" WS quoted WS*
condition   := WS* "when" WS expr WS*
extra       := WS* ("and" | "or") WS expr WS*
expr        := ref WS op WS literal
             | "not" WS "(" expr ")"
ref         := ident "." ident
op          := "=" | "==" | "!=" | ">" | ">=" | "<" | "<="
literal     := quoted | number | "true" | "false"
action      := WS* "then" WS call WS*
call        := "log" "(" quoted ")"
quoted      := '"' (escaped-char | any-but-quote)* '"'
number      := "-"? digit+ ("." digit+)?
ident       := (letter | "_") (letter | digit | "_" | "-")*
```

- The header sets the rule's `name`. `id` is derived from the name (§ Identity).
- A `when` line starts the condition; each following `and`/`or` line conjoins or disjoins one more
  comparison at the same level. Mixed `and`/`or` at one level is **rejected**: the grammar has no
  precedence rule, and inventing one silently would make two authors' readings differ. Use `not(…)` for
  grouping, which is what the form can express.
- `then log("…")` is the only action the grammar carries, because it is the only one the interface
  produces. `InsertFact`/`UpdateFact`/`RetractFact`/`CallFunction` are expressible in the AST and in JSON;
  adding them here is a deliberate extension, not an oversight — a text form for `UpdateFact` needs
  arithmetic expressions, which is a bigger language than this one.
- Values: a quoted string is `Value::String`; a bare integer is `Value::Integer`; a decimal is
  `Value::Float`; `true`/`false` are `Value::Bool`. **A quoted number is a string** — the form's type selector
  exists for the same reason.

## Identity

`parse_rule_text` returns a `Rule` whose `id` **equals its `name`**: the API imposes the document's UUID on
whatever it is given (`write_executable_rule`), so the parser's id is only a placeholder that keeps the rule
valid on its own. Nothing downstream may rely on it.

## Errors

Every error carries the 1-based line number and what was expected, e.g.

```
line 2: expected a comparison such as `customer.status = "premium"`, found `when`
```

The parser is total: any input yields either a `Rule` or an error, never a panic, and never a partially
built rule. Long input is bounded by the same token budget the FEEL parser uses, so a body cannot be a
denial-of-service vector through a rules file.

## What it is not

- **Not a scripting language.** There are no variables, no expressions, no arithmetic.
- **Not a second semantics.** `parse(render(rule)) == rule` for every rule the interface can build, and that
  equality is a test (`engine/tests/rule_syntax.rs`), not a claim.
- **Not required.** A rule can be created entirely through the form, or entirely through the API's JSON. The
  grammar exists so text is a first-class way to author, not the only one.
