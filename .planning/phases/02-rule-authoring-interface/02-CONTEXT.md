# Phase 2: Rule Authoring Interface - Context

**Gathered:** 2026-05-23
**Status:** Ready for planning

## Phase Boundary

Provide intuitive web-based tools for business users and developers to create, edit, and validate rules. This includes a full-featured IDE with syntax highlighting, spreadsheet-style decision tables for non-technical users, form-based guided editors, rule templates with parameterization, and inline validation.

## Requirements (locked by Phase 1 decisions)

Phase 1 established:
- Hybrid approach: Both API and file-based DSL for rule definition
- Explicit session lifecycle: Stateful/stateless execution modes
- Automatic chaining mode selection

This phase focuses on the UI layer that exposes these capabilities to users.

## Implementation Decisions

### IDE Design Patterns (D-04)
- **IDE-A1:** CodeMirror 6 as editor architecture - lightweight, extensible, React-friendly for DSL syntax highlighting
- **IDE-A2:** Hybrid validation approach - syntax real-time as-you-type, semantic validation on save
- **IDE-A3:** Dual-pane mode workflow - both free-form code editor and guided view visible side-by-side

### Decision Table UI (D-05)
- **DT-U1:** ag-Grid spreadsheet library for enterprise-grade features and performance
- **DT-U2:** Mixed editing approach - simple cases use dropdown selectors, advanced users can edit formulas directly
- **DT-U3:** Full CRUD on rows - insert/delete/move with drag-and-drop reordering

### Guided Editor Flow (D-06)
- **GE-F1:** Hybrid accordion structure - start with all steps visible in expandable sections, collapse completed ones after review
- **GE-F2:** Adaptive display for business vocabulary - technical names by default with toggle to business terms
- **GE-F3:** Smart defaults with override - auto-generate rule name/description but allow easy editing

### Rule Templates (D-06)
- **RT-P1:** Class-based inheritance for parameterization - templates define base structure; instances extend with specific values
- **RT-S1:** User-private + shared storage model - users have private templates plus access to team-shared ones
- **RT-V1:** Full version history - track changes to templates with rollback capability

### Claude's Discretion
- Specific UI component library (MUI, Chakra, Tailwind) left to planner/researcher evaluation
- State management approach (Redux, Zustand, Context API) can be determined during implementation
- Exact styling approach (CSS Modules, styled-components, Tailwind classes)

## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project-Level
- `.planning/PROJECT.md` — Core value, constraints (Rust backend, React frontend), hybrid Git+DB storage
- `.planning/REQUIREMENTS.md` — Phase 2 covers RULE-01 through RULE-06

### Phase 1 Context (locked decisions)
- `.planning/phases/01-core-engine-foundation/01-CONTEXT.md` — Hybrid approach, session management, chaining mode selection

### Phase 2 Requirements
- `RULE-01`: Web-based IDE for rule authoring with syntax highlighting
- `RULE-02`: Decision table editor (spreadsheet-style grids)
- `RULE-03`: Guided rule editor (form-based UI for business users)
- `RULE-04`: Rule templates with parameterization for reuse
- `RULE-05`: Expression language support (MVEL-like syntax)
- `RULE-06`: Inline rule validation and error reporting

### Roadmap
- `.planning/ROADMAP.md` — Phase 2 details, success criteria, and plan breakdown

## Existing Code Insights

### Reusable Assets
(None yet — first phase of implementation; Phase 1 will establish engine patterns)

### Established Patterns
(None yet — initial project setup)

### Integration Points
Phase 2 UI must integrate with:
- Phase 1 Engine API - for rule execution, session management, fact operations
- Future Phase 3 REST API - for CRUD operations on rules
- Hybrid Git + PostgreSQL storage (Git for versioning, DB for metadata per PROJECT.md)

## Specific Ideas

### Dual-Pane Editor Architecture
```
┌─────────────────────────┬──────────────────────────────┐
│    Code Editor          │   Guided Rule Builder        │
│  (CodeMirror 6)         │                                │
│  - Syntax highlighting  │  Form fields for:            │
│  - Real-time syntax     │    • Entity selection        │
│    validation           │    • Condition type          │
│  - Code folding         │    • Value inputs            │
│                         │    • Actions                 │
└─────────────────────────┴──────────────────────────────┘
```

### Decision Table Structure
- Each row = one rule in decision table format
- Columns: Conditions (left side), Actions (right side)
- Mixed editing: simple conditions via dropdown, complex via formula cell
- Row operations: insert/delete/move with drag-and-drop

### Template Parameterization Example
```
# Template base class
template "high-value-customer" {
  condition: "{customer_field} > {threshold_value}"
  actions: [
    add_tag("high_priority"),
    apply_discount({discount_percent})
  ]
}

# Instance extending template
rule " VIP customer discount" extends "high-value-customer" {
  customer_field = "annual_spend"
  threshold_value = 10000
  discount_percent = 15
}
```

## Deferred Ideas

### Reviewed Todos (not folded)
(None — no todos were cross-referenced or reviewed)

### General
(None — discussion stayed within phase scope)

---

*Phase: 2-Rule Authoring Interface*
*Context gathered: 2026-05-23*
