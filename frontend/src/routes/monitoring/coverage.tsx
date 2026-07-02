import { createFileRoute } from '@tanstack/react-router'
import { RuleCoverageDashboard } from '@/features/monitoring/dashboards'

export const Route = createFileRoute('/monitoring/coverage')({
  component: RuleCoverageDashboard,
})