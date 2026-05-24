---
phase: 01-core-engine-foundation
plan: 04
type: execute
wave: 3
depends_on:
  - "01-01"
  - "01-02"
  - "01-03"
files_modified:
  - src/engine/dsl/parser.rs
  - src/engine/dsl/lexer.rs
  - src/engine/dsl/rule_syntax.rs
  - src/api/rules/route.rs
  - tests/integration/hot_reload.rs
autonomous: true
requirements:
  - CORE-01
  - CORE-08
user_setup: []
must_haves:
  truths:
    - "Hybrid rule definition supports both API and file-based DSL"
    - "Syntax validation catches errors before compilation"
    - "KIE base caches compiled rules for efficient reuse"
    - "Hot reload updates KIE base without service restart"
  artifacts:
    - path: "src/engine/dsl/parser.rs"
      provides: "DSL parser converting rule text to AST"
      exports: ["parse_rule", "parse_ruleset"]
    - path: "src/engine/dsl/lexer.rs"
      provides: "Token lexer for DSL syntax"
      exports: ["tokenize", "TokenType"]
    - path: "src/engine/dsl/rule_syntax.rs"
      provides: "Syntax definitions and AST types"
      exports: ["RuleAst", "ConditionNode", "ActionNode"]
    - path: "src/api/rules/route.rs"
      provides: "API endpoints for rule CRUD operations"
      exports: ["POST /rules", "GET /rules/{id}", "PUT /rules/{id}", "DELETE /rules/{id}"]
    - path: "tests/integration/hot_reload.rs"
      provides: "Hot reload integration tests"
      tests: ["reload_after_rule_change", "concurrent_sessions_survive_reload"]
  key_links:
    - from: "src/engine/dsl/parser.rs"
      to: "src/engine/rules.rs"
      via: "AST to Rule conversion"
      pattern: "RuleAst::to_rule|convert_ast_to_rule"
    - from: "src/api/rules/route.rs"
      to: "src/engine/kie_cache.rs"
      via: "Rule compilation and caching"
      pattern: "compile_rules.*KieCache|cache_ruleset"
    - from: "tests/integration/hot_reload.rs"
      to: "src/engine/execution/results.rs"
      via: "Verify execution after reload"
      pattern: "execute_after_reload.*fired_rules"
---

## Phase 1 Plan 04: Hybrid Rule Definition & DSL

<objective>
Implement hybrid rule definition system supporting both programmatic API and file-based DSL with syntax validation. This plan enables users to define rules either via code or declaratively while ensuring all inputs are validated before compilation.

Purpose: Per D-10, the engine must support both API (programmatic) and file-based DSL (declarative) entry points that converge to the same compiled representation. Per D-11, syntax + semantic validation catches errors early.
Output: DSL lexer/parser, rule CRUD API endpoints, validation pipeline integrated with compilation flow.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/01-core-engine-foundation/01-PLAN.md (Plan 01)
@.planning/phases/01-core-engine-foundation/02-PLAN.md (Plan 02)
@.planning/phases/01-core-engine-foundation/03-PLAN.md (Plan 03)
@.planning/phases/01-core-engine-foundation/01-CONTEXT.md

**Dependencies:**
- D-10: Hybrid approach — API for programmatic, file-based DSL for declarative
- D-11: Syntax + semantic validation before compilation
- D-12: Hot reload support for KIE base without service restart
- CORE-08: Caching of compiled rule sets (KIE bases)
</context>

<tasks>

<task type="auto">
  <name>Implement DSL lexer and token types per D-10, D-11</name>
  <files>src/engine/dsl/lexer.rs, src/engine/dsl/tokens.rs</files>
  <action>Create the lexical analysis layer for rule expressions:

1. Implement `TokenType` enum:
   - Keyword: RULE, WHEN, THEN, IF, END
   - Comparison: EQ, NEQ, LT, GT, LTE, GTE
   - Boolean: AND, OR, NOT
   - Literal: IDENT, STRING, INTEGER, FLOAT, TRUE, FALSE
   - Punctuation: LPAREN, RPAREN, LBRACE, RBRACE, COMMA, SEMICOLON

2. Implement `Token` struct:
   - token_type: TokenType
   - span: SourceSpan (start/end positions)
   - lexeme: String (original text)

3. Implement `Lexer::tokenize(input: &str) -> Result<Vec<Token>, LexError>`:
   - Handle multi-character operators (==, !=, <=, >=)
   - Skip whitespace and comments
   - Report source location for errors

4. Add error handling:
   - LexicalError with position information
   - Expected token recovery suggestions

This provides the foundation for syntax validation per D-11.</action>
  <verify>Lexer converts rule text to tokens; errors include source positions</verify>
  <done>Lexing module compiles and handles all required token types</done>
</task>

<task type="auto">
  <name>Implement DSL parser and AST construction per D-10, D-11</name>
  <files>src/engine/dsl/parser.rs, src/engine/dsl/ast.rs</files>
  <action>Create the parsing layer converting tokens to Rule AST:

