import { useCallback, useEffect, useState } from 'react'
import type { DatabaseAdminPayload } from '@/features/database/types'
import { apiPost } from '@/lib/api/client'
import { loadDatabaseAdminData } from '@/lib/server-fns/database-admin'

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

type PasswordResetResponse = {
  accepted: boolean
}

export function useDatabaseAdminData(
  initialData: DatabaseAdminPayload = EMPTY_PAYLOAD,
) {
  const [data, setData] = useState<DatabaseAdminPayload>(initialData)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setData(initialData)
    setLoading(false)
    setError(initialData.error ?? null)
  }, [initialData])

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const payload = await loadDatabaseAdminData()
      if (payload.error) {
        throw new Error(payload.error)
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

  const requestPasswordReset = useCallback(async (accountId: string) => {
    return apiPost<PasswordResetResponse>(
      `/api/database/accounts/${encodeURIComponent(accountId)}/password-reset`,
    )
  }, [])

  const renameCharacter = useCallback(
    async (playerEntityId: string, displayName: string) => {
      await apiPost<{ playerEntityId: string; displayName: string }>(
        `/api/database/characters/${encodeURIComponent(playerEntityId)}/display-name`,
        { displayName },
      )

      setData((current) => ({
        ...current,
        accounts: current.accounts.map((account) => ({
          ...account,
          characters: account.characters.map((character) =>
            character.playerEntityId === playerEntityId
              ? { ...character, displayName }
              : character,
          ),
        })),
      }))
    },
    [],
  )

  return {
    data,
    loading,
    error,
    refresh,
    requestPasswordReset,
    renameCharacter,
  }
}
