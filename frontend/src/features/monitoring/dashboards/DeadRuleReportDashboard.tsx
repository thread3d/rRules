import { useState, useEffect } from 'react'

interface DeadRuleInfo {
  ruleId: string
  ruleName: string
  category: string
  evaluationCount: number
  fireCount: number
  recommendation: string
}

// Contract adapter (Pitfall 4): the backend dead_rules_endpoint returns
// Vec<String> of rule IDs (api/src/metrics.rs), NOT DeadRuleInfo[]. Map each
// rule ID into the frontend shape. Data is legitimately empty while
// execute_single is a stub (D-22-03); the adapter handles [] gracefully.
function adaptDeadRules(payload: unknown): DeadRuleInfo[] {
  if (!Array.isArray(payload)) return []
  return payload.map((id) => {
    const ruleId = typeof id === 'string' ? id : String(id)
    return {
      ruleId,
      ruleName: ruleId,
      category: 'unknown',
      evaluationCount: 0,
      fireCount: 0,
      recommendation: 'No execution tracking data yet (execution engine is a stub).',
    }
  })
}

export function DeadRuleReportDashboard() {
  const [deadRules, setDeadRules] = useState<DeadRuleInfo[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetchDeadRules()
  }, [])

  async function fetchDeadRules() {
    try {
      // /v1/metrics/dead-rules (route moved into /v1 by 22-01 Task 3).
      const apiUrl = import.meta.env.VITE_API_URL || '/v1'
      const response = await fetch(`${apiUrl}/metrics/dead-rules`)
      if (response.ok) {
        const payload: unknown = await response.json()
        setDeadRules(adaptDeadRules(payload))
      }
    } catch (error) {
      console.error('Failed to fetch dead rules:', error)
    } finally {
      setLoading(false)
    }
  }

  const totalRules = deadRules.length
  const neverEvaluated = deadRules.filter((r) => r.evaluationCount === 0).length

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-2xl font-bold">Dead Rule Report</h2>
        <p className="text-gray-500">
          Rules that have never fired during the analysis period
        </p>
      </div>

      {/* Summary Stats */}
      <div className="grid grid-cols-2 gap-4">
        <div className={`border rounded-lg p-4 ${totalRules > 0 ? 'bg-red-50 dark:bg-red-900/10 border-red-200' : 'bg-white dark:bg-gray-800'}`}>
          <div className="text-sm text-gray-500">Dead Rules</div>
          <div className={`text-2xl font-bold ${totalRules > 0 ? 'text-red-600' : ''}`}>{totalRules}</div>
        </div>
        <div className="border rounded-lg p-4 bg-white dark:bg-gray-800">
          <div className="text-sm text-gray-500">Never Evaluated</div>
          <div className="text-2xl font-bold">{neverEvaluated}</div>
        </div>
      </div>

      {/* Dead Rules Table */}
      <table className="w-full border-collapse border">
        <thead>
          <tr className="bg-gray-100 dark:bg-gray-800">
            <th className="p-2 text-left">Rule</th>
            <th className="p-2 text-left hidden md:table-cell">Category</th>
            <th className="p-2 text-center">Evaluations</th>
            <th className="p-2 text-center">Fires</th>
          </tr>
        </thead>
        <tbody>
          {loading ? (
            <tr>
              <td colSpan={4} className="p-8 text-center">
                Loading...
              </td>
            </tr>
          ) : deadRules.length === 0 ? (
            <tr>
              <td colSpan={4} className="p-8 text-center">
                No dead rules detected
              </td>
            </tr>
          ) : (
            deadRules.map((rule) => (
              <tr key={rule.ruleId} className="border-t hover:bg-gray-50 dark:hover:bg-gray-700">
                <td className="p-2 font-medium">{rule.ruleName}</td>
                <td className="p-2 text-sm text-gray-500 hidden md:table-cell">
                  {rule.category}
                </td>
                <td className="p-2 text-center text-red-600 font-semibold">
                  {rule.evaluationCount}
                </td>
                <td className={`p-2 text-center ${rule.fireCount === 0 ? 'text-red-600' : ''}`}>
                  {rule.fireCount}
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>

      {/* Recommendations */}
      {deadRules.length > 0 && (
        <div className="border rounded-lg p-4 bg-white dark:bg-gray-800">
          <h3 className="font-semibold mb-4">Recommendations</h3>
          <ul className="space-y-2">
            {deadRules.map((rule) => (
              <li key={rule.ruleId} className="flex gap-2 items-start">
                <span className="text-red-500 mt-1">⚠️</span>
                <div>
                  <p className="font-medium">{rule.ruleName}</p>
                  <p className="text-sm text-gray-600 dark:text-gray-400">{rule.recommendation}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}
