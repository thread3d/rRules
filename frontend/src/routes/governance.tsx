// Governance routes for rRules BRMS

import { createFileRoute, Link, Outlet } from '@tanstack/react-router'

export function GovernancePage() {
  return (
    <div className="p-6 space-y-8">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold">Governance</h1>
        <p className="text-gray-500">Approval workflows, audit trails, and access control</p>
      </div>

      {/* Dashboard Tabs */}
      <div className="flex gap-4 border-b overflow-x-auto">
        <Link
          to="/governance/approvals"
          activeOptions={{ exact: true }}
          className="py-2 px-4 font-medium whitespace-nowrap border-b-2 transition-colors data-[active=true]:border-blue-600 data-[active=true]:text-blue-600 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          Approval Queue
        </Link>
        <Link
          to="/governance/audit"
          className="py-2 px-4 font-medium whitespace-nowrap border-b-2 transition-colors data-[active=true]:border-blue-600 data-[active=true]:text-blue-600 text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
        >
          Audit Log
        </Link>
      </div>

      {/* Dynamic content based on route */}
      <Outlet />
    </div>
  )
}

export const Route = createFileRoute('/governance')({
  component: GovernancePage,
})
