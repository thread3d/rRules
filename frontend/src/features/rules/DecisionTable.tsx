import { useMemo, useState } from 'react'
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
} from '@tanstack/react-table'

import type { HitPolicy, Aggregator, DecisionTableColumn, DecisionTableRow } from './types'
import { HIT_POLICY_META } from './types'
import { RuleEditor } from '@/components/RuleEditor'
import { Button } from '@/components/ui/button'
import { ValidationDisplay } from '@/components/validation/ValidationDisplay'
import type { ValidationError, ValidationWarning } from '@/components/validation/ValidationDisplay'
import { evaluateDecision } from '@/api/decisions'

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

  // Evaluate the decision table with current inputs
  async function handleEvaluate() {
    if (!decisionId) return

    setIsEvaluating(true)
    setEvaluationResult(null)
    setShowTrace(false)

    try {
      // Build inputs keyed by column accessorKey / input_expression name.
      // The first row's conditions are used as sample input values; the keys
      // come from iterating input_clauses (populated via dmnSerializer when
      // the backend actually evaluates). For now we use column labels as keys
      // since this is the editor view — real evaluation goes through the
      // serializer in Plan 03.
      const inputs: Record<string, unknown> = {}
      for (const col of columns) {
        if (col.accessorKey && col.label) {
          const sampleRow = rows[0]
          if (sampleRow?.conditions) {
            inputs[col.accessorKey] =
              sampleRow.conditions[col.label] ?? ''
          }
        }
      }

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
      // Could show error in UI here
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
          <pre className="text-sm overflow-auto max-h-64 bg-white dark:bg-gray-900 p-2 rounded">
            {JSON.stringify(evaluationResult.outputs, null, 2)}
          </pre>

          {showTrace && evaluationResult.trace && (
            <div className="mt-4">
              <h4 className="font-semibold mb-1">Trace Summary</h4>
              <p className="text-sm text-gray-600">
                {evaluationResult.trace.eventCount} events, {evaluationResult.trace.durationMs.toFixed(2)}ms total
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
