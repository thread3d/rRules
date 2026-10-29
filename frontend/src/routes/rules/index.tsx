import { createFileRoute } from '@tanstack/react-router'
import { RulesList, useRules } from '@/features/rules'

function RulesPage() {
  const { rules, isLoading, error, onRefresh, totalCount, page, totalPages, goToPage } = useRules()

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Manage Rules</h1>
      {error && (
        <div className="mb-4 rounded-md bg-red-50 p-4 dark:bg-red-900/20">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
        </div>
      )}
      <RulesList
        rules={rules}
        isLoading={isLoading}
        onRefresh={onRefresh}
        totalCount={totalCount}
        page={page}
        totalPages={totalPages}
        goToPage={goToPage}
      />
    </div>
  )
}

export const Route = createFileRoute('/rules/')({
  component: RulesPage,
})