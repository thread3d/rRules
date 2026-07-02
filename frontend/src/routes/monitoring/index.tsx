import { createFileRoute } from '@tanstack/react-router'
import { EngineMetricsDashboard } from '@/features/monitoring/dashboards'

function MonitoringIndexPage() {
  return (
    <div className="p-6 space-y-6">
      <div>
        <h1 className="text-2xl font-bold">Monitoring</h1>
        <p className="text-gray-500">Engine metrics, rule coverage, and dead-rule reports.</p>
      </div>
      <EngineMetricsDashboard />
    </div>
  )
}

export const Route = createFileRoute('/monitoring/')({
  component: MonitoringIndexPage,
})