import { createFileRoute } from '@tanstack/react-router'
import { DeadRuleReportDashboard } from '@/features/monitoring/dashboards'

export const Route = createFileRoute('/monitoring/dead-rules')({
  component: DeadRuleReportDashboard,
})