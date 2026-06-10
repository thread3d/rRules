import { HIT_POLICIES, HitPolicyId } from './types'

export interface HitPolicySelectorProps {
  value: HitPolicyId
  onChange: (policy: HitPolicyId) => void
  showDescription?: boolean
}

export function HitPolicySelector({
  value,
  onChange,
  showDescription = true,
}: HitPolicySelectorProps) {
  return (
    <div className="space-y-2">
      <label className="font-medium text-sm">Hit Policy:</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as HitPolicyId)}
        className="w-full px-4 py-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        {HIT_POLICIES.map((policy) => (
          <option key={policy.id} value={policy.id}>
            {policy.name}
          </option>
        ))}
      </select>

      {showDescription && (
        <div className="text-sm text-gray-600 dark:text-gray-400">
          {
            HIT_POLICIES.find((p) => p.id === value)?.description
          }
        </div>
      )}
    </div>
  )
}

export function HitPolicyBadge({ policy }: { policy: HitPolicyId }) {
  const hitPolicy = HIT_POLICIES.find((p) => p.id === policy)

  // Color based on whether it requires aggregation
  const colorClass = hitPolicy?.requiresAggregation
    ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400'
    : 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'

  return (
    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${colorClass}`}>
      {hitPolicy?.name || policy}
      {hitPolicy?.requiresAggregation && (
        <span className="ml-1.5 inline-block w-1 h-1 bg-current rounded-full"></span>
      )}
    </span>
  )
}

export default HitPolicySelector
