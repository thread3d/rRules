// The viewer fetches `GET /v1/audit` itself (features/governance/audit), so this route
// only mounts it.
import { createFileRoute } from '@tanstack/react-router'
import { AuditLogViewer } from '@/features/governance/audit'
export const Route = createFileRoute('/governance/audit')({
  component: AuditLogViewer,
})