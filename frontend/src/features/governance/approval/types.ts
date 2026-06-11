// Approval workflow types for Phase 18 Enterprise Governance

export type ApprovalState = 'draft' | 'submitted' | 'in_review' | 'approved' | 'rejected' | 'escalated'

export interface ApprovalRequest {
  id: string
  workflowId: number
  ruleId: string
  makerId: string
  makerName?: string
  state: ApprovalState
  currentState: ApprovalStateDetails
  currentTier: number
  reviewers: Approver[]
  requiredApprovals: number
  approvedBy: string[]
  submittedAt?: string
  startedAt?: string
  completedAt?: string
  rejectionReason?: string
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
