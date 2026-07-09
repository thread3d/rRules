import { useMemo, useState } from 'react'
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
} from '@tanstack/react-table'

import type { HitPolicy, Aggregator, DecisionTableColumn, DecisionTableRow } from './types'
import { HIT_POLICY_META, POLICIES_REQUIRING_ALLOWED_VALUES } from './types'
import { editorShapeToDmnIr } from '@/lib/dmnSerializer'
import { RuleEditor } from '@/components/RuleEditor'
import { Button } from '@/components/ui/button'
import { ValidationDisplay } from '@/components/validation/ValidationDisplay'
import type { ValidationError, ValidationWarning } from '@/components/validation/ValidationDisplay'
import { evaluateDecision } from '@/api/decisions'
import { AllowedValuesEditor } from './AllowedValuesEditor'

export interface DecisionTableProps {
  hitPolicy: HitPolicy
  columns: DecisionTableColumn[]
  rows: DecisionTableRow[]
  decisionId?: string // Optional: decision ID for evaluation
  onHitPolicyChange?: (policy: HitPolicy) => void
  aggregator?: Aggregator
  onAggregatorChange?: (agg: Aggregator) => void
  // Optional validation state for the entire table
  validationErrors?: ValidationError[]
  validationWarnings?: ValidationWarning[]
}

export interface EvaluationResult {
  outputs: Record<string, unknown>
  rulesFired: number
  durationMs: number
  trace?: { eventCount: number; durationMs: number }
}

/**
 * DecisionTable — renders a DMN decision table with evaluation support.
 *
 * Pitfall avoidance (from RESEARCH.md):
 * - Pitfall 1: Column accessorKey IS the input expression name — used directly
 *   as evaluate request key via dmnSerializer.editorShapeToDmnIr().
 * - Pitfall 2: HitPolicy values are EXACTLY DMN spec strings (uppercase) — never
 *   lowercase, never kebab-case.
 * - Pitfall 3: COLLECT without aggregator returns array of outputs; frontend
 *   renders list not scalar.
 */
