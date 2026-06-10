import type { Rule, HitPolicyId, Condition, Action, PaginationMeta } from '@/types/common'

// API Request/Response types

export interface CreateRuleRequest {
  name: string
  description?: string
  hitPolicy: HitPolicyId
  conditions: Omit<Condition, 'id'>[]
  actions: Omit<Action, 'id'>[]
}

export interface UpdateRuleRequest extends Partial<CreateRuleRequest> {}

export interface RulesResponse {
  success: boolean
  data: Rule[]
}

export interface RuleResponse {
  success: boolean
  data: Rule
}

export interface PaginatedRulesResponse {
  success: boolean
  data: Rule[]
  meta: PaginationMeta
}

export interface FilterRulesRequest {
  page?: number
  perPage?: number
  category?: string
  owner?: string
  status?: string
  search?: string
  sortBy?: string
  sortOrder?: 'asc' | 'desc'
}

// Auth API types

export interface LoginRequest {
  email: string
  password: string
}

export interface LoginResponse {
  success: boolean
  token: string
  user: {
    id: string
    email: string
    displayName: string
    role: string
  }
}

export interface RegisterRequest {
  email: string
  password: string
  displayName: string
}

export interface RegisterResponse extends LoginResponse {}
