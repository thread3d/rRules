import { useState, useEffect, useCallback, useRef } from 'react'

// Types for engine metrics
export interface EngineState {
  engine_name: string
  rules_fired: number
  facts_inserted: number
  active_sessions: number
  memory_bytes: number
  last_updated: string
}

export interface MetricsData {
  engineName: string
  metrics: EngineMetricPoint[]
}

export interface EngineMetricPoint {
  timestamp: string
  rulesFired: number
  factsInserted: number
  executionTimeMs: number
}

// Plan 22-04 (D-22-04 / D-22-10): single source of truth for the API base URL.
// `VITE_API_URL` defaults to '/v1' (set in .env.development/.env.production by
// plan 22-01). When an absolute URL is configured (D-22-05 separate API host,
// e.g. 'https://api.rrules.example.com/v1'), the fetch + WS helpers below
// derive the right URL from it.
const API_URL = import.meta.env.VITE_API_URL || '/v1'

// The HTTP monitoring endpoint returns a SINGLE `EngineStateResponse` object
// (api/src/v1/routes/monitoring.rs), but this hook exposes a `MetricsData[]`
// array (one entry per engine, each with a time-series of points). This is the
// Pitfall 4 contract mismatch: the backend returns one object, the hook
// expected an array. `adaptEngineStateResponse` maps the single object into
// the array shape the UI consumes. Field names match the backend
// `#[derive(Serialize)]` struct (snake_case), NOT assumptions.
export interface EngineStateResponse {
  engine_name: string
  rules_fired: number
  facts_inserted: number
  active_sessions: number
  memory_bytes: number
  alpha_memory_size: number
  beta_memory_size: number
  network_traversals: number
  execution_time_ms: number
  last_updated: string
}

/** Map a single `EngineStateResponse` → the `MetricsData[]` shape the UI reads. */
function adaptEngineStateResponse(state: EngineStateResponse): MetricsData[] {
  return [
    {
      engineName: state.engine_name,
      metrics: [
        {
          timestamp: state.last_updated,
          rulesFired: state.rules_fired,
          factsInserted: state.facts_inserted,
          executionTimeMs: state.execution_time_ms,
        },
      ],
    },
  ]
}

/**
 * Build the WebSocket URL for the real-time metrics stream (Plan 22-04).
 *
 * When `API_URL` is a path prefix (the default '/v1'), the WS URL is
 * same-origin: `ws(s)://<location.host>/v1/metrics/ws`. When `API_URL` is an
 * absolute URL (D-22-05 separate API host), the http(s) scheme is converted to
 * ws(s) and the absolute host is used directly (no `location.host` prefix).
 * Replaces the stale `ws://localhost:8080/metrics/ws` default.
 */
function buildMetricsWsUrl(): string {
  if (API_URL.startsWith('http://') || API_URL.startsWith('https://')) {
    return `${API_URL.replace(/^http/, 'ws')}/metrics/ws`
  }
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${proto}//${window.location.host}${API_URL}/metrics/ws`
}

/**
 * Fetch the engine state from the HTTP monitoring endpoint and adapt it to the
 * `MetricsData[]` shape. Used both as the WS fallback and by `refresh`.
 * Targets `/v1/monitoring/engine-state` (Plan 22-04, D-22-10), NOT the stale
 * legacy `/api/...` path the prior hook used (which never matched a route).
 */
async function fetchEngineMetrics(): Promise<MetricsData[]> {
  const response = await fetch(`${API_URL}/monitoring/engine-state`)
  if (!response.ok) {
    throw new Error(`monitoring/engine-state returned ${response.status}`)
  }
  const state: EngineStateResponse = await response.json()
  return adaptEngineStateResponse(state)
}

// Hook for real-time engine metrics via WebSocket
export function useEngineMetrics(url: string = buildMetricsWsUrl()) {
  const [data, setData] = useState<MetricsData[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [isConnected, setIsConnected] = useState(false)
  const wsRef = useRef<WebSocket | null>(null)

  useEffect(() => {
    // Try to connect via WebSocket for real-time updates
    let isSubscribed = true

    const connectWebSocket = () => {
      try {
        const ws = new WebSocket(url)

        ws.onopen = () => {
          if (isSubscribed) {
            setIsConnected(true)
            setError(null)
            // The WS stream is read-only (server broadcasts EngineState); no
            // subscribe message is required, but the prior behavior requested
            // initial metrics — kept for back-compat with any server that
            // echoes one. Also clear `loading` once the connection is live so
            // the UI is not stuck "loading" on a successful WS connection.
            ws.send(JSON.stringify({ type: 'subscribe' }))
            setLoading(false)
          }
        }

        ws.onmessage = (event) => {
          if (!isSubscribed) return

          try {
            const message: EngineState | { type: string } = JSON.parse(event.data)

            if ('engine_name' in message) {
              // Update metrics with received state. The broadcast
              // `metrics::EngineState` (api/src/metrics.rs) does NOT carry
              // `execution_time_ms`, so it maps to 0 here; the HTTP path
              // (`adaptEngineStateResponse`) surfaces the real value.
              const newState: MetricsData = {
                engineName: message.engine_name,
                metrics: [{
                  timestamp: message.last_updated,
                  rulesFired: message.rules_fired,
                  factsInserted: message.facts_inserted,
                  executionTimeMs: 0, // Not provided by the WS broadcast
                }],
              }

              setData((prev) => {
                const existing = prev.find(m => m.engineName === newState.engineName)
                if (existing) {
                  return prev.map(m =>
                    m.engineName === newState.engineName ? newState : m
                  )
                }
                return [...prev, newState]
              })
            }
          } catch (e) {
            console.error('Failed to parse WebSocket message:', e)
          }
        }

        ws.onclose = () => {
          if (isSubscribed) {
            setIsConnected(false)
            // Reconnect after delay
            setTimeout(connectWebSocket, 2000)
          }
        }

        ws.onerror = (event) => {
          if (isSubscribed) {
            setError('WebSocket connection error')
            console.error('WebSocket error:', event)
          }
        }

        wsRef.current = ws
      } catch (err) {
        if (isSubscribed) {
          setError(`Failed to connect: ${err instanceof Error ? err.message : 'Unknown error'}`)
          // Fallback to HTTP polling if WebSocket fails
          startHttpPolling()
        }
      }
    }

    const startHttpPolling = async () => {
      try {
        const metrics = await fetchEngineMetrics()
        if (isSubscribed) {
          setData(metrics)
          setError(null)
        }
      } catch (error) {
        console.error('Failed to fetch metrics:', error)
      } finally {
        if (isSubscribed) {
          setLoading(false)
        }
      }
    }

    connectWebSocket()

    return () => {
      isSubscribed = false
      if (wsRef.current) {
        wsRef.current.close()
      }
    }
  }, [url])

  // Manual refresh function
  const refresh = useCallback(async () => {
    try {
      setLoading(true)
      const metrics = await fetchEngineMetrics()
      setData(metrics)
      setError(null)
    } catch (error) {
      console.error('Failed to refresh metrics:', error)
      setError('Failed to refresh metrics')
    } finally {
      setLoading(false)
    }
  }, [])

  return {
    data,
    loading,
    error,
    isConnected,
    refresh,
  }
}