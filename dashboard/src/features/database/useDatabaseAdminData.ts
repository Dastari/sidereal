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

  const requestPasswordReset = useCallback(async (accountId: string) => {
    const response = await fetch(
      `/api/database/accounts/${encodeURIComponent(accountId)}/password-reset`,
      { method: 'POST' },
    )
    const payload = (await response.json().catch(() => ({}))) as {
      accepted?: unknown
      resetToken?: unknown
      error?: unknown
    }
    if (!response.ok) {
      throw new Error(
        typeof payload.error === 'string'
          ? payload.error
          : 'Failed to request password reset',
      )
    }
    return {
      accepted: payload.accepted === true,
      resetToken:
        typeof payload.resetToken === 'string' ? payload.resetToken : null,
    }
  }, [])

  const renameCharacter = useCallback(
    async (playerEntityId: string, displayName: string) => {
      const response = await fetch(
        `/api/database/characters/${encodeURIComponent(playerEntityId)}/display-name`,
        {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ displayName }),
        },
      )
      const payload = (await response.json().catch(() => ({}))) as {
        error?: unknown
      }
      if (!response.ok) {
        throw new Error(
          typeof payload.error === 'string'
            ? payload.error
            : 'Failed to rename character',
        )
      }

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

  useEffect(() => {
    void refresh()
  }, [refresh])

  return {
    data,
    loading,
    error,
    refresh,
    requestPasswordReset,
    renameCharacter,
  }
}
