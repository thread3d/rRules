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

// Approval types

export interface ApprovalState {
  Draft: string
  Submitted: { submitted_at: string }
  InReview: { started_at: string, current_tier: number }
  Approved: { approved_by: string[], completed_at: string }
  Rejected: { rejected_by: string, reason?: string, completed_at: string }
  Escalated: { escalated_to_tier: number, escalated_at: string }
}

export interface ApprovalRequest {
  id: string
  rule_id: string
  workflow_id: string
  maker_id: string
  state: string
  reviewers: string[]
  required_approvals: number
  approved_by: string[]
  created_at: string
}

export interface SubmitApprovalRequest {
  workflow_id?: string
  maker_id?: string
}

export interface ApprovalActionRequest {
  approver_id?: string
  reason?: string
}

export interface ApprovalsResponse {
  total_count: number
  approvals: Array<{
    id: string
    rule_id: string
    maker_id: string
    state: string
    created_at: string
  }>
}
