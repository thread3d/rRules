import { useEffect } from 'react'
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from 'recharts'
import { useEngineMetrics } from '@/hooks/useEngineMetrics'

export function EngineMetricsDashboard() {
  const { data, loading, error, isConnected, refresh } = useEngineMetrics()

  // Auto-refresh with interval when connected
  useEffect(() => {
    if (isConnected) {
      const interval = setInterval(refresh, 5000)
      return () => clearInterval(interval)
    }
    return undefined
  }, [isConnected, refresh])

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col md:flex-row justify-between items-center gap-4">
        <h2 className="text-2xl font-bold">Engine Metrics Dashboard</h2>

        <div className="flex items-center gap-4">
          {isConnected && (
            <span className="inline-flex items-center rounded-full bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800 dark:bg-green-900 dark:text-green-300">
              Real-time (WebSocket)
            </span>
          )}

          <label className="text-sm text-gray-500">Refresh:</label>
          <select
            value={5000}
            onChange={() => {}}
            disabled={true}
            className="px-3 py-2 border rounded-md opacity-60"
          >
            <option value={1000}>1 second</option>
            <option value={5000}>5 seconds (auto)</option>
            <option value={15000}>15 seconds</option>
            <option value={60000}>1 minute</option>
          </select>

          <button
            onClick={refresh}
            disabled={loading}
            className="px-3 py-2 border rounded-md hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50"
          >
            {loading ? '...' : 'Refresh'}
          </button>
        </div>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-600 dark:border-red-900 dark:bg-red-950 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Metrics Grid */}
      {loading ? (
        <div className="text-center py-8">Loading metrics...</div>
      ) : data.length === 0 ? (
        <div className="text-center py-8">No metrics available</div>
      ) : (
        <>
          {/* Key Performance Indicators */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            {data.map((engine) => {
              const latest = engine.metrics[engine.metrics.length - 1] || {}
              return (
                <div key={engine.engineName} className="border rounded-lg p-4 bg-white dark:bg-gray-800">
                  <h3 className="font-semibold text-sm text-gray-500">{engine.engineName}</h3>
                  <div className="mt-2 space-y-1">
                    <div className="text-2xl font-bold text-blue-600">
                      {latest.rulesFired || 0} rules/s
                    </div>
                    <div className="text-sm text-gray-500">Rules fired</div>
                  </div>
                </div>
              )
            })}
          </div>

          {/* Charts */}
          {data.map((engine) => (
            <div key={engine.engineName} className="border rounded-lg p-4 bg-white dark:bg-gray-800">
              <h3 className="font-semibold mb-4">{engine.engineName} - Rules Firing Rate</h3>
              <ResponsiveContainer width="100%" height={250}>
                <LineChart data={engine.metrics.slice(-30)}>
                  <CartesianGrid strokeDasharray="3 3" />
                  <XAxis dataKey="timestamp" tick={{ fontSize: 10 }} />
                  <YAxis />
                  <Tooltip
                    contentStyle={{ backgroundColor: '#fff', border: '1px solid #ccc' }}
                  />
                  <Line
                    type="monotone"
                    dataKey="rulesFired"
                    stroke="#3b82f6"
                    strokeWidth={2}
                    name="Rules Fired"
                    dot={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          ))}
        </>
      )}
    </div>
  )
}
