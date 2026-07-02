import { createFileRoute } from '@tanstack/react-router'

function IndexComponent() {
  return (
    <div className="space-y-4 p-6">
      <h1 className="text-2xl font-bold">Welcome to rRules</h1>
      <p>A Business Rules Management System for defining and managing complex business decisions.</p>
    </div>
  )
}

export const Route = createFileRoute('/')({
  component: IndexComponent,
})