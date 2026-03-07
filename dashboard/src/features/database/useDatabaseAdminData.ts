import { useCallback, useEffect, useState } from 'react'
import type { DatabaseAdminPayload } from '@/features/database/types'

const EMPTY_PAYLOAD: DatabaseAdminPayload = {
  summary: {
    accountCount: 0,
    characterCount: 0,
    tableCount: 0,
    scriptDocumentCount: 0,
  },
  accounts: [],
  tables: [],
  scriptDocuments: [],
}

export function useDatabaseAdminData() {
  const [data, setData] = useState<DatabaseAdminPayload>(EMPTY_PAYLOAD)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const response = await fetch('/api/database')
      const payload = (await response.json()) as DatabaseAdminPayload
      if (!response.ok || payload.error) {
        throw new Error(payload.error ?? 'Failed to load database admin data')
      }
      setData(payload)
    } catch (fetchError) {
      setError(
        fetchError instanceof Error
          ? fetchError.message
          : 'Failed to load database admin data',
      )
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh])

  return {
    data,
    loading,
    error,
    refresh,
  }
}
