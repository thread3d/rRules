// The queue fetches `GET /v1/approvals/pending` itself (features/governance/approval),
// so this route only mounts it.
import { createFileRoute } from '@tanstack/react-router'
import { ApprovalQueue } from '@/features/governance/approval'
export const Route = createFileRoute('/governance/approvals')({
  component: ApprovalQueue,
})