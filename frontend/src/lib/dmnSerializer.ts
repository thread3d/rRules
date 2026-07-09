// Editor ↔ DMN IR serializer (Phase 34, Plan 02).
//
// Single source of truth for converting between the editor-friendly column/row
// shape and the backend's canonical DMN IR (per engine/src/dmn/model.rs).
//
// Pitfall avoidance:
// - Column `accessorKey` IS the input expression name. Use it directly as the
//   input key when building evaluate requests — never prefix with `input_${idx}_`.
// - HitPolicy values are EXACTLY DMN spec strings (uppercase, e.g. "RULE ORDER").
// - COLLECT without aggregator returns array of outputs; frontend must handle
//   both scalar and list output shapes in Plan 03.

import type { HitPolicy, Aggregator } from '@/features/rules/types'

// ---- Editor shape (what the React component tree owns): ----

export interface AllowedValueItem {
  id: string
  text: string
}

export interface EditorShapeColumn {
  id: string
  label: string
  accessorKey?: string // used as input_expression for conditions
  type: 'condition' | 'action'
  allowedValues?: AllowedValueItem[] // populated by dmnIrToEditorShape for PRIORITY/OUTPUT ORDER columns
}

export interface EditorShapeRow {
  id: string
  conditions: Record<string, unknown>
  actions: Record<string, unknown>
}

/** Editor-friendly shape owned by the React component tree. */
export interface EditorShape {
  columns: EditorShapeColumn[]
  rows: EditorShapeRow[]
  hitPolicy: HitPolicy
  aggregator?: Aggregator // only meaningful when hitPolicy === 'COLLECT'
}

// ---- DMN IR shapes (mirror engine/src/dmn/model.rs): ----

export interface DmnInputClause {
  id?: string | null
  label?: string | null
  input_expression: string
  input_values?: unknown // AllowedValues — not used in MVP
  description?: unknown | null
}

export interface DmnOutputClause {
  id?: string | null
  label?: string | null
  name?: string | null
  type_ref?: string | null
  output_values?: AllowedValuesShape // Enumeration or UnaryTests — see below
  default_output_entry?: string | null
  description?: unknown | null
}

/** DMN AllowedValues shape (simplified for JSON transport). */
export interface AllowedValuesShape {
  kind: 'enumeration' | 'unary_tests'
  values: Array<{ content: string; is_cdata: boolean }>
}

export interface DmnRule {
  id?: string | null
  description?: unknown | null
  input_entries: Array<{ text: { content: string; is_cdata: boolean } }>
  output_entries: Array<{ text: { content: string; is_cdata: boolean } }>
  annotation_entries: Array<{ text: { content: string; is_cdata: boolean } }>
}

export interface DmnDecisionTable {
  id?: string | null
  name?: string | null
  hit_policy: HitPolicy // canonical DMN spec string
  aggregation?: 'SUM' | 'MIN' | 'MAX' | 'COUNT' // only for COLLECT
  preferred_orientation?: string | null
  output_label?: string | null
  input_clauses: DmnInputClause[]
  output_clauses: DmnOutputClause[]
  rules: DmnRule[]
}

// ---- Mapper functions (the contract): ----

