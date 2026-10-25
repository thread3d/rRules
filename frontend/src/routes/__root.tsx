import { createRootRoute, Outlet } from '@tanstack/react-router'
import { Navbar } from '@/components/layout/Navbar'

export const Route = createRootRoute({
  component: RootComponent,
  notFoundComponent: NotFound,
})

function RootComponent() {
  return (
    <div className="min-h-screen bg-background text-foreground">
      {/* The navbar existed but nothing mounted it, so the application had no
          navigation at all: every page was reachable only by typing its URL. */}
      <Navbar />
      <main className="flex-1">
        <Outlet />
      </main>
    </div>
  )
}

function NotFound() {
  return (
    <div className="mx-auto max-w-2xl space-y-3 p-6">
      <h1 className="text-2xl font-bold">Page not found</h1>
      <p className="text-gray-500">
        That route does not exist. Use the navigation above to reach the rule list,
        governance, or monitoring.
      </p>
    </div>
  )
}
