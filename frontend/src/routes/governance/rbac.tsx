import { createFileRoute } from '@tanstack/react-router'
import { useAuth } from '@/lib/auth-context'

// TODO(22-05/22-06): a full RBAC admin page (role/permission assignment UI)
// does not yet exist in features/governance/rbac (only ProtectedRoute +
// usePermission helpers). This route mounts a minimal admin surface that
// surfaces the current user's role + permissions so the route resolves and
// the auth context is exercised (D-22-09). The full management UI lands in
// 22-05/22-06.
function RbacAdminPage() {
  const { user } = useAuth()

  return (
    <div className="p-6 space-y-4">
      <div>
        <h1 className="text-2xl font-bold">RBAC Administration</h1>
        <p className="text-gray-500">Role-based access control management.</p>
      </div>
      <div className="border rounded-lg p-4 bg-white dark:bg-gray-800 space-y-2">
        <div className="text-sm text-gray-500">Current user</div>
        {user ? (
          <>
            <div className="text-lg font-semibold">{user.displayName} ({user.role})</div>
            <div className="text-sm text-gray-500 mt-2">Permissions</div>
            <pre className="text-xs bg-gray-50 dark:bg-gray-900 p-2 rounded">
              {JSON.stringify(user.permissions ?? [], null, 2)}
            </pre>
          </>
        ) : (
          <div className="text-sm text-gray-500">Not signed in.</div>
        )}
      </div>
    </div>
  )
}

export const Route = createFileRoute('/governance/rbac')({
  component: RbacAdminPage,
})