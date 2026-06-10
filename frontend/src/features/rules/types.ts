// Types for the Rules feature

import type { Rule } from '@/types/common'

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
