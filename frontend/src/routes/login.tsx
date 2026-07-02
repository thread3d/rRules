import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useAuth } from '@/lib/auth-context'

// Minimal login page so Navbar/ProtectedRoute typed `<Link to="/login" />`
// resolves (D-22-09). Full login UX polish is out of scope for 22-03.
function LoginPage() {
  const { login } = useAuth()
  const navigate = useNavigate()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setSubmitting(true)
    try {
      await login({ email, password })
      navigate({ to: '/' })
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center p-6">
      <form onSubmit={handleSubmit} className="w-full max-w-sm space-y-4 border rounded-lg p-6 bg-white dark:bg-gray-800">
        <h1 className="text-2xl font-bold">Sign in to rRules</h1>
        {error && (
          <div className="rounded-md bg-red-50 p-3 text-sm text-red-600 dark:bg-red-900/20">
            {error}
          </div>
        )}
        <label className="block space-y-1">
          <span className="text-sm font-medium">Email</span>
          <input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            className="w-full px-3 py-2 border rounded-md"
          />
        </label>
        <label className="block space-y-1">
          <span className="text-sm font-medium">Password</span>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            className="w-full px-3 py-2 border rounded-md"
          />
        </label>
        <button
          type="submit"
          disabled={submitting}
          className="w-full px-4 py-2 bg-blue-600 text-white rounded-md disabled:opacity-50"
        >
          {submitting ? 'Signing in…' : 'Sign in'}
        </button>
      </form>
    </div>
  )
}

export const Route = createFileRoute('/login')({
  component: LoginPage,
})