// Auth types for rRules BRMS

export interface User {
  id: string
  email: string
  displayName: string
  role: Role
  permissions: Permission[]
  avatarUrl?: string
  lastLoginAt?: string
}

export type Role = 'viewer' | 'author' | 'reviewer' | 'admin'

export type Permission =
  | 'rule:read'
  | 'rule:create'
  | 'rule:update'
  | 'rule:delete'
  | 'approval:read'
  | 'approval:create'
  | 'approval:approve'
  | 'approval:reject'
  | 'metrics:read'
  | 'settings:read'
  | 'settings:update'

export interface AuthState {
  user: User | null
  isAuthenticated: boolean
  isLoading: boolean
  error: string | null
}

export interface AuthCredentials {
  email: string
  password: string
}

export interface AuthContextValue {
  user: User | null
  isAuthenticated: boolean
  isLoading: boolean
  login: (credentials: AuthCredentials) => Promise<void>
  logout: () => void
  refreshUser: () => Promise<void>
  hasPermission: (permission: Permission, resourceId?: string) => boolean
  hasRole: (role: Role) => boolean
}
