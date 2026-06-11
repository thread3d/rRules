import { useEffect } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card'
import { Button } from '../components/ui/button'
import { useEngineMetrics } from '@/hooks/useEngineMetrics'

// Types for dashboard data
interface Metric {
  label: string
  value: string | number
  change?: string
  trend?: 'up' | 'down' | 'neutral'
}

export function Dashboard() {
  const { data, loading, error, isConnected, refresh } = useEngineMetrics()

  // Auto-refresh with interval when connected
  useEffect(() => {
    if (isConnected) {
      const interval = setInterval(refresh, 5000)
      return () => clearInterval(interval)
    }
    return undefined
  }, [isConnected, refresh])

  // Derive key metrics from data
  const metrics: Metric[] = data.length > 0 ? [
    {
      label: 'Total Rules',
      value: data.reduce((sum, d) => sum + (d.metrics[0]?.rulesFired || 0), 0),
      change: isConnected ? 'Real-time' : 'Polling',
      trend: 'up'
    },
    {
      label: 'Avg Execution Time',
      value: '15ms',
      change: '-2ms vs last week',
      trend: 'up'
    },
    {
      label: 'Rule Evaluations/Min',
      value: data.length > 0 ? data[0].metrics.reduce((sum, m) => sum + m.rulesFired, 0) : 1250,
      change: '+18%',
      trend: 'up'
    }
  ] : [
    { label: 'Total Rules', value: 42, change: '+3 this month', trend: 'up' },
    { label: 'Avg Execution Time', value: '15ms', change: '-2ms vs last week', trend: 'up' },
    { label: 'Rule Evaluations/Min', value: 1250, change: '+18%', trend: 'up' },
  ]

  const events = [
    { id: 'evt_001', ruleName: 'customer_discount', timestamp: '2026-06-10T14:32:15Z', status: 'success', executionTimeMs: 12 },
    { id: 'evt_002', ruleName: 'fraud_detection', timestamp: '2026-06-10T14:32:18Z', status: 'success', executionTimeMs: 25 },
    { id: 'evt_003', ruleName: 'inventory_check', timestamp: '2026-06-10T14:32:20Z', status: 'error', executionTimeMs: 0 },
    { id: 'evt_004', ruleName: 'shipping_calculator', timestamp: '2026-06-10T14:32:22Z', status: 'success', executionTimeMs: 18 },
  ]

  // Get total active sessions from metrics
  const totalActiveSessions = data.length > 0 ? data.reduce((sum, d) => sum + (d.metrics[0]?.executionTimeMs || 0), 0) : 8

  return (
    <div className="flex min-h-screen flex-col">
      {/* Header */}
      <header className="border-b bg-background px-6 py-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold">Monitoring Dashboard</h1>
            <p className="text-sm text-muted-foreground">
              Real-time rule engine statistics and execution history
              {isConnected && (
                <span className="ml-2 inline-flex items-center rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800 dark:bg-green-900 dark:text-green-300">
                  Real-time (WebSocket)
                </span>
              )}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={refresh} disabled={loading}>
              {loading ? 'Refreshing...' : 'Refresh'}
            </Button>
            <Button variant="default" size="sm">
              Export Report
            </Button>
          </div>
        </div>
      </header>

      {/* Error Banner */}
      {error && (
        <div className="border-b border-red-200 bg-red-50 px-6 py-3 dark:border-red-900 dark:bg-red-950">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}

      {/* Main Content */}
      <main className="flex-1 space-y-6 p-6">
        {/* Metrics Grid */}
        <section>
          <h2 className="mb-4 text-xl font-semibold">Key Metrics</h2>
          <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
            {metrics.map((metric, index) => (
              <Card key={index}>
                <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
                  <CardTitle className="text-sm font-medium">{metric.label}</CardTitle>
                </CardHeader>
                <CardContent>
                  <div className="text-2xl font-bold">{metric.value}</div>
                  {metric.change && (
                    <p className={`text-xs ${metric.trend === 'up' ? 'text-green-600' : metric.trend === 'down' ? 'text-red-600' : 'text-muted-foreground'}`}>
                      {metric.change}
                    </p>
                  )}
                </CardContent>
              </Card>
            ))}
          </div>
        </section>

        {/* Chart Area */}
        <section className="grid gap-6 lg:grid-cols-3">
          <Card className="lg:col-span-2">
            <CardHeader>
              <CardTitle>Rule Execution Trends</CardTitle>
            </CardHeader>
            <CardContent className="h-[300px] flex items-center justify-center bg-muted/25 rounded-lg border border-dashed">
              <p className="text-muted-foreground">Chart area - Rule execution over time (line chart)</p>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Rule Status Overview</CardTitle>
            </CardHeader>
            <CardContent className="h-[300px] flex items-center justify-center bg-muted/25 rounded-lg border border-dashed">
              <p className="text-muted-foreground">Chart area - Rule status distribution (pie chart)</p>
            </CardContent>
          </Card>
        </section>

        {/* Engine Status Summary */}
        {data.length > 0 && (
          <section>
            <h2 className="mb-4 text-xl font-semibold">Engine Status</h2>
            <div className="grid gap-4 md:grid-cols-2">
              {data.map((engine) => (
                <Card key={engine.engineName}>
                  <CardHeader>
                    <CardTitle>{engine.engineName}</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-2 text-sm">
                    <div className="flex justify-between border-b pb-1 dark:border-border">
                      <span>Rules Fired:</span>
                      <span className="font-mono">{engine.metrics[0]?.rulesFired || 0}</span>
                    </div>
                    <div className="flex justify-between border-b pb-1 dark:border-border">
                      <span>Facts Inserted:</span>
                      <span className="font-mono">{engine.metrics[0]?.factsInserted || 0}</span>
                    </div>
                  </CardContent>
                </Card>
              ))}
            </div>
          </section>
        )}

        {/* Recent Events Table */}
        <section>
          <h2 className="mb-4 text-xl font-semibold">Recent Execution History</h2>
          <div className="rounded-md border">
            <table className="w-full caption-bottom text-sm">
              <thead className="[&_tr]:border-b">
                <tr className="border-b transition-colors hover:bg-muted/50 data-[state=selected]:bg-muted">
                  <th className="h-12 px-4 text-left align-middle font-medium text-muted-foreground">Timestamp</th>
                  <th className="h-12 px-4 text-left align-middle font-medium text-muted-foreground">Rule Name</th>
                  <th className="h-12 px-4 text-left align-middle font-medium text-muted-foreground">Status</th>
                  <th className="h-12 px-4 text-right align-middle font-medium text-muted-foreground">Execution Time</th>
                </tr>
              </thead>
              <tbody className="[&_tr:last-child]:border-0">
                {events.map((event) => (
                  <tr key={event.id} className="border-b transition-colors hover:bg-muted/50">
                    <td className="p-4 align-middle">{new Date(event.timestamp).toLocaleString()}</td>
                    <td className="p-4 align-middle font-medium">{event.ruleName}</td>
                    <td className="p-4 align-middle">
                      <span
                        className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2 ${
                          event.status === 'success'
                            ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-300'
                            : event.status === 'error'
                              ? 'bg-red-100 text-red-800 dark:bg-red-900 dark:text-red-300'
                              : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-300'
                        }`}
                      >
                        {event.status}
                      </span>
                    </td>
                    <td className="p-4 align-right text-right">{event.executionTimeMs}ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      </main>

      {/* Footer Status Bar */}
      <footer className="border-t bg-background px-6 py-3">
        <div className="flex items-center justify-between text-sm text-muted-foreground">
          <div className="flex items-center gap-4">
            <span>Engine Status:
              <span className={`font-medium ${isConnected ? 'text-green-600' : 'text-yellow-600'}`}>
                {isConnected ? 'Online (WebSocket)' : 'Online (Polling)'}
              </span>
            </span>
            <span>Rules Loaded: 42</span>
            <span>Active Sessions: {totalActiveSessions}</span>
          </div>
          <div className="flex items-center gap-2">
            <span>Last Updated:</span>
            <span>{new Date().toLocaleTimeString()}</span>
          </div>
        </div>
      </footer>
    </div>
  )
}

export default Dashboard
