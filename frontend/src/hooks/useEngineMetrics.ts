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

// Hook for real-time engine metrics via WebSocket
export function useEngineMetrics(url: string = 'ws://localhost:8080/metrics/ws') {
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
            // Request initial metrics on connect
            ws.send(JSON.stringify({ type: 'subscribe' }))
          }
        }

        ws.onmessage = (event) => {
          if (!isSubscribed) return

          try {
            const message: EngineState | { type: string } = JSON.parse(event.data)

            if ('engine_name' in message) {
              // Update metrics with received state
              const newState: MetricsData = {
                engineName: message.engine_name,
                metrics: [{
                  timestamp: message.last_updated,
                  rulesFired: message.rules_fired,
                  factsInserted: message.facts_inserted,
                  executionTimeMs: 0, // Not provided by current API
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
        const response = await fetch('/api/v1/metrics')
        if (response.ok) {
          const metrics: MetricsData[] = await response.json()
          if (isSubscribed) {
            setData(metrics)
            setError(null)
          }
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
      const response = await fetch('/api/v1/metrics')
      if (response.ok) {
        const metrics: MetricsData[] = await response.json()
        setData(metrics)
        setError(null)
      }
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
