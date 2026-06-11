import { useState } from 'react'
import { usePermission } from '@/features/governance/rbac/usePermission'

export function AuditLogViewer() {
  const { canManageRoles } = usePermission()
  const [events, setEvents] = useState<AuditEvent[]>([])
  const [filter, setFilter] = useState<AuditFilter>({})

  if (!canManageRoles) {
    return <div className="p-4">Access denied. Admin permissions required.</div>
  }

  // Fetch events on mount and when filter changes
  // This would connect to the API endpoint

  return (
    <div className="space-y-4">
      <h1 className="text-2xl font-bold">Audit Log</h1>

      {/* Filter Form */}
      <div className="card space-y-3 p-4">
        <div className="grid grid-cols-2 gap-4">
          <select
            value={filter.eventType || ''}
            onChange={(e) => setFilter({ ...filter, eventType: e.target.value })}
            className="input input-sm"
          >
            <option value="">All Event Types</option>
            <option value="rule.created">Rule Created</option>
            <option value="rule.updated">Rule Updated</option>
            <option value="role.assigned">Role Assigned</option>
            <option value="approval.submitted">Approval Submitted</option>
          </select>

          <input
            type="date"
            value={filter.startTime || ''}
            onChange={(e) => setFilter({ ...filter, startTime: e.target.value })}
            className="input input-sm"
          />
        </div>

        <button
          onClick={() => fetchEvents(filter)}
          className="btn btn-primary"
        >
          Apply Filters
        </button>
      </div>

      {/* Events List */}
      <div className="card">
        <h2 className="text-lg font-semibold">Audit Events</h2>
        {events.length === 0 ? (
          <p className="text-gray-500">No audit events found.</p>
        ) : (
          <div className="space-y-2">
            {events.map((event) => (
              <AuditEventCard key={event.eventId} event={event} />
            ))}
          </div>
        )}
      </div>

      {/* Export Button */}
      <button
        onClick={() => handleExport()}
        className="btn btn-outline"
      >
        Export to CSV
      </button>
    </div>
  )

  async function fetchEvents(filter: AuditFilter) {
    // API call would go here
  }

  async function handleExport() {
    // Export functionality would go here
  }
}

export interface AuditEvent {
  eventId: string
  eventType: string
  actorId?: number
  actorName?: string
  subjectType: string
  subjectId?: string
  occurredAt: string
  payload: Record<string, unknown>
}

interface AuditFilter {
  eventType?: string
  startTime?: string
  endTime?: string
  actorId?: string
}

function AuditEventCard({ event }: { event: AuditEvent }) {
  const icons: Record<string, string> = {
    'rule.created': 'file-plus',
    'rule.updated': 'file-text',
    'role.assigned': 'shield-check',
    'approval.submitted': 'check-circle',
  }

  return (
    <div className="card p-4">
      <div className="flex items-start justify-between">
        <div>
          <span className="badge badge-sm badge-outline mb-2">
            {event.eventType}
          </span>
          <p className="text-sm text-gray-600">{JSON.stringify(event.payload, null, 2)}</p>
          <p className="text-xs text-gray-400 mt-1">
            At {new Date(event.occurredAt).toLocaleString()} by user {event.actorId}
          </p>
        </div>
      </div>
    </div>
  )
}
