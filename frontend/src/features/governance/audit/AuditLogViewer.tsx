import { useState, useEffect, useCallback } from 'react'
import apiClient from '@/api/client'

// All fetch goes through the shared apiClient (baseURL = VITE_API_URL, default
// '/v1') — D-22-04/D-22-05. No hardcoded API base strings.
export function AuditLogViewer() {
  const [events, setEvents] = useState<AuditEvent[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [filter, setFilter] = useState<AuditFilter>({})
  const [page, setPage] = useState(1)
  const [totalCount, setTotalCount] = useState(0)
  const [totalPages, setTotalPages] = useState(0)
  const pageSize = 50

  // Check permissions - simplified
  const canManageRoles = true

  const fetchEvents = useCallback(
    async (filterToApply: AuditFilter, pageNum: number = 1) => {
      if (!canManageRoles) return
      try {
        setLoading(true)
        setError(null)

        // Map frontend filter fields → backend AuditQuery
        // (backend uses `action` not `eventType`, and `since` not `start_time`).
        const params: Record<string, string> = {
          page: String(pageNum),
          page_size: String(pageSize),
        }
        if (filterToApply.eventType) params.action = filterToApply.eventType
        if (filterToApply.resourceType) params.resource_type = filterToApply.resourceType
        if (filterToApply.actorId) params.user_id = filterToApply.actorId
        if (filterToApply.startTime) params.since = filterToApply.startTime

        const response = await apiClient.get<BackendAuditListResponse>('/audit', {
          params,
        })
        const body = response.data
        const mapped = (body.events || []).map(mapBackendAuditEvent)
        setEvents(mapped)
        setTotalCount(body.total_count ?? mapped.length)
        setTotalPages(body.total_pages ?? 0)
        setPage(body.page ?? pageNum)
      } catch (err) {
        console.error('Failed to fetch audit events:', err)
        setError(err instanceof Error ? err.message : 'Failed to load audit events')
        setEvents([])
      } finally {
        setLoading(false)
      }
    },
    [canManageRoles],
  )

  useEffect(() => {
    fetchEvents(filter, 1)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  function applyFilters() {
    setPage(1)
    fetchEvents(filter, 1)
  }

  function gotoPage(next: number) {
    if (next < 1 || (totalPages > 0 && next > totalPages)) return
    fetchEvents(filter, next)
  }

  async function exportLog(format: 'csv' | 'json') {
    try {
      setError(null)
      // Authenticated export via the shared apiClient (VITE_API_URL /v1).
      // Backend returns text/csv for csv and JSON for json.
      const response = await apiClient.get('/audit/export', {
        params: { format },
        responseType: 'blob',
      })
      const blob = response.data as Blob
      const url = window.URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `audit_log.${format}`
      document.body.appendChild(link)
      link.click()
      link.remove()
      window.URL.revokeObjectURL(url)
    } catch (err) {
      console.error(`Failed to export audit log as ${format}:`, err)
      setError(err instanceof Error ? err.message : `Failed to export as ${format}`)
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
    { value: 'create', label: 'Create' },
    { value: 'read', label: 'Read' },
    { value: 'update', label: 'Update' },
    { value: 'delete', label: 'Delete' },
    { value: 'approve', label: 'Approve' },
    { value: 'reject', label: 'Reject' },
    { value: 'execute', label: 'Execute' },
    { value: 'export', label: 'Export' },
  ]

  const resourceTypes = [
    { value: '', label: 'All Resource Types' },
    { value: 'rule', label: 'Rule' },
    { value: 'category', label: 'Category' },
    { value: 'tag', label: 'Tag' },
    { value: 'user', label: 'User' },
    { value: 'approval', label: 'Approval' },
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

          <select
            value={filter.resourceType || ''}
            onChange={(e) => setFilter({ ...filter, resourceType: e.target.value })}
            className="border rounded px-3 py-2"
          >
            {resourceTypes.map((opt) => (
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

          <input
            type="text"
            placeholder="User ID"
            value={filter.actorId || ''}
            onChange={(e) => setFilter({ ...filter, actorId: e.target.value })}
            className="border rounded px-3 py-2"
          />
        </div>

        <button onClick={applyFilters} className="btn btn-primary">
          Apply Filters
        </button>
      </div>

      {error && (
        <div className="mb-4 rounded-md bg-red-50 p-4 dark:bg-red-900/20">
          <p className="text-sm text-red-600 dark:text-red-400">{error}</p>
          <button
            onClick={() => fetchEvents(filter, page)}
            className="mt-2 btn btn-outline btn-sm"
          >
            Retry
          </button>
        </div>
      )}

      {/* Events List */}
      {loading ? (
        <p>Loading audit events...</p>
      ) : events.length === 0 ? (
        <p className="text-gray-500">No audit events found.</p>
      ) : (
        <>
          <div className="space-y-4">
            {events.map((event) => (
              <AuditEventCard key={event.eventId} event={event} />
            ))}
          </div>

          {/* Pagination */}
          {totalPages > 1 && (
            <div className="flex items-center justify-between mt-4">
              <span className="text-sm text-gray-500">
                {totalCount} events · page {page} of {totalPages}
              </span>
              <div className="space-x-2">
                <button
                  onClick={() => gotoPage(page - 1)}
                  disabled={page <= 1}
                  className="btn btn-outline btn-sm disabled:opacity-50"
                >
                  Previous
                </button>
                <button
                  onClick={() => gotoPage(page + 1)}
                  disabled={page >= totalPages}
                  className="btn btn-outline btn-sm disabled:opacity-50"
                >
                  Next
                </button>
              </div>
            </div>
          )}
        </>
      )}

      {/* Export Buttons */}
      <div className="mt-4 flex space-x-2">
        <button onClick={() => exportLog('csv')} className="btn btn-outline">
          Export to CSV
        </button>
        <button onClick={() => exportLog('json')} className="btn btn-outline">
          Export to JSON
        </button>
      </div>
    </div>
  )
}

/**
 * Frontend display shape for an audit event.
 */
export interface AuditEvent {
  eventId: string
  eventType: string
  actorId?: string
  actorName?: string
  subjectType: string
  subjectId?: string
  resourceName?: string
  occurredAt: string
  payload: Record<string, unknown>
}

export interface AuditFilter {
  eventType?: string
  resourceType?: string
  startTime?: string
  endTime?: string
  actorId?: string
}

/**
 * Backend AuditEvent as returned by GET /v1/audit (see api/src/v1/audit.rs).
 * Field names differ from the frontend display shape — Pitfall 4 contract
 * mismatch, resolved by `mapBackendAuditEvent` below.
 */
interface BackendAuditEvent {
  id: string
  timestamp: string
  user_id: Option<string | null>
  action: string
  resource_type: string
  resource_id: string
  resource_name: Option<string | null>
  details: Record<string, unknown> | null
  ip_address: Option<string | null>
  user_agent: Option<string | null>
}

interface BackendAuditListResponse {
  total_count: number
  events: BackendAuditEvent[]
  page: number
  page_size: number
  total_pages: number
}

type Option<T> = T | null | undefined

/**
 * Adapter mapping backend AuditEvent → frontend display shape (Pitfall 4).
 * Prevents `undefined` renders after the hardcoded-data→real swap (P-6 backstop).
 *
 * Mapping:
 *   id            → eventId
 *   action        → eventType
 *   user_id       → actorId (and actorName fallback — backend has no display name)
 *   resource_type → subjectType
 *   resource_id   → subjectId
 *   resource_name → resourceName
 *   timestamp     → occurredAt
 *   details       → payload
 */
function mapBackendAuditEvent(event: BackendAuditEvent): AuditEvent {
  return {
    eventId: event.id,
    eventType: event.action,
    actorId: event.user_id ?? undefined,
    actorName: event.user_id ?? undefined,
    subjectType: event.resource_type,
    subjectId: event.resource_id,
    resourceName: event.resource_name ?? undefined,
    occurredAt: event.timestamp,
    payload: event.details ?? {},
  }
}

function AuditEventCard({ event }: { event: AuditEvent }) {
  return (
    <div className="card p-4 border rounded-lg hover:bg-gray-50 dark:hover:bg-gray-800">
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <span className="badge badge-sm badge-outline mb-2 inline-block">
            {event.eventType}
          </span>
          {event.resourceName && (
            <p className="text-sm font-medium mt-1">{event.resourceName}</p>
          )}
          <p className="text-sm text-gray-600 dark:text-gray-400 mt-2">
            {JSON.stringify(event.payload, null, 2)}
          </p>
          <div className="flex items-center gap-2 mt-2 text-xs text-gray-500">
            <span>At {new Date(event.occurredAt).toLocaleString()}</span>
            {event.actorName && <span>by {event.actorName}</span>}
            {event.subjectId && (
              <span>| Subject: {event.subjectType}/{event.subjectId.slice(0, 8)}...</span>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}