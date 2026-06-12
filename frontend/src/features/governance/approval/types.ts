// Approval workflow types for Phase 18 Enterprise Governance

export type ApprovalState = 'draft' | 'submitted' | 'in_review' | 'approved' | 'rejected' | 'escalated'

export interface ApprovalRequest {
  id: string
  rule_id: string
  workflow_id: string
  maker_id: string
  state: string
  reviewers?: string[]
  required_approvals?: number
  approved_by?: string[]
  created_at: string
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

export interface ApprovalStateDetails {
  status: string
  timestamp: string
}

export interface Approver {
  userId: string
  name: string
  hasApproved?: boolean
}

export interface TierConfig {
  reviewers: string[]
  requiredApprovals: number
}

export interface ApprovalWorkflow {
  id: number
  name: string
  description?: string
  tiers: TierConfig[]
  autoEscapeHours: number
}

export type ApprovalWebhookEvent = 'submitted' | 'approved' | 'rejected'
