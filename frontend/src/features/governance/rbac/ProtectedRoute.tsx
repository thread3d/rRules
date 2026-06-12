import { Navigate } from '@tanstack/react-router'
import { useMemo } from 'react'
import { useAuth } from '@/lib/auth-context'

// Type alias for compatibility
type Perm = string

interface UserPermissions {
  permissions: string[]
}

/**
 * Hook to check if the current user has a specific permission.
 *
 * @param permission - The permission string to check (e.g., 'rule:write')
 * @returns True if user has the permission, false otherwise
 */
function usePermission(permission: Perm): boolean {
  const { user } = useAuth()

  return useMemo(() => {
    if (!user) return false

    // Admin has all permissions
    if (user.role === 'admin') return true

    // Check if permission is in user's permissions array
    const permissions = (user as unknown as UserPermissions).permissions || []
    return permissions.includes(permission)
  }, [user, permission])
}

interface ProtectedRouteProps {
  children: React.ReactNode
  requiredPermission?: string | Perm
}

export function ProtectedRoute({
  children,
  requiredPermission,
}: ProtectedRouteProps) {
  const hasPermission = usePermission(requiredPermission as Perm)

  // Check auth via localStorage - simplified for demo
  const isAuthenticated = useMemo(() => {
    const token = localStorage.getItem('auth_token')
    return !!token
  }, [])

  if (!isAuthenticated) {
    return <Navigate to="/login" />
  }

  if (requiredPermission && !hasPermission) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <div className="text-center">
          <h1 className="text-2xl font-bold">Access Denied</h1>
          <p className="mt-2 text-gray-600">You don't have permission to view this page.</p>
        </div>
      </div>
    )
  }

  return <>{children}</>
}

export function withPermissionProtection<T extends Record<string, unknown>>(
  Component: React.ComponentType<T>,
  requiredPermission: string | Perm,
) {
  // eslint-disable-next-line react/display-name
  return function ProtectedComponent(props: T) {
    return (
      <ProtectedRoute requiredPermission={requiredPermission}>
        <Component {...props} />
      </ProtectedRoute>
    )
  }
}
