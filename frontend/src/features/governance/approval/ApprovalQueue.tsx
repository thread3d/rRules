import { useState, useEffect } from 'react'

export function ApprovalQueue() {
  const [requests, setRequests] = useState<ApprovalRequest[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    fetchPendingApprovals()
  }, [])

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold">Approval Queue</h1>

      {loading ? (
        <p>Loading pending approvals...</p>
      ) : requests.length === 0 ? (
        <div className="card p-8 text-center">
          <h3 className="text-lg font-semibold">No Pending Approvals</h3>
          <p className="text-gray-500 mt-1">All approval requests have been processed.</p>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {requests.map((request) => (
            <ApprovalCard
              key={request.id}
              request={request}
              onApprove={() => handleReview(request.id, true)}
              onReject={() => handleReview(request.id, false)}
            />
          ))}
        </div>
      )}
    </div>
  )

  async function fetchPendingApprovals() {
    try {
      const response = await fetch('/api/v1/approvals?state=submitted')
      if (response.ok) {
        setRequests(await response.json())
      }
    } finally {
      setLoading(false)
    }
  }

  async function handleReview(id: string, approve: boolean, reason?: string) {
    try {
      const response = await fetch(`/api/v1/approvals/${id}/review`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ approve, reason }),
      })

      if (response.ok) {
        setRequests((prev) => prev.filter((r) => r.id !== id))
      }
    } catch (error) {
      console.error('Approval failed:', error)
    }
  }
}

interface ApprovalRequest {
  id: string
  ruleName: string
  makerName: string
  submittedAt: string
  tier: number
  workflowType: string
}

function ApprovalCard({
  request,
  onApprove,
  onReject,
}: {
  request: ApprovalRequest
  onApprove: () => void
  onReject: () => void
}) {
  return (
    <div className="card p-4">
      <h3 className="font-semibold">{request.ruleName}</h3>
      <p className="text-sm text-gray-600 mt-1">
        Submitted by {request.makerName} on {new Date(request.submittedAt).toLocaleString()}
      </p>
      <div className="mt-4 flex justify-end space-x-2">
        <button
          onClick={onReject}
          className="btn btn-error"
        >
          Reject
        </button>
        <button
          onClick={onApprove}
          className="btn btn-primary"
        >
          Approve
        </button>
      </div>
    </div>
  )
}
