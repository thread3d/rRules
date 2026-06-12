import { useState, useEffect } from 'react'
import { getRules } from '@/api/rules'
import type { Rule } from '@/types/common'

export function useRules() {
  const [rules, setRules] = useState<Rule[]>([])
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetchRules()
  }, [])

  async function fetchRules() {
    try {
      setIsLoading(true)
      const fetchedRules = await getRules()
      setRules(fetchedRules)
      setError(null)
    } catch (err) {
      console.error('Failed to fetch rules:', err)
      setError(err instanceof Error ? err.message : 'Unknown error')
    } finally {
      setIsLoading(false)
    }
  }

  return { rules, isLoading, error, onRefresh: fetchRules }
}
