// RBAC exports for Phase 18 Enterprise Governance

export { ProtectedRoute, withPermissionProtection } from './ProtectedRoute'
export { usePermission, usePermissions, useAnyPermission, useRole } from '@/hooks/use-permission'
export type { RoleName, UserWithRoles, PermissionInfo, RoleAssignment } from './types'
