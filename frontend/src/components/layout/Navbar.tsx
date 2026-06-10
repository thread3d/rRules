import { Link } from '@tanstack/react-router'
import { useAuth } from '@/lib/auth-context'

export function Navbar() {
  const { user, isAuthenticated, logout } = useAuth()

  if (!isAuthenticated) {
    return (
      <nav className="border-b">
        <div className="container mx-auto px-4 py-3 flex justify-between items-center">
          <Link to="/" className="text-xl font-bold">rRules</Link>
          <div className="flex gap-4">
            <Link to="/login" className="hover:underline">Login</Link>
          </div>
        </div>
      </nav>
    )
  }

  // Navigation items based on role
  const navItems = [
    { label: 'Rules', path: '/rules', roles: ['viewer', 'author', 'reviewer', 'admin'] },
    { label: 'New Rule', path: '/rules/new', roles: ['author', 'admin'] },
  ]

  return (
    <nav className="border-b">
      <div className="container mx-auto px-4 py-3 flex justify-between items-center">
        <Link to="/" className="text-xl font-bold">rRules</Link>
        <div className="flex gap-4 items-center">
          {navItems.map((item) => {
            const hasAccess = user ? item.roles.includes(user.role) : false
            if (!hasAccess) return null
            return (
              <Link key={item.path} to={item.path} className="hover:underline">
                {item.label}
              </Link>
            )
          })}
          <span className="text-sm text-gray-500">{user?.displayName}</span>
          <button onClick={logout} className="text-sm hover:underline">Logout</button>
        </div>
      </div>
    </nav>
  )
}
