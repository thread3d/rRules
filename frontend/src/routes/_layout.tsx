import {
  createRootRoute,
  Outlet,
} from '@tanstack/react-router'

export const Route = createRootRoute({
  component: RootComponent,
})

function RootComponent() {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <Layout>
        <Outlet />
      </Layout>
    </div>
  )
}

// Simple layout component
function Layout({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col">
      {/* Navbar would go here */}
      <main className="flex-1">{children}</main>
    </div>
  )
}
