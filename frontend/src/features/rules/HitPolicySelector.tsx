import { HIT_POLICY_META, AGGREGATORS, type HitPolicy, type Aggregator } from './types'

export interface HitPolicySelectorProps {
  value: HitPolicy
  onChange: (policy: HitPolicy) => void
  showDescription?: boolean
  aggregator?: Aggregator
  onAggregatorChange?: (agg: Aggregator) => void
}

function AggregatorSelector({
  aggregator,
  onAggregatorChange,
}: {
  aggregator?: Aggregator
  onAggregatorChange?: (agg: Aggregator) => void
}) {
  return (
    <div className="space-y-1">
      <label className="font-medium text-sm">Aggregator:</label>
      <select
        value={aggregator ?? 'SUM'}
        onChange={(e) => onAggregatorChange?.(e.target.value as Aggregator)}
        className="w-full px-4 py-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        {AGGREGATORS.map((agg) => (
          <option key={agg.id} value={agg.id}>
            {agg.name}
          </option>
        ))}
      </select>
    </div>
  )
}

export function HitPolicySelector({
  value,
  onChange,
  showDescription = true,
  aggregator,
  onAggregatorChange,
}: HitPolicySelectorProps) {
  const meta = HIT_POLICY_META[value]

  return (
    <div className="space-y-2">
      <label className="font-medium text-sm">Hit Policy:</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as HitPolicy)}
        className="w-full px-4 py-2 border rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
      >
        {Object.values(HIT_POLICY_META).map((policyMeta) => (
          <option key={policyMeta.id} value={policyMeta.id}>
            {policyMeta.name}
          </option>
        ))}
      </select>

      {showDescription && meta && (
        <div className="text-sm text-gray-600 dark:text-gray-400">
          {meta.description}
        </div>
      )}

      {value === 'COLLECT' && onAggregatorChange && (
        <AggregatorSelector aggregator={aggregator} onAggregatorChange={onAggregatorChange} />
      )}
    </div>
  )
}

export function HitPolicyBadge({ policy }: { policy: HitPolicy }) {
  const meta = HIT_POLICY_META[policy]

  // Purple for COLLECT only (the only policy that requires an aggregator)
  const colorClass = meta?.requiresAggregator
    ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400'
    : 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400'

  return (
    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${colorClass}`}>
      {meta?.name || policy}
      {meta?.requiresAggregator && (
        <span className="ml-1.5 inline-block w-1 h-1 bg-current rounded-full"></span>
      )}
    </span>
  )
}

export default HitPolicySelector
