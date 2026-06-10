import { useMemo } from 'react'
import { useAuth } from '@/lib/auth-context'

// Type alias for compatibility - use string directly to avoid conflicts with auth.ts Permission
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
export function usePermission(permission: Perm): boolean {
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

/**
 * Hook to check multiple permissions at once
 */
export function usePermissions(...permissions: Perm[]): Record<Perm, boolean> {
  const { user } = useAuth()

  return useMemo(() => {
    if (!user) {
      return Object.fromEntries(permissions.map((p) => [p, false])) as Record<Perm, boolean>
    }

    // Admin has all permissions
    if (user.role === 'admin') {
      return Object.fromEntries(permissions.map((p) => [p, true])) as Record<Perm, boolean>
    }

    const userPermissions = new Set<string>((user as unknown as UserPermissions).permissions || [])
    return Object.fromEntries(
      permissions.map((p) => [p, userPermissions.has(p)])
    ) as Record<Perm, boolean>
  }, [user, permissions])
}

/**
 * Hook to check if user has any of the specified permissions
 */
export function useAnyPermission(...permissions: Perm[]): boolean {
  const { user } = useAuth()

  return useMemo(() => {
    if (!user) return false
    if (user.role === 'admin') return true

    const userPermissions = new Set<string>((user as unknown as UserPermissions).permissions || [])
    return permissions.some((p) => userPermissions.has(p))
  }, [user, permissions])
}

/**
 * Hook to check role-based access control
 */
export function useRole(role: string): boolean {
  const { user } = useAuth()
  return user?.role === role
}