1. Implement `RuleAst` struct:
   - name: String
   - salience: i32 (default 0)
   - condition: Option<ConditionNode>
   - actions: Vec<ActionNode>

2. Implement `ConditionNode` enum:
   - And(Vec<ConditionNode>)
   - Or(Vec<ConditionNode>)
   - Not(Box<ConditionNode>)
   - Comparison(Variable, Operator, Value)
   - PatternMatch(FactType, FieldPattern)

3. Implement `ActionNode` enum:
   - InsertFact { fact_type: String, fields: HashMap<String, String> }
   - UpdateFact { fact_id: String, updates: HashMap<String, String> }
   - RetractFact { fact_id: String }

4. Implement `Parser::parse(tokens: &[Token]) -> Result<RuleAst, ParseError>`:
   - Recursive descent parsing
   - Precedence handling for boolean operators
   - Error recovery with helpful messages

5. Add validation:
   - Variable type checking (vs FactType schema)
   - Duplicate variable detection

This enables D-10's DSL rule definition.</action>
  <verify>Parser converts tokens to AST; errors include helpful messages and locations</verify>
  <done>Parser compiles with recursive descent and proper operator precedence</done>
</task>

<task type="auto">
  <name>Implement rule API endpoints per D-10, CORE-08</name>
  <files>src/api/rules/route.rs, src/api/mod.rs</files>
  <action>Create REST API for rule management:

1. Implement `POST /rules`:
   - Accept: JSON with rule definition or DSL string
   - Validate syntax (per D-11)
   - Compile to Rule and add to KIE base
   - Return: { id, name, salience }

2. Implement `GET /rules/{id}`:
   - Retrieve rule by ID
   - Include compilation status

3. Implement `PUT /rules/{id}`:
   - Update existing rule (versioning per GOV-02)
   - Validate and recompile

4. Implement `DELETE /rules/{id}`:
   - Remove from KIE base
   - Trigger hot reload (per D-12)

5. Add error responses:
   - 400 Bad Request (validation errors)
   - 409 Conflict (rule ID exists on create)
   - 404 Not Found

6. Export API module from lib.rs.

This provides the programmatic entry point per D-10.</action>
  <verify>API routes compile; POST creates rule, GET retrieves it, DELETE removes</verify>
  <done>All four CRUD endpoints implemented with proper validation</done>
</task>

<task type="auto">
  <name>Add syntax validation pipeline per D-11</name>
  <files>src/engine/validation/syntax.rs, src/engine/validation/pipeline.rs</files>
  <action>Create comprehensive validation pipeline before compilation:

1. Implement `SyntaxValidator`:
   - Token validation (balanced parentheses, brackets)
   - Keyword ordering (RULE must start, WHEN before THEN)
   - Required fields check
   - Literal type correctness

2. Add semantic validation (inherited from Plan 01):
   - FactType references exist in registry
   - Field types match fact schema
   - Circular dependencies between rules detected

3. Implement `ValidationPipeline`:
   - run_syntax(input: &str) → ValidationResult
   - run_semantic(ast: &RuleAst) → ValidationResult
   - run_all(input: &str) → ValidationResult (combined)

4. Add error aggregation:
   - Collect all errors instead of failing fast
   - Group by severity (error vs warning)
   - Include suggestion for common mistakes

5. Integration with API endpoints to reject invalid rules.

This enforces D-11: "Syntax + semantic validation before compilation."</action>
  <verify>Validation rejects malformed rules; helpful suggestions provided</verify>
  <done>Syntax and semantic validators produce ValidationResult with collected errors</done>
</task>

<task type="auto">
  <name>Implement hot reload for KIE base per D-12, CORE-08</name>
  <files>src/engine/hot_reload.rs, src/engine/kie_base_updater.rs</files>
  <action>Create mechanism to update KIE base without service restart:

1. Implement `KieBaseUpdater`:
   - load_new_rules(new_rule_set: RuleSet) → Result<KieBase>
   - transition_to_new(old: &Arc<RwLock<KieBase>>, new: KieBase)
   - Graceful handling of active executions

2. Add version tracking:
   - KIE base version number
   - Rule set hash for change detection
   - Version history (configurable retention)

3. Implement atomic swap:
   - Build new KIE base in memory
   - Swap Arc pointer atomically
   - Old base cleaned up via reference counting

4. Add reload triggers:
   - API endpoint: POST /rules/reload
   - File watcher for external DSL files
   - Manual refresh request

5. Handle edge cases:
   - Active sessions using old KIE base (allow to complete)
   - Concurrent reload requests (queue or reject)

This satisfies D-12: "Hot reload support — KIE base updates dynamically."</action>
  <verify>Hot reload swaps KIE base; active sessions continue with previous version</verify>
  <done>KieBaseUpdater compiles and handles graceful transition between versions</done>
</task>

<task type="auto">
  <name>Create DSL file loader for external rule files per D-10, D-12</name>
  <files>src/engine/dsl/file_loader.rs</files>
  <action>Create system to load rules from external DSL files:

1. Implement `FileRuleLoader`:
   - watch_directory(path: &Path) → Result<()>
   - on_file_change(path: &Path, content: String)
   - parse_and_load_rules(content: &str) → Result<(), ValidationError>

