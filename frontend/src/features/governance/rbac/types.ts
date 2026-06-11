// RBAC types for Phase 18 Enterprise Governance

export interface Role {
  id: number
  name: string
  description?: string
  isDefault: boolean
  createdAt: string
}

export type RoleName = 'admin' | 'author' | 'reviewer' | 'viewer'

export interface UserWithRoles {
  userId: string
  email: string
  displayName: string
  roles: string[]
  permissions: PermissionInfo[]
}

export interface PermissionInfo {
  resource: string
  action: string
  scope: string
}

export interface RoleAssignment {
  userId: string
  roleId: number
  assignedBy?: string
  assignedAt?: string
}

export const ROLE_NAMES: RoleName[] = ['admin', 'author', 'reviewer', 'viewer']

export const ROLE_DESCRIPTIONS: Record<RoleName, string> = {
  admin: 'Full access to all features including system configuration',
  author: 'Create and edit rules, submit for approval',
  reviewer: 'Review and approve/reject rule changes',
  viewer: 'Read-only access to rules and audit logs',
}

export const ROLE_PERMISSIONS: Record<RoleName, PermissionInfo[]> = {
  admin: [
    { resource: '*', action: '*', scope: '*' },
  ],
  author: [
    { resource: 'rule', action: 'read', scope: '*' },
    { resource: 'rule', action: 'create', scope: '*' },
    { resource: 'rule', action: 'update', scope: '*' },
    { resource: 'approval', action: 'submit', scope: '*' },
  ],
  reviewer: [
    { resource: 'rule', action: 'read', scope: '*' },
    { resource: 'approval', action: 'read', scope: '*' },
    { resource: 'approval', action: 'review', scope: '*' },
    { resource: 'audit', action: 'read', scope: '*' },
  ],
  viewer: [
    { resource: 'rule', action: 'read', scope: '*' },
    { resource: 'audit', action: 'read', scope: '*' },
  ],
}
