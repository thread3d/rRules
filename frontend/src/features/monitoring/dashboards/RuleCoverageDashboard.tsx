import { useState, useEffect } from 'react'

interface RuleCoverageData {
  ruleId: string
  ruleName: string
  category: string
  evaluatedCount: number
  firedCount: number
  coveragePercent: number
  lastFiredAt?: string
}

// Shape returned by the backend coverage_endpoint (CoverageTracking::to_json in
// engine/src/monitoring/coverage.rs): { rules: [{rule_id, evaluated, fired,
// evaluation_count, fire_count, skipped_reason}], summary: {...} }. The
// frontend expects RuleCoverageData[] (Pitfall 4 contract mismatch).
interface BackendCoverageRule {
  rule_id?: string
  evaluated?: boolean
  fired?: boolean
  evaluation_count?: number
  fire_count?: number
}
interface BackendCoveragePayload {
  rules?: BackendCoverageRule[]
  summary?: {
    total_rules?: number
    evaluated_count?: number
    fired_count?: number
    coverage_percent?: number
  }
}

// Contract adapter (Pitfall 4): map the backend {rules[],summary} object to
// the frontend RuleCoverageData[]. coveragePercent per rule = evaluated>0 ?
// (fired/evaluated)*100 : 0. Data is empty while execute_single is a stub
// (D-22-03); the adapter handles missing/empty rules gracefully (yields []).
function adaptCoverage(payload: unknown): RuleCoverageData[] {
  if (!payload || typeof payload !== 'object') return []
  const rules = (payload as BackendCoveragePayload).rules
  if (!Array.isArray(rules)) return []
  return rules.map((r) => {
    const ruleId = r.rule_id ?? 'unknown'
    const evaluatedCount = r.evaluation_count ?? (r.evaluated ? 1 : 0)
    const firedCount = r.fire_count ?? (r.fired ? 1 : 0)
    const coveragePercent =
      evaluatedCount > 0 ? (firedCount / evaluatedCount) * 100 : 0
    return {
      ruleId,
      ruleName: ruleId,
      category: 'unknown',
      evaluatedCount,
      firedCount,
      coveragePercent,
    }
  })
}

export function RuleCoverageDashboard() {
  const [data, setData] = useState<RuleCoverageData[]>([])
  const [filterCategory, setFilterCategory] = useState<string>('all')
  const [showOnlyDead, setShowOnlyDead] = useState(false)

  useEffect(() => {
    fetchCoverage()
  }, [])

  async function fetchCoverage() {
    try {
      // /v1/metrics/coverage (route moved into /v1 by 22-01 Task 3).
      const apiUrl = import.meta.env.VITE_API_URL || '/v1'
      const response = await fetch(`${apiUrl}/metrics/coverage`)
      if (response.ok) {
        const payload: unknown = await response.json()
        setData(adaptCoverage(payload))
      }
    } catch (error) {
      console.error('Failed to fetch coverage:', error)
    }
  }

  const categories = Array.from(new Set(data.map((r) => r.category))).sort()

  const filteredData = data.filter((rule) => {
    if (filterCategory !== 'all' && rule.category !== filterCategory) return false
    if (showOnlyDead && rule.coveragePercent > 0) return false
    return true
  })

  // Sort by coverage (lowest first)
  void [...filteredData].sort((a, b) => a.coveragePercent - b.coveragePercent)

  const totalRules = data.length
  const deadRules = data.filter((r) => r.coveragePercent === 0).length
  const averageCoverage =
    data.reduce((sum, r) => sum + r.coveragePercent, 0) /
      (data.length || 1) || 0

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-2xl font-bold">Rule Coverage Dashboard</h2>
        <p className="text-gray-500">
          Track which rules are being evaluated and fired in production
        </p>
      </div>

      {/* Summary Stats */}
      <div className="grid grid-cols-3 gap-4">
        <div className="border rounded-lg p-4 bg-white dark:bg-gray-800">
          <div className="text-sm text-gray-500">Total Rules</div>
          <div className="text-2xl font-bold">{totalRules}</div>
        </div>
        <div className={`border rounded-lg p-4 ${deadRules > 0 ? 'bg-red-50 dark:bg-red-900/10 border-red-200' : 'bg-white dark:bg-gray-800'}`}>
          <div className="text-sm text-gray-500">Dead Rules</div>
          <div className={`text-2xl font-bold ${deadRules > 0 ? 'text-red-600' : ''}`}>{deadRules}</div>
        </div>
        <div className="border rounded-lg p-4 bg-white dark:bg-gray-800">
          <div className="text-sm text-gray-500">Average Coverage</div>
          <div className="text-2xl font-bold">{averageCoverage.toFixed(1)}%</div>
        </div>
      </div>

      {/* Filters */}
      <div className="flex flex-col md:flex-row gap-4">
        <select
          value={filterCategory}
          onChange={(e) => setFilterCategory(e.target.value)}
          className="px-4 py-2 border rounded-md"
        >
          <option value="all">All Categories</option>
          {categories.map((cat) => (
            <option key={cat} value={cat}>
              {cat}
            </option>
          ))}
        </select>

        <button
          onClick={() => setShowOnlyDead(!showOnlyDead)}
          className={`px-4 py-2 rounded-md ${
            showOnlyDead ? 'bg-red-600 text-white' : 'border hover:bg-gray-100 dark:hover:bg-gray-800'
          }`}
        >
          {showOnlyDead ? 'Show All Rules' : 'Show Dead Rules Only'}
        </button>

        <button
          onClick={fetchCoverage}
          className="px-4 py-2 border rounded-md hover:bg-gray-100 dark:hover:bg-gray-800"
        >
          Refresh
        </button>
      </div>

      {/* Coverage Table */}
      <table className="w-full border-collapse border">
        <thead>
          <tr className="bg-gray-100 dark:bg-gray-800">
            <th className="p-2 text-left">Rule</th>
            <th className="p-2 text-left hidden md:table-cell">Category</th>
            <th className="p-2 text-center">Evaluated</th>
            <th className="p-2 text-center">Fired</th>
            <th className="p-2 text-right">Coverage</th>
          </tr>
        </thead>
        <tbody>
          {filteredData.length === 0 ? (
            <tr>
              <td colSpan={5} className="p-8 text-center">
                No rules found
              </td>
            </tr>
          ) : (
            filteredData.map((rule) => {
              const isDead = rule.coveragePercent === 0
              return (
                <tr key={rule.ruleId} className={`border-t ${isDead ? 'bg-red-50/30 dark:bg-red-900/5' : ''}`}>
                  <td className="p-2 font-medium">{rule.ruleName}</td>
                  <td className="p-2 text-sm text-gray-500 hidden md:table-cell">
                    {rule.category}
                  </td>
                  <td className="p-2 text-center">{rule.evaluatedCount}</td>
                  <td className={`p-2 text-center ${isDead ? 'text-red-600' : ''}`}>
                    {rule.firedCount}
                  </td>
                  <td className="p-2 text-right">
                    <div className="flex items-center gap-2">
                      <span
                        className={`px-2 py-1 rounded text-sm ${
                          rule.coveragePercent === 0
                            ? 'bg-red-100 text-red-700'
                            : rule.coveragePercent >= 50
                            ? 'bg-green-100 text-green-700'
                            : 'bg-yellow-100 text-yellow-700'
                        }`}
                      >
                        {rule.coveragePercent.toFixed(1)}%
                      </span>
                    </div>
                  </td>
                </tr>
              )
            })
          )}
        </tbody>
      </table>
    </div>
  )
}
