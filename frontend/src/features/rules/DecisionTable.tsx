import { useMemo } from 'react'
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
} from '@tanstack/react-table'

import type { HitPolicyId, DecisionTableColumn, DecisionTableRow } from './types'
import { RuleEditor } from '@/components/RuleEditor'
import { Button } from '@/components/ui/button'

export interface DecisionTableProps {
  hitPolicy: HitPolicyId
  columns: DecisionTableColumn[]
  rows: DecisionTableRow[]
  onHitPolicyChange?: (policy: HitPolicyId) => void
}

const HIT_POLICY_OPTIONS = [
  { id: 'unique', label: 'Unique' },
  { id: 'any', label: 'Any' },
  { id: 'priority', label: 'Priority' },
  { id: 'collect', label: 'Collect' },
  { id: 'rule-order', label: 'Rule Order' },
  { id: 'sum', label: 'Sum' },
  { id: 'min', label: 'Min' },
  { id: 'max', label: 'Max' },
  { id: 'count', label: 'Count' },
]

export function DecisionTable({
  hitPolicy,
  columns,
  rows,
  onHitPolicyChange,
}: DecisionTableProps) {
  const table = useReactTable({
    data: rows,
    columns: useMemo(() => columns, [columns]),
    getCoreRowModel: getCoreRowModel(),
  })

  return (
    <div className="space-y-4">
      {/* Hit Policy Selector */}
      <div className="flex items-center gap-2">
        <label className="font-medium">Hit Policy:</label>
        <select
          value={hitPolicy}
          onChange={(e) => onHitPolicyChange?.(e.target.value as HitPolicyId)}
          className="px-4 py-2 border rounded-md"
        >
          {HIT_POLICY_OPTIONS.map((policy) => (
            <option key={policy.id} value={policy.id}>
              {policy.label}
            </option>
          ))}
        </select>

        <span className="text-sm text-gray-500">
          {
            HIT_POLICY_OPTIONS.find((p) => p.id === hitPolicy)?.label
          }
        </span>
      </div>

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
        onChange={(val) => cell.row.original[cell.column.id] = JSON.parse(val)}
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