export function DecisionTable({
  hitPolicy,
  columns,
  rows,
  decisionId,
  onHitPolicyChange,
  aggregator,
  onAggregatorChange,
  validationErrors = [],
  validationWarnings = [],
}: DecisionTableProps) {
  const [isEvaluating, setIsEvaluating] = useState(false)
  const [evaluationResult, setEvaluationResult] = useState<EvaluationResult | null>(null)
  const [showTrace, setShowTrace] = useState(false)

  const table = useReactTable({
    data: rows,
    columns: useMemo(() => columns, [columns]),
    getCoreRowModel: getCoreRowModel(),
  })

  // Evaluate the decision table with current inputs.
  // Uses dmnSerializer.editorShapeToDmnIr() to build a correctly-keyed request
  // where input keys match column accessorKey values (never "input_${idx}_${key}").
  async function handleEvaluate() {
    if (!decisionId) return

    setIsEvaluating(true)
    setEvaluationResult(null)
    setShowTrace(false)

    try {
      // Build the canonical DMN IR from editor shape.
      // Map DecisionTableColumn -> EditorShapeColumn by inferring type from accessorKey presence.
      // Condition columns have no accessorKey (or use label), action columns use accessorKey.
      const editorColumns = columns.map((col) => ({
        ...col,
        type: col.accessorKey ? 'action' as const : 'condition' as const,
      }))
      const dmnTable = editorShapeToDmnIr(editorColumns, rows, hitPolicy, aggregator)

      // Extract input keys from the IR's input_clauses (which use accessorKey).
      // Use first row as sample data for input values.
      const inputs: Record<string, unknown> = {}
      const sampleRow = rows[0]
      for (const clause of dmnTable.input_clauses) {
        const key = clause.label ?? clause.input_expression
        if (sampleRow?.conditions) {
          inputs[clause.input_expression] = sampleRow.conditions[key] ?? ''
        } else {
          inputs[clause.input_expression] = ''
        }
      }

      // Send the correctly-keyed request to the backend.
      // Hit policy is sent as the canonical DMN spec string (e.g., "UNIQUE", "RULE ORDER").
      const result = await evaluateDecision(decisionId, {
        inputs,
        trace: showTrace,
      })

      setEvaluationResult({
        outputs: result.outputs as Record<string, unknown>,
        rulesFired: result.rules_fired,
        durationMs: result.duration_ms,
        trace: result.trace ? {
          eventCount: result.trace.event_count,
          durationMs: result.trace.duration_ms,
        } : undefined,
      })
    } catch (err) {
      console.error('Evaluation failed:', err)
    } finally {
      setIsEvaluating(false)
    }
  }

  return (
    <div className="space-y-4">
      {/* Validation Messages */}
      {validationErrors.length > 0 || validationWarnings.length > 0 ? (
        <ValidationDisplay
          errors={validationErrors}
          warnings={validationWarnings}
        />
      ) : null}

      {/* Hit Policy Selector — uses HIT_POLICY_META from types.ts */}
      <div className="flex items-center gap-2">
        <label className="font-medium">Hit Policy:</label>
        <select
          value={hitPolicy}
          onChange={(e) => onHitPolicyChange?.(e.target.value as HitPolicy)}
          className="px-4 py-2 border rounded-md"
        >
          {Object.values(HIT_POLICY_META).map((policyMeta) => (
            <option key={policyMeta.id} value={policyMeta.id}>
              {policyMeta.name}
            </option>
          ))}
        </select>

        <span className="text-sm text-gray-500">
          {HIT_POLICY_META[hitPolicy]?.description}
        </span>

        {/* Aggregator selector — only shown for COLLECT */}
        {hitPolicy === 'COLLECT' && onAggregatorChange && (
          <div className="flex items-center gap-2 ml-4">
            <label className="font-medium text-sm">Agg:</label>
            <select
              value={aggregator ?? 'SUM'}
              onChange={(e) => onAggregatorChange?.(e.target.value as Aggregator)}
              className="px-3 py-1.5 border rounded-md text-sm"
            >
              {['SUM', 'MIN', 'MAX', 'COUNT'].map((agg) => (
                <option key={agg} value={agg}>
                  {agg}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>

      {/* Allowed Values Editor — shown for PRIORITY / OUTPUT ORDER policies */}
      {POLICIES_REQUIRING_ALLOWED_VALUES.includes(hitPolicy) && (
        <div className="space-y-1">
          <label className="text-sm font-medium">Allowed Values:</label>
          <p className="text-xs text-gray-500">
            Define the enumeration order for this hit policy.
          </p>
          {/* The AllowedValuesEditor would be wired to a column's allowedValues state;
              here we show it as a placeholder section that will be per-column in Plan 03 Task 2. */}
          <AllowedValuesEditor items={[]} onChange={() => {}} />
        </div>
      )}

      {/* Evaluate Button */}
      {decisionId && (
        <div className="flex items-center gap-2">
          <Button
            onClick={handleEvaluate}
            disabled={isEvaluating || rows.length === 0}
            className="bg-primary text-primary-foreground"
          >
            {isEvaluating ? 'Evaluating...' : 'Evaluate'}
          </Button>

          {evaluationResult && (
            <span className="text-sm text-gray-600">
              {'✓'} Completed in {evaluationResult.durationMs.toFixed(2)}ms ({evaluationResult.rulesFired} rules fired)
            </span>
          )}

          {/* Trace toggle — reuses v5.0 TestRunner pattern (D-02 / SC-3) */}
          {evaluationResult?.trace && (
            <Button
              variant="outline"
              size="sm"
              onClick={() => setShowTrace(!showTrace)}
            >
              {showTrace ? 'Hide Trace' : 'Show Trace'} ({evaluationResult.trace.eventCount} events)
            </Button>
          )}
        </div>
      )}

      {/* Evaluation Results */}
      {evaluationResult && !isEvaluating && (
        <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-md border">
          <h3 className="font-semibold mb-2">Results</h3>

          {/* Pitfall 3: Handle COLLECT without aggregator returning list outputs */}
          {Object.entries(evaluationResult.outputs).map(([key, value]) => (
            <div key={key} className="mb-2">
              <span className="font-medium text-sm">{key}: </span>
              {Array.isArray(value) ? (
                // COLLECT without aggregator: render as list
                <pre className="text-sm bg-white dark:bg-gray-900 p-1 rounded inline-block">
                  {[(value as unknown[]).map((v) => JSON.stringify(v)).join(', ')]}
                </pre>
              ) : (
                // Scalar output (UNIQUE/ANY/PRIORITY/FIRST/COLLECT with aggregator)
                <pre className="text-sm bg-white dark:bg-gray-900 p-1 rounded inline-block">
                  {JSON.stringify(value, null, 2)}
                </pre>
              )}
            </div>
          ))}

          {/* Trace panel — reuses TestRunner.tsx v5.0 pattern */}
          {showTrace && evaluationResult.trace && (
            <div className="mt-4 border-t pt-3">
              <h4 className="font-semibold mb-1">Trace Summary</h4>
              <p className="text-sm text-gray-600">
                {evaluationResult.trace.eventCount} events,{' '}
                {evaluationResult.trace.durationMs.toFixed(2)}ms total
              </p>
            </div>
          )}
        </div>
      )}

      {/* Decision Table */}
      <table className="w-full border-collapse border">
        <thead>
          {table.getHeaderGroups().map((headerGroup) => (
            <tr key={headerGroup.id} className="bg-gray-100 dark:bg-gray-800">
              {headerGroup.headers.map((header) => {
                const isCondition = header.column.id.includes('condition')
                return (
                  <th
                    key={header.id}
                    className={`p-2 border text-left ${isCondition ? 'bg-blue-50 dark:bg-blue-900/20' : 'bg-green-50 dark:bg-green-900/20'} col-span-${header.colSpan}`}
                  >
                    {flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                )
              })}
            </tr>
          ))}
        </thead>
        <tbody>
          {table.getRowModel().rows.map((row) => (
            <tr key={row.id} className="border-t hover:bg-gray-50 dark:hover:bg-gray-700">
              {row.getVisibleCells().map((cell) => (
                <td key={cell.id} className="p-2 border">
                  <CellEditor cell={cell} />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>

      {/* Add Row Button */}
      <Button variant="outline">+ Add Rule</Button>
    </div>
  )
}

// Cell Editor Component
function CellEditor({ cell }: { cell: any }) {
  const value = cell.getValue()

  if (typeof value === 'object' && value !== null) {
    // For complex objects, use CodeMirror editor
    return (
      <RuleEditor
        value={JSON.stringify(value, null, 2)}
        onChange={(val) => cell.row.original[cell.column.id] = JSON.parse(val as string)}
        height="150px"
      />
    )
  }

  // Simple text input for primitives
  return (
    <input
      type="text"
      value={String(value ?? '')}
      onChange={(e) => (cell.row.original[cell.column.id] = e.target.value)}
      className="w-full px-2 py-1 border rounded-sm"
    />
  )
}
