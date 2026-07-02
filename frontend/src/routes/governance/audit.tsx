import { createFileRoute } from '@tanstack/react-router'
import { AuditLogViewer } from '@/features/governance/audit'

// TODO(22-05): wire AuditLogViewer fetch to /v1/audit/* (mount-only here).
export const Route = createFileRoute('/governance/audit')({
  component: AuditLogViewer,
})