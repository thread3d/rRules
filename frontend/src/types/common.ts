// Common types for rRules BRMS

export interface HitPolicy {
  id: string
  name: string
  description: string
}

export const HIT_POLICIES: HitPolicy[] = [
  { id: 'unique', name: 'Unique', description: 'Only one rule can match' },
  { id: 'any', name: 'Any', description: 'Multiple rules can match, order doesn\'t matter' },
  { id: 'priority', name: 'Priority', description: 'Rules are ordered by priority' },
  { id: 'collect', name: 'Collect', description: 'All matching rules are fired' },
  { id: 'rule-order', name: 'Rule Order', description: 'First matching rule fires' },
  { id: 'sum', name: 'Sum', description: 'Aggregates values from matching rules' },
  { id: 'min', name: 'Min', description: 'Selects minimum value from matching rules' },
  { id: 'max', name: 'Max', description: 'Selects maximum value from matching rules' },
  { id: 'count', name: 'Count', description: 'Counts matching rules' },
]

export type HitPolicyId = (typeof HIT_POLICIES)[number]['id']

export interface Rule {
  id: string
  name: string
  description?: string
  status: RuleStatus
  hitPolicy: HitPolicyId
  conditions: Condition[]
  actions: Action[]
  ownerId: string
  createdAt: string
  updatedAt: string
  version: number
}

export type RuleStatus = 'draft' | 'active' | 'inactive' | 'pending-approval' | 'deprecated'

export interface Condition {
  id: string
  field: string
  operator: OperatorType
  value: unknown
  conjunction?: ConjunctionType
}

export type OperatorType =
  | 'equals'
  | 'not-equals'
  | 'less-than'
  | 'less-than-or-equal'
  | 'greater-than'
  | 'greater-than-or-equal'
  | 'contains'
  | 'starts-with'
  | 'ends-with'
  | 'in'

export type ConjunctionType = 'and' | 'or'

export interface Action {
  id: string
  type: ActionType
  field: string
  value: unknown
}

export type ActionType =
  | 'set-value'
  | 'increment'
  | 'decrement'
  | 'call-function'
  | 'log'

export interface ApiError {
  code: string
  message: string
  details?: Record<string, unknown>
}

export interface PaginatedResponse<T> {
  data: T[]
  meta: PaginationMeta
}

export interface PaginationMeta {
  page: number
  perPage: number
  total: number
  totalPages: number
}
