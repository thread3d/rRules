import { useAuth } from '@/lib/auth-context'
import type { Permission as AuthPermission } from '@/types/auth'

export function usePermission() {
  const { user, isAuthenticated } = useAuth()

  // Check if user has a specific permission string
  const hasPermission = (permission: AuthPermission): boolean => {
    if (!isAuthenticated || !user) return false
    if (user.role === 'admin') return true

    // Check user's permissions list
    return user.permissions?.includes(permission)
  }

  // Convenience methods for common permission checks
  const canCreateRules = () => hasPermission('rule:create')
  const canApproveRules = () => hasPermission('approval:approve')
  const canManageRoles = () => hasPermission('settings:update')

  return {
    hasPermission,
    canCreateRules,
    canApproveRules,
    canManageRoles,
    isAuthenticated,
  }
}