/** Convert editor shape → DMN IR for evaluation or save. */
export function editorShapeToDmnIr(
  columns: EditorShape['columns'],
  rows: EditorShape['rows'],
  hitPolicy: HitPolicy,
  aggregator?: Aggregator,
): DmnDecisionTable {
  const inputClauses: DmnInputClause[] = []
  const outputClauses: DmnOutputClause[] = []

  // Build clauses from columns.
  for (const col of columns) {
    if (col.type === 'condition') {
      inputClauses.push({
        label: col.label,
        input_expression: col.accessorKey ?? col.label,
      })
    } else {
      const clause: DmnOutputClause = {
        label: col.label,
        name: col.accessorKey ?? col.label,
      }
      // If the column carries allowedValues (from AllowedValuesEditor in Plan 03),
      // map them into output_values as an enumeration.
      if ((col as EditorShapeColumn & { allowedValues?: Array<{ id: string; text: string }> }).allowedValues) {
        const av = (col as EditorShapeColumn & { allowedValues: Array<{ id: string; text: string }> }).allowedValues!
        clause.output_values = {
          kind: 'enumeration',
          values: av.map((item) => ({ content: item.text, is_cdata: false })),
        }
      }
      outputClauses.push(clause)
    }
  }

  // Build rules from rows.
  const rules: DmnRule[] = rows.map((row) => {
    // Use accessorKey (input_expression name) as the row condition key — this is
    // what the frontend editor uses internally and matches dmnIrToEditorShape's
    // reverse lookup. Falls back to label when no accessorKey was set.
    const inputEntries = inputClauses.map((clause) => {
      const key = clause.input_expression ?? clause.label ?? ''
      const value = row.conditions[key]
      return {
        text: {
          content: String(value ?? ''),
          is_cdata: false,
        },
      }
    })

    // Use name (accessorKey for actions) as the row action key.
    const outputEntries = outputClauses.map((clause) => {
      const key = clause.name ?? clause.label ?? ''
      const value = row.actions[key]
      return {
        text: {
          content: String(value ?? ''),
          is_cdata: false,
        },
      }
    })

    return {
      id: undefined,
      input_entries: inputEntries,
      output_entries: outputEntries,
      annotation_entries: [],
    }
  })

  const result: DmnDecisionTable = {
    hit_policy: hitPolicy,
    input_clauses: inputClauses,
    output_clauses: outputClauses,
    rules,
  }

  // Only set aggregation for COLLECT.
  if (hitPolicy === 'COLLECT' && aggregator) {
    result.aggregation = aggregator
  }

  return result
}

/** Convert parsed DMN IR → editor shape for rendering. */
export function dmnIrToEditorShape(table: DmnDecisionTable): EditorShape {
  const columns: EditorShapeColumn[] = []

  // Map input_clauses to condition columns.
  for (const clause of table.input_clauses) {
    columns.push({
      id: clause.id ?? `input-${clause.label ?? clause.input_expression}`,
      label: clause.label ?? clause.input_expression,
      accessorKey: clause.input_expression,
      type: 'condition',
    })
  }

  // Map output_clauses to action columns.
  for (const clause of table.output_clauses) {
    const col: EditorShapeColumn = {
      id: clause.id ?? `output-${clause.label ?? clause.name ?? ''}`,
      label: clause.label ?? clause.name ?? '',
      accessorKey: clause.name ?? clause.label ?? undefined,
      type: 'action',
    }
    // If output_values is present (enumeration), extract into allowedValues.
    if (clause.output_values && clause.output_values.kind === 'enumeration') {
      col.allowedValues = clause.output_values.values.map((v) => ({
        id: crypto.randomUUID(),
        text: v.content,
      }))
    }
    columns.push(col)
  }

  // Map rules back to rows.
  const rows: EditorShapeRow[] = table.rules.map((rule, idx) => {
    const conditions: Record<string, unknown> = {}
    const actions: Record<string, unknown> = {}

    // Use input_expression (the accessorKey-equivalent key) for condition lookup —
    // matches what editorShapeToDmnIr writes.
    for (let i = 0; i < table.input_clauses.length && i < rule.input_entries.length; i++) {
      const clause = table.input_clauses[i]
      const key = clause.input_expression ?? clause.label ?? ''
      conditions[key] = rule.input_entries[i].text.content
    }

    // Use name (accessorKey for actions) for action lookup.
    for (let i = 0; i < table.output_clauses.length && i < rule.output_entries.length; i++) {
      const clause = table.output_clauses[i]
      const key = clause.name ?? clause.label ?? ''
      actions[key] = rule.output_entries[i].text.content
    }

    return {
      id: rule.id ?? `rule-${idx}`,
      conditions,
      actions,
    }
  })

  return {
    columns,
    rows,
    hitPolicy: table.hit_policy,
    aggregator: table.aggregation,
  }
}
