# Phase 2: Rule Authoring Interface - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-05-23
**Phase:** 2-Rule Authoring Interface
**Areas discussed:** IDE design patterns, Decision table UI, Guided editor flow, Rule templates

---

## IDE Design Patterns

| Option | Description | Selected |
|--------|-------------|----------|
| CodeMirror 6 | Lightweight, extensible, React-friendly; good for DSL syntax highlighting | ✓ |
| Monaco Editor | VS Code's engine - rich features but larger bundle size | |
| Custom solution | Build from scratch using React and textarea for maximum control | |

**User's choice:** CodeMirror 6

---

## IDE Validation Feedback

| Option | Description | Selected |
|--------|-------------|----------|
| Real-time as-you-type | Instant feedback on syntax errors while typing | |
| On-save validation | Validate only when user clicks save button | |
| Hybrid approach | Syntax real-time, semantic validation on save | ✓ |

**User's choice:** Hybrid approach

---

## IDE Workflow

| Option | Description | Selected |
|--------|-------------|----------|
| Free-form editor first | Start with raw code editor; guided tools available via toggle | |
| Guided wizard first | Start with form-based creation; switch to code view when ready | |
| Dual-pane mode | Both views side-by-side - edit anywhere, both update | ✓ |

**User's choice:** Dual-pane mode

---

## Decision Table Grid Library

| Option | Description | Selected |
|--------|-------------|----------|
| React Table (tanstack) | Lightweight, flexible, good for custom cell rendering | |
| ag-Grid | Feature-rich but larger; enterprise-grade features | ✓ |
| Custom grid | Build simple spreadsheet-style grid from scratch | |

**User's choice:** ag-Grid

---

## Decision Table Condition Editing

| Option | Description | Selected |
|--------|-------------|----------|
| Formula cells | Users type formulas directly like Excel (e.g., `age > 18 && income < 50k`) | |
| Dropdown selectors | Form-style dropdowns for operators and values | |
| Mixed approach | Simple cases use dropdowns; advanced users can edit formula directly | ✓ |

**User's choice:** Mixed approach

---

## Decision Table Row Operations

| Option | Description | Selected |
|--------|-------------|----------|
| Insert/delete/move rows | Full CRUD operations with drag-and-drop reordering | ✓ |
| Limited operations | Only insert and delete - no row movement after creation | |
| Template-based additions | Add from predefined action templates only | |

**User's choice:** Insert/delete/move rows

---

## Guided Editor Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Step-by-step wizard | Multiple pages: Select entity → Choose condition type → Set values → Review | |
| Single-page accordion | All steps visible in expandable sections on one page | |
| Hybrid approach | Start with accordion; collapse completed steps after review | ✓ |

**User's choice:** Hybrid approach

---

## Guided Editor Vocabulary

| Option | Description | Selected |
|--------|-------------|----------|
| Technical terms (field names, operators) | Uses actual data field names and programming-like operators | |
| Business-friendly terms | Maps technical fields to business terminology (e.g., 'age' → 'customer years') | |
| Adaptive display | Show technical names by default; toggle for business terms | ✓ |

**User's choice:** Adaptive display

---

## Guided Editor Metadata

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, automatically | Generate from condition values and action type | |
| No, user must enter manually | Name and description always require user input | |
| Smart defaults with override | Auto-generate but allow easy editing | ✓ |

**User's choice:** Smart defaults with override

---

## Template Parameterization

| Option | Description | Selected |
|--------|-------------|----------|
| Variable substitution syntax | Use placeholders like ${variable} in rule bodies, filled at usage time | |
| JSON config with defaults | Template stores default values; users override via JSON merge | |
| Class-based inheritance | Templates define base class; instances extend with specific values | ✓ |

**User's choice:** Class-based inheritance

---

## Template Storage Model

| Option | Description | Selected |
|--------|-------------|----------|
| Project-level only | All users in project share same template library | |
| User-private + shared | Users can have private templates plus access to team-shared ones | ✓ |
| Global system templates | Pre-built templates for common patterns available to all | |

**User's choice:** User-private + shared

---

## Template Versioning

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, full version history | Track changes to templates with rollback capability | ✓ |
| No, only template library version | Templates can be added/removed but not individually versioned | |
| Opt-in versioning | Users can enable versioning per-template | |

**User's choice:** Full version history

---

## Claude's Discretion

- Specific UI component library (MUI, Chakra, Tailwind) for base styling
- State management approach (Redux, Zustand, Context API)
- Exact CSS/styling methodology (CSS Modules, styled-components, Tailwind)

---

## Deferred Ideas

(None - discussion stayed within phase scope)
