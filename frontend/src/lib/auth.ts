import type { User, Role } from '@/types/auth'

// Check if user has a specific role
export function hasRole(user: User | null, role: Role): boolean {
  if (!user) return false
  return user.role === role
}

// Check if user has a specific permission
export function hasPermission(user: User | null, permission: string): boolean {
  if (!user) return false

  // Admin has all permissions
  if (user.role === 'admin') return true

  // Check if permission is in user's permissions array
  const permissions = user.permissions || []
  return permissions.includes(permission as any)
}

// Get allowed actions for a specific resource based on user role
export function getAllowedActions(
  user: User | null,
  resourceType: string
): Record<string, boolean> {
  if (!user) {
    return {}
  }

  const isAdmin = user.role === 'admin'

  switch (resourceType) {
    case 'rule':
      return {
        read: true,
        create: isAdmin || user.role === 'author',
        update: isAdmin || user.role === 'author',
        delete: isAdmin,
      }
    case 'approval':
      return {
        read: true,
        create: true,
      }
    default:
      return {}
  }
}

// Parse user from localStorage
export function parseUserFromStorage(): User | null {
  try {
    const stored = localStorage.getItem('user')
    if (!stored) return null
    return JSON.parse(stored) as User
  } catch {
    return null
  }
}

// Check authentication status
export function isAuthenticated(): boolean {
  const token = localStorage.getItem('auth_token')
  return !!token && parseUserFromStorage() !== null
}
