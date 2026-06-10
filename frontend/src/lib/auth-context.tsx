import { createContext, useContext, useEffect, useState } from 'react'
import type {
  AuthState,
  AuthCredentials,
  User,
} from '@/types/auth'

interface AuthContextValue {
  user: User | null
  isAuthenticated: boolean
  isLoading: boolean
  login: (credentials: AuthCredentials) => Promise<void>
  logout: () => void
  refreshUser: () => Promise<void>
  hasPermission: (permission: string, resourceId?: string) => boolean
  hasRole: (role: string) => boolean
}

const AuthContext = createContext<AuthContextValue | undefined>(undefined)

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<AuthState>({
    user: null,
    isAuthenticated: false,
    isLoading: true,
    error: null,
  })

  // Check auth status on mount
  useEffect(() => {
    checkAuth()
  }, [])

  async function checkAuth() {
    try {
      const token = localStorage.getItem('auth_token')
      if (!token) {
        setState((prev) => ({ ...prev, isLoading: false }))
        return
      }

      // Fetch user from API
      const response = await fetch('/api/v1/auth/me', {
        headers: { Authorization: `Bearer ${token}` },
      })

      if (response.ok) {
        const user = (await response.json()) as User
        localStorage.setItem('user', JSON.stringify(user))
        setState({
          user,
          isAuthenticated: true,
          isLoading: false,
          error: null,
        })
      } else {
        clearAuth()
      }
    } catch (error) {
      clearAuth()
    }
  }

  async function login(credentials: AuthCredentials): Promise<void> {
    setState((prev) => ({ ...prev, isLoading: true, error: null }))

    try {
      const response = await fetch('/api/v1/auth/login', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(credentials),
      })

      if (response.ok) {
        const data = await response.json()
        localStorage.setItem('auth_token', data.token)
        localStorage.setItem('user', JSON.stringify(data.user))
        setState({
          user: data.user,
          isAuthenticated: true,
          isLoading: false,
          error: null,
        })
      } else {
        const errorData = await response.json()
        setState((prev) => ({
          ...prev,
          isLoading: false,
          error: errorData.message || 'Login failed',
        }))
      }
    } catch (error) {
      setState((prev) => ({
        ...prev,
        isLoading: false,
        error: 'Network error. Please try again.',
      }))
    }
  }

  function logout() {
    clearAuth()
    setState({
      user: null,
      isAuthenticated: false,
      isLoading: false,
      error: null,
    })
  }

  function refreshUser(): Promise<void> {
    return checkAuth()
  }

  function hasPermission(permission: string, resourceId?: string): boolean {
    if (!state.user) return false
    if (state.user.role === 'admin') return true

    const permissions = state.user.permissions || []
    const hasDirectPermission = permissions.includes(permission as any)

    if (!resourceId) return hasDirectPermission

    // For resource-specific checks, additional logic would be needed
    return hasDirectPermission
  }

  function hasRole(role: string): boolean {
    return state.user?.role === role
  }

  function clearAuth() {
    localStorage.removeItem('auth_token')
    localStorage.removeItem('user')
  }

  const value: AuthContextValue = {
    user: state.user,
    isAuthenticated: state.isAuthenticated,
    isLoading: state.isLoading,
    login,
    logout,
    refreshUser,
    hasPermission,
    hasRole,
  }

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth() {
  const context = useContext(AuthContext)
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider')
  }
  return context
}

export default AuthContext
