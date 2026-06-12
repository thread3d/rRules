import { useState, useEffect } from 'react'
import apiClient from '@/api/client'
import type { ApprovalRequest } from './types'

export function ApprovalQueue() {
  const [approvals, setApprovals] = useState<ApprovalRequest[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetchPendingApprovals()
  }, [])

  async function fetchPendingApprovals() {
    try {
      setLoading(true)
      const response = await apiClient.get<{ approvals: ApprovalRequest[] }>('/approvals/pending')
      setApprovals(response.data.approvals || [])
      setError(null)
    } catch (err) {
      console.error('Failed to fetch approvals:', err)
      setError(err instanceof Error ? err.message : 'Failed to load approvals')
    } finally {
      setLoading(false)
    }
  }

  async function handleReview(id: string, approve: boolean) {
    try {
      const endpoint = approve ? `/approvals/${id}/approve` : `/approvals/${id}/reject`
      await apiClient.post(endpoint, { approver_id: 'current-user' })
      setApprovals((prev) => prev.filter((a) => a.id !== id))
    } catch (err) {
      console.error(`${approve ? 'Approve' : 'Reject'} failed:`, err)
      setError(err instanceof Error ? err.message : `Failed to ${approve ? 'approve' : 'reject'}`)
    }
  }

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Approval Queue</h1>

      {error && (
        <div className="mb-4 rounded-md bg-red-50 p-4 dark:bg-red-900/20">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
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
                    className="btn btn-primary"
                  >
                    Approve
                  </button>
                  <button
                    onClick={() => handleReview(approval.id, false)}
                    className="btn btn-error"
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
