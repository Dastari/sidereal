export type DatabaseAccountRecord = {
  accountId: string
  email: string
  primaryPlayerEntityId: string
  characterCount: number
  mfaTotpEnabled: boolean
  mfaVerifiedAtEpochS: number | null
  createdAtEpochS: number
  characters: Array<DatabaseCharacterRecord>
}

export type DatabaseCharacterRecord = {
  playerEntityId: string
  createdAtEpochS: number
  updatedAtEpochS: number
  displayName: string | null
  status: string
}

export type DatabaseTableRecord = {
  schemaName: string
  tableName: string
  tableType: string
  rowEstimate: number | null
}

export type ScriptDocumentRecord = {
  scriptPath: string
  family: string
  activeRevision: number | null
  hasDraft: boolean
}

export type DatabaseAdminPayload = {
  summary: {
    accountCount: number
    characterCount: number
    tableCount: number
    scriptDocumentCount: number
  }
  accounts: Array<DatabaseAccountRecord>
  tables: Array<DatabaseTableRecord>
  scriptDocuments: Array<ScriptDocumentRecord>
  error?: string
}
