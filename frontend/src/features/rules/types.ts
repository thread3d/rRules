// Types for the Rules feature

import type { Rule } from '@/types/common'
import type { ValidationError, ValidationWarning } from '@/components/validation/ValidationDisplay'

export interface RulesListProps {
  rules: Rule[]
  isLoading?: boolean
  onRefresh?: () => void
}

export interface RulesStats {
  total: number
  active: number
  draft: number
  zeroHit: number
  conflicts: number
}

export interface FilterState {
  category: string | 'all'
  status: string | 'all'
  sortBy: string
  sortOrder: 'asc' | 'desc'
  search?: string
}

// Decision table types

export interface HitPolicy {
  id: string
  name: string
  description: string
  requiresAggregation?: boolean
}

export const HIT_POLICIES: HitPolicy[] = [
  { id: 'unique', name: 'Unique', description: 'Only one rule can match' },
  { id: 'any', name: 'Any', description: 'Multiple rules can match, order doesn\'t matter' },
  { id: 'priority', name: 'Priority', description: 'Rules are ordered by priority' },
  { id: 'collect', name: 'Collect', description: 'All matching rules are fired', requiresAggregation: true },
  { id: 'rule-order', name: 'Rule Order', description: 'First matching rule fires' },
  { id: 'sum', name: 'Sum', description: 'Aggregates values from matching rules', requiresAggregation: true },
  { id: 'min', name: 'Min', description: 'Selects minimum value from matching rules', requiresAggregation: true },
  { id: 'max', name: 'Max', description: 'Selects maximum value from matching rules', requiresAggregation: true },
  { id: 'count', name: 'Count', description: 'Counts matching rules', requiresAggregation: true },
]

export type HitPolicyId = (typeof HIT_POLICIES)[number]['id']

export interface DecisionTableColumn {
  id: string
  label: string
  accessorKey?: string
}

export interface DecisionTableRow {
  id: string
  conditions: Record<string, unknown>
  actions: Record<string, unknown>
}

// Validation types for decision table editor

export interface CellValidationState {
  rowId: string
  columnId: string
  errors: ValidationError[]
  warnings: ValidationWarning[]
}
