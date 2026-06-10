import { Navigate } from '@tanstack/react-router'
import { usePermission } from './usePermission'
import type { Permission as AuthPermission } from '@/types/auth'

interface ProtectedRouteProps {
  children: React.ReactNode
  requiredPermission?: string | AuthPermission
}

export function ProtectedRoute({
  children,
  requiredPermission,
}: ProtectedRouteProps) {
  const { isAuthenticated, hasPermission } = usePermission()

  if (!isAuthenticated) {
    return <Navigate to="/login" />
  }

  if (requiredPermission && !hasPermission(requiredPermission as AuthPermission)) {
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
  requiredPermission: string | AuthPermission,
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
