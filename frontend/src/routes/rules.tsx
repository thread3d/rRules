// Rules routes for rRules BRMS

import { RulesList } from '@/features/rules'
import { useRules } from '@/features/rules'

export function RulesPage() {
  const { rules, isLoading, error } = useRules()

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Manage Rules</h1>
      {error && (
        <div className="mb-4 rounded-md bg-red-50 p-4 dark:bg-red-900/20">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}
      <RulesList rules={rules} isLoading={isLoading} />
    </div>
  )
}
