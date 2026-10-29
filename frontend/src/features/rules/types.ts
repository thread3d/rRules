// Types for the Rules feature

import type { Rule } from '@/types/common'
import type { ValidationError, ValidationWarning } from '@/components/validation/ValidationDisplay'

export interface RulesListProps {
  rules: Rule[]
  isLoading?: boolean
  onRefresh?: () => void
  /** Total rules on the server, which is not `rules.length` — that is one page. */
  totalCount?: number
  page?: number
  totalPages?: number
  goToPage?: (page: number) => void
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

// ---- DMN 1.5 Canonical Hit-Policy Model (per DMN spec Clause 8.2.12) ----
// Aligns frontend vocabulary with engine/src/dmn/model.rs HitPolicy enum.

/** String union of exactly the 7 canonical DMN 1.5 hit-policy values. */
export type HitPolicy = 'UNIQUE' | 'ANY' | 'PRIORITY' | 'FIRST' | 'RULE ORDER' | 'OUTPUT ORDER' | 'COLLECT'

/** String union for COLLECT aggregators (only meaningful when HitPolicy === 'COLLECT'). */
export type Aggregator = 'SUM' | 'MIN' | 'MAX' | 'COUNT'

/** Which hit policies support aggregation. Per DMN spec, only COLLECT does. */
export const AGGREGATOR_POLICIES: HitPolicy[] = ['COLLECT']

/** Default aggregator for new COLLECT tables. */
export const DEFAULT_AGGREGATOR: Aggregator = 'SUM'

/** Aggregator display options. */
export const AGGREGATORS: { id: Aggregator; name: string }[] = [
  { id: 'SUM', name: 'Sum' },
  { id: 'MIN', name: 'Min' },
  { id: 'MAX', name: 'Max' },
  { id: 'COUNT', name: 'Count' },
]

/** Metadata shape for each hit policy. */
export interface HitPolicyMeta {
  id: HitPolicy
  name: string
  description: string
  requiresAggregator: boolean
}

/** Full metadata record mapping each canonical HitPolicy to its spec-accurate description (DMN 1.5 Clause 8.2.12). */
export const HIT_POLICY_META: Record<HitPolicy, HitPolicyMeta> = {
  UNIQUE: {
    id: 'UNIQUE',
    name: 'Unique',
    description: "Only one rule can match. Multiple matches is a violation.",
    requiresAggregator: false,
  },
  ANY: {
    id: 'ANY',
    name: 'Any',
    description: "Multiple rules can match; all must agree on outputs.",
    requiresAggregator: false,
  },
  PRIORITY: {
    id: 'PRIORITY',
    name: 'Priority',
    description: "Select the matching rule whose output has highest allowedValues position.",
    requiresAggregator: false,
  },
  FIRST: {
    id: 'FIRST',
    name: 'First',
    description: "First matching rule in document order wins. No conflict detection.",
    requiresAggregator: false,
  },
  'RULE ORDER': {
    id: 'RULE ORDER',
    name: 'Rule Order',
    description: "All matching rules' outputs returned in document order.",
    requiresAggregator: false,
  },
  'OUTPUT ORDER': {
    id: 'OUTPUT ORDER',
    name: 'Output Order',
    description: "All matching rules' outputs sorted by allowedValues enumeration.",
    requiresAggregator: false,
  },
  COLLECT: {
    id: 'COLLECT',
    name: 'Collect',
    description: "Collect all matching rule outputs; optionally aggregate (SUM/MIN/MAX/COUNT).",
    requiresAggregator: true,
  },
}

/** Array form for dropdown rendering — derived from HIT_POLICY_META keys. */
export const HIT_POLICIES: HitPolicy[] = Object.keys(HIT_POLICY_META) as HitPolicy[]

// ---- Allowed values (for PRIORITY / OUTPUT ORDER hit policies). ----

/** Item in an enumeration of allowed values on an output column. */
export interface AllowedValueItem {
  id: string // unique within this editor instance
  text: string // display value (e.g., "Low", "Medium", "High")
}

/** Hit policies that require output column allowedValues. */
export const POLICIES_REQUIRING_ALLOWED_VALUES: HitPolicy[] = ['PRIORITY', 'OUTPUT ORDER']

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
