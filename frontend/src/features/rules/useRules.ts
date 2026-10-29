import { useState, useEffect } from 'react'
import { getRules } from '@/api/rules'
import type { Rule } from '@/types/common'

export function useRules() {
  const [rules, setRules] = useState<Rule[]>([])
  const [page, setPage] = useState<{ totalCount: number; page: number; pageSize: number; totalPages: number }>({
    totalCount: 0,
    page: 1,
    pageSize: 0,
    totalPages: 0,
  })
  const [requestedPage, setRequestedPage] = useState(1)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    fetchRules()
    // `requestedPage` is a dependency so paging refetches; the server owns paging, the client must not
    // pretend a local slice of the first page is the whole set.
  }, [requestedPage])

  async function fetchRules() {
    try {
      setIsLoading(true)
      const fetched = await getRules({ page: requestedPage })
      setRules(fetched.rules)
      setPage({
        totalCount: fetched.totalCount,
        page: fetched.page,
        pageSize: fetched.pageSize,
        totalPages: fetched.totalPages,
      })
      setError(null)
    } catch (err) {
      console.error('Failed to fetch rules:', err)
      setError(err instanceof Error ? err.message : 'Unknown error')
    } finally {
      setIsLoading(false)
    }
  }

  return {
    rules,
    isLoading,
    error,
    onRefresh: fetchRules,
    totalCount: page.totalCount,
    page: page.page,
    totalPages: page.totalPages,
    goToPage: setRequestedPage,
  }
}
