import { Link } from '@tanstack/react-router'
import { useAuth } from '@/lib/auth-context'

// Sections that make up the application. They are listed whether or not a session
// exists: the server enforces authorisation on `/v1`, and hiding the navigation
// behind a login left a visitor with a landing page and no way to discover the rest.
const SECTIONS = [
  { label: 'Rules', path: '/rules' as const },
  { label: 'Governance', path: '/governance/approvals' as const },
  { label: 'Monitoring', path: '/monitoring' as const },
]

export function Navbar() {
  const { user, isAuthenticated, logout } = useAuth()

  return (
    <nav className="border-b">
      <div className="container mx-auto flex items-center justify-between px-4 py-3">
        <div className="flex items-center gap-6">
          <Link to="/" className="text-xl font-bold">
            rRules
          </Link>
          <div className="flex gap-4">
            {SECTIONS.map((section) => (
              <Link
                key={section.path}
                to={section.path}
                className="text-sm hover:underline"
                activeProps={{ className: 'text-sm font-semibold underline' }}
              >
                {section.label}
              </Link>
            ))}
          </div>
        </div>
        <div className="flex items-center gap-4">
          {isAuthenticated ? (
            <>
              <span className="text-sm text-gray-500">{user?.displayName}</span>
              <button onClick={logout} className="text-sm hover:underline">
                Logout
              </button>
            </>
          ) : (
            <Link to="/login" className="text-sm hover:underline">
              Login
            </Link>
          )}
        </div>
      </div>
    </nav>
  )
}
