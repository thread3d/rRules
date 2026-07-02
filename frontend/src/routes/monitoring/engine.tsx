import { createFileRoute } from '@tanstack/react-router'
import { EngineMetricsDashboard } from '@/features/monitoring/dashboards'

export const Route = createFileRoute('/monitoring/engine')({
  component: EngineMetricsDashboard,
})