import { useState, useEffect } from 'react'

export function AuditLogViewer() {
  const [events, setEvents] = useState<AuditEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState<AuditFilter>({})

  // Check permissions - simplified
  const canManageRoles = true

  useEffect(() => {
    if (canManageRoles) {
      fetchEvents({})
    }
  }, [])

  async function fetchEvents(filter: AuditFilter) {
    try {
      setLoading(true)
      const params: Record<string, string> = {}
      if (filter.eventType) params.event_type = filter.eventType
      if (filter.startTime) params.start_time = filter.startTime

      console.log('Fetching audit events with filter:', filter)

      // This would fetch from /v1/audit endpoint in production
      const mockEvents: AuditEvent[] = [
        {
          eventId: 'evt-001',
          eventType: 'rule.created',
          actorId: 1,
          actorName: 'admin',
          subjectType: 'rule',
          subjectId: 'rule-001',
          occurredAt: new Date().toISOString(),
          payload: { rule_name: 'Test Rule' },
        },
      ]

      setEvents(mockEvents)
    } finally {
      setLoading(false)
    }
  }

  if (!canManageRoles) {
    return (
      <div className="p-4">
        Access denied. Admin permissions required to view audit logs.
      </div>
    )
  }

  const eventTypes = [
    { value: '', label: 'All Event Types' },
    { value: 'rule.created', label: 'Rule Created' },
    { value: 'rule.updated', label: 'Rule Updated' },
    { value: 'role.assigned', label: 'Role Assigned' },
    { value: 'approval.submitted', label: 'Approval Submitted' },
  ]

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Audit Log</h1>

      {/* Filter Form */}
      <div className="card space-y-3 p-4 mb-4">
        <div className="grid grid-cols-2 gap-4">
          <select
            value={filter.eventType || ''}
            onChange={(e) => setFilter({ ...filter, eventType: e.target.value })}
            className="border rounded px-3 py-2"
          >
            {eventTypes.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>

          <input
            type="date"
            value={filter.startTime || ''}
            onChange={(e) => setFilter({ ...filter, startTime: e.target.value })}
            className="border rounded px-3 py-2"
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
      {loading ? (
        <p>Loading audit events...</p>
      ) : events.length === 0 ? (
        <p className="text-gray-500">No audit events found.</p>
      ) : (
        <div className="space-y-4">
          {events.map((event) => (
            <AuditEventCard key={event.eventId} event={event} />
          ))}
        </div>
      )}

      {/* Export Button */}
      <button
        onClick={() => console.log('Export functionality not yet implemented')}
        className="btn btn-outline mt-4"
      >
        Export to CSV
      </button>
    </div>
  )
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

export interface AuditFilter {
  eventType?: string
  startTime?: string
  endTime?: string
  actorId?: string
}

function AuditEventCard({ event }: { event: AuditEvent }) {
  return (
    <div className="card p-4 border rounded-lg hover:bg-gray-50 dark:hover:bg-gray-800">
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <span className="badge badge-sm badge-outline mb-2 inline-block">
            {event.eventType}
          </span>
          <p className="text-sm text-gray-600 dark:text-gray-400 mt-2">
            {JSON.stringify(event.payload, null, 2)}
          </p>
          <div className="flex items-center gap-2 mt-2 text-xs text-gray-500">
            <span>At {new Date(event.occurredAt).toLocaleString()}</span>
            {event.actorName && <span>by {event.actorName}</span>}
            {event.subjectId && <span>| Subject: {event.subjectType}/{event.subjectId.slice(0, 8)}...</span>}
          </div>
        </div>
      </div>
    </div>
  )
}
