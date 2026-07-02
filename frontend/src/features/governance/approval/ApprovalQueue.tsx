import { useState, useEffect, useCallback } from 'react'
import apiClient from '@/api/client'
import { useAuth } from '@/lib/auth-context'
import type { ApprovalRequest } from './types'

export function ApprovalQueue() {
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [acting, setActing] = useState<string | null>(null)
  const { user } = useAuth()

  const fetchPendingApprovals = useCallback(async () => {
    try {
      setLoading(true)
      setError(null)
      // All fetch via shared apiClient (baseURL = VITE_API_URL /v1) — no hardcoded base.
      const response = await apiClient.get<{ approvals: ApprovalRequest[] }>(
        '/approvals/pending',
      )
      setApprovals(response.data.approvals || [])
    } catch (err) {
      console.error('Failed to fetch approvals:', err)
      setError(err instanceof Error ? err.message : 'Failed to load approvals')
      setApprovals([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchPendingApprovals()
  }, [fetchPendingApprovals])

  async function handleReview(id: string, approve: boolean) {
    try {
      setActing(id)
      setError(null)
      const endpoint = approve ? `/approvals/${id}/approve` : `/approvals/${id}/reject`
      // Backend ApprovalActionRequest.approver_id — use resolved user id when
      // available, else 'current-user' for the demo (RESEARCH A4).
      await apiClient.post(endpoint, { approver_id: user?.id ?? 'current-user' })
      // Refresh the pending list from the backend after a successful action.
      await fetchPendingApprovals()
    } catch (err) {
      console.error(`${approve ? 'Approve' : 'Reject'} failed:`, err)
      setError(err instanceof Error ? err.message : `Failed to ${approve ? 'approve' : 'reject'}`)
    } finally {
      setActing(null)
    }
  }

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Approval Queue</h1>

      {error && (
        <div className="mb-4 rounded-md bg-red-50 p-4 dark:bg-red-900/20">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          <button
            onClick={fetchPendingApprovals}
            className="mt-2 btn btn-outline btn-sm"
          >
            Retry
          </button>
        </div>
      )}

      {loading ? (
        <p>Loading pending approvals...</p>
      ) : approvals.length === 0 ? (
        <div className="card p-8 text-center">
          <h3 className="text-lg font-semibold">No Pending Approvals</h3>
          <p className="text-gray-500 mt-1">All approval requests have been processed.</p>
        </div>
      ) : (
        <div className="space-y-4">
          {approvals.map((approval) => (
            <div key={approval.id} className="card p-4 border rounded-lg">
              <div className="flex items-start justify-between">
                <div>
                  <h3 className="font-semibold">Rule: {approval.rule_id.slice(0, 8)}...</h3>
                  <p className="text-sm text-gray-600 mt-1">
                    Submitted by {approval.maker_id} on {new Date(approval.created_at).toLocaleString()}
                  </p>
                  <p className="text-xs text-gray-400 mt-1">State: {approval.state}</p>
                </div>
                <div className="flex space-x-2">
                  <button
                    onClick={() => handleReview(approval.id, true)}
                    disabled={acting === approval.id}
                    className="btn btn-primary disabled:opacity-50"
                  >
                    Approve
                  </button>
                  <button
                    onClick={() => handleReview(approval.id, false)}
                    disabled={acting === approval.id}
                    className="btn btn-error disabled:opacity-50"
                  >
                    Reject
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      <button
        onClick={fetchPendingApprovals}
        className="mt-4 btn btn-outline"
      >
        Refresh
      </button>
    </div>
  )
}
