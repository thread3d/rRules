import { createFileRoute } from '@tanstack/react-router'
import { ApprovalQueue } from '@/features/governance/approval'

// TODO(22-05): wire ApprovalQueue fetch to /v1/approvals/* (mount-only here).
export const Route = createFileRoute('/governance/approvals')({
  component: ApprovalQueue,
})