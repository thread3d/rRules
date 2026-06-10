import { useState, useMemo } from 'react'

// Import types
import type { RulesListProps, RulesStats, FilterState } from './types'
import { usePermission } from '@/hooks/use-permission'

const STATUS_BADGE_COLORS: Record<string, string> = {
  draft: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300',
  active: 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400',
  inactive: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
  'pending-approval': 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400',
  deprecated: 'bg-gray-500 text-white',
}

export function RulesList({ rules, isLoading }: RulesListProps) {
  const [filter, setFilter] = useState<FilterState>({
    category: 'all',
    status: 'all',
    sortBy: 'createdAt',
    sortOrder: 'desc',
  })
  const canCreateRule = usePermission('rule:create')

  // Filter and sort rules
  const filteredRules = useMemo(() => {
    let result = [...rules]

    // Search filter
    if (filter.search) {
      const searchLower = filter.search.toLowerCase()
      result = result.filter(
        (r) =>
          r.name.toLowerCase().includes(searchLower) ||
          (r.description && r.description.toLowerCase().includes(searchLower))
      )
    }

    // Status filter
    if (filter.status !== 'all') {
      result = result.filter((r) => r.status === filter.status)
    }

    // Sorting
    result.sort((a, b) => {
      const sortOrder = filter.sortOrder === 'asc' ? 1 : -1

      switch (filter.sortBy) {
        case 'name':
          return sortOrder * a.name.localeCompare(b.name)
        case 'lastModified':
          return sortOrder * new Date(a.updatedAt).getTime() - new Date(b.updatedAt).getTime()
        default:
          return sortOrder * new Date(a.createdAt).getTime() - new Date(b.createdAt).getTime()
      }
    })

    return result
  }, [rules, filter])

  // Calculate stats
  const stats = useMemo<RulesStats>(() => {
    return {
      total: rules.length,
      active: rules.filter((r) => r.status === 'active').length,
      draft: rules.filter((r) => r.status === 'draft').length,
      zeroHit: 0, // Not implemented yet
      conflicts: 0, // Not implemented yet
    }
  }, [rules])

  return (
    <div className="space-y-4">
      {/* Stats Cards */}
      <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
        <StatCard title="Total Rules" value={stats.total} />
        <StatCard title="Active" value={stats.active} color="green" />
        <StatCard title="Draft" value={stats.draft} color="gray" />
        <StatCard title="Zero Hit" value={stats.zeroHit} color="orange" />
        <StatCard title="Conflicts" value={stats.conflicts} color="red" />
      </div>

      {/* Filter Bar */}
      <div className="flex flex-col md:flex-row gap-2 items-center">
        <div className="relative flex-1 w-full">
          <input
            type="text"
            placeholder="Search rules..."
            value={filter.search || ''}
            onChange={(e) => setFilter({ ...filter, search: (e.target as HTMLInputElement).value })}
            className="pl-9 pr-4 py-2 border rounded-md w-full"
          />
        </div>

        <select
          value={filter.status}
          onChange={(e) =>
            setFilter({
              ...filter,
              status: e.target.value as any,
            })
          }
          className="px-4 py-2 border rounded-md"
        >
          <option value="all">All Statuses</option>
          {Object.keys(STATUS_BADGE_COLORS).map((status) => (
            <option key={status} value={status}>
              {status.charAt(0).toUpperCase() + status.slice(1)}
            </option>
          ))}
        </select>

        <button
          onClick={() => setFilter({ ...filter, search: '' })}
          className="px-4 py-2 border rounded-md hover:bg-gray-100 dark:hover:bg-gray-800"
        >
          Reset
        </button>

        {canCreateRule && (
          <button className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700">
            Create Rule
          </button>
        )}
      </div>

      {/* Rules Table */}
      <table className="w-full border-collapse border">
        <thead>
          <tr className="bg-gray-100 dark:bg-gray-800">
            <th className="p-2 text-left">Name</th>
            <th className="p-2 text-left">Status</th>
            <th className="p-2 text-left hidden md:table-cell">Conditions</th>
          </tr>
        </thead>
        <tbody>
          {isLoading ? (
            <tr>
              <td colSpan={3} className="p-8 text-center">
                Loading rules...
              </td>
            </tr>
          ) : filteredRules.length === 0 ? (
            <tr>
              <td colSpan={3} className="p-8 text-center">
                No rules found. {canCreateRule && 'Click "Create Rule" to add one.'}
              </td>
            </tr>
          ) : (
            filteredRules.map((rule) => (
              <tr key={rule.id} className="border-t hover:bg-gray-50 dark:hover:bg-gray-700">
                <td className="p-2 font-medium">{rule.name}</td>
                <td className="p-2">
                  <span
                    className={`px-2 py-1 rounded text-sm ${
                      STATUS_BADGE_COLORS[rule.status] || 'bg-gray-100 text-gray-700'
                    }`}
                  >
                    {rule.status.charAt(0).toUpperCase() + rule.status.slice(1)}
                  </span>
                </td>
                <td className="p-2 text-sm text-gray-500 hidden md:table-cell">
                  {rule.conditions.length} conditions
                </td>
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  )
}

// Stat Card Component
function StatCard({
  title,
  value,
  color = 'blue',
}: {
  title: string
  value: number
  color?: string
}) {
  const colorClasses: Record<string, string> = {
    blue: 'text-blue-600 dark:text-blue-400',
    green: 'text-green-600 dark:text-green-400',
    gray: 'text-gray-600 dark:text-gray-400',
    orange: 'text-orange-600 dark:text-orange-400',
    red: 'text-red-600 dark:text-red-400',
  }

  return (
    <div className="border rounded-lg p-4 flex items-center justify-between">
      <div>
        <p className="text-sm font-medium text-gray-500">{title}</p>
        <p className={`text-2xl font-bold ${colorClasses[color] || colorClasses.blue}`}>
          {value}
        </p>
      </div>
    </div>
  )
}