2. Support multiple file formats:
   - .rules (custom DSL)
   - .json (rule definitions as JSON)
   - .yaml (YAML rule definitions)

3. Add hot reload integration:
   - File watcher triggers KieBaseUpdater
   - Error handling with rollback to previous version

4. Configuration via EngineBuilder:
   - rules_dir: Option<PathBuf>
   - watch_enabled: bool

5. File naming conventions for ordering:
   - Prefix numbers for load order (01-rules.rules, 02-override.rules)

This enables D-10's file-based DSL approach.</action>
  <verify>FileLoader reads .rules files; changes trigger hot reload</verify>
  <done>File loader compiles with directory watching and automatic reloading</done>
</task>

<task type="auto">
  <name>Create integration tests for DSL and hot reload per D-10, D-12</name>
  <files>tests/integration/hot_reload.rs, tests/integration/dsl_loading.rs</files>
  <action>Write integration tests:

1. Hot reload tests (`hot_reload.rs`):
   - reload_after_rule_change: KIE base updates with new rule
   - concurrent_sessions_survive_reload: Active sessions complete without error
   - rollback_on_validation_error: Invalid update reverts to previous version

2. DSL loading tests (`dsl_loading.rs`):
   - parse_ruleset_from_file: File loader reads and parses .rules file
   - syntax_errors_reported_with_positions: Error includes line/column
   - api_and_dsl_converge_same_compiled: Programmatic and DSL rules behave identically

3. Add test fixtures:
   - Sample .rules files with valid/invalid syntax
   - Rule definitions in both API format and DSL format for comparison

These tests verify the hybrid definition approach works correctly.</action>
  <verify>All integration tests pass; DSL and API produce same results</verify>
  <done>DSL and hot reload test suites compile and pass</done>
</task>

<task type="auto">
  <name>Add rule compilation metrics per MON-02 (early implementation)</name>
  <files>src/engine/compilation_stats.rs</files>
  <action>Create metrics for rule compilation performance:

1. Implement `CompilationMetrics`:
   - parse_time_ms: Duration
   - validate_time_ms: Duration
   - compile_time_ms: Duration
   - total_time_ms: Duration

2. Add tracking per operation:
   - Rules processed count
   - Errors/warnings count
   - Memory allocated for AST

3. Export methods:
   - get_avg_compile_time() → f64
   - get_error_rate() → f64
   - reset_stats()

4. Integration with API endpoints to return metrics.

This provides early data for MON-02 compliance in Phase 5.</action>
  <verify>CompilationMetrics tracks all timing data; methods return valid statistics</verify>
  <done>Metric collection compiles and updates during rule compilation</done>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| File system → DSL parser | External files with rule definitions |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-01-04-01 | D | Malicious file content in external rules | mitigate | Syntax validation rejects malformed input; resource limits on expression depth |
| T-01-04-02 | E | Hot reload causing service instability | accept | Atomic swap with reference counting prevents memory leaks; rollback on error |
| T-01-04-03 | I | KIE base version confusion | mitigate | Explicit version numbering; API returns current version |
| T-01-04-04 | D | File watcher race condition | accept | Race handled by file hash comparison; only actual changes trigger reload |
| T-01-04-05 | S | External DSL files expose sensitive paths | accept | File paths configurable via EngineBuilder; no path disclosure in errors |

**Note:** File watching runs with same permissions as engine process. Users must ensure external rule directories have appropriate access controls.
</threat_model>

<verification>
- All integration tests pass: `cargo test --test hot_reload --test dsl_loading`
- Hot reload verified via file watcher triggering API call
- DSL parsing produces same Rule structure as programmatic creation

## Success Criteria

1. **Hybrid rule definition** — Both API and DSL entry points produce identical compiled rules per D-10
2. **Syntax validation before compilation** — Invalid rules rejected with helpful error messages per D-11
3. **Hot reload without restart** — KIE base updates dynamically via API endpoint or file watcher per D-12

---

## Implementation Notes for Phase 1 Plan 04

### DSL Example (Sample)
```rust
// Sample .rules file syntax
RULE "High Value Customer Discount"
salience: 10
when:
    customer.type == "premium" AND customer.spend > 1000
then:
    apply_discount(customer.id, 0.15)

RULE "Standard Discount"
salience: 5
when:
    customer.type == "standard"
then:
    apply_discount(customer.id, 0.05)
```

### API Request Example
```json
POST /rules
{
  "name": "High Value Customer",
  "salience": 10,
  "condition": {
    "and": [
      {"field": "customer.type", "op": "==", "value": "premium"},
      {"field": "customer.spend", "op": ">", "value": 1000}
    ]
  },
  "actions": [{"type": "apply_discount", "customer_id": "customer.id"}]
}
```

### Open Questions for Implementation
1. **DSL expression language** — What operators/functions to support? Need FEEL subset or custom?
2. **Multi-file rule loading order** — Should files be merged or overridden? Documented behavior needed.
3. **Version control integration** — How do rules relate to Git versioning per PROJECT.md?
