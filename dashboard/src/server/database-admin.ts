import type { DatabaseAdminPayload } from '@/features/database/types'
import type { PgClient } from '@/server/postgres'
import { getPostgresPool, safeGraphName } from '@/server/postgres'

type ResolvedTableRef = {
  qualifiedName: string
}

const EMPTY_DATABASE_ADMIN_PAYLOAD: DatabaseAdminPayload = {
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

function isSafeIdentifier(value: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(value)
}

async function resolveTableRef(
  client: PgClient,
  tableName: string,
  schemaCandidates: Array<string>,
): Promise<ResolvedTableRef | null> {
  if (!isSafeIdentifier(tableName)) {
    return null
  }
  for (const schemaName of schemaCandidates) {
    if (!isSafeIdentifier(schemaName)) continue
    const qualified = `${schemaName}.${tableName}`
    const result = await client.query(
      'SELECT to_regclass($1) IS NOT NULL AS present',
      [qualified],
    )
    if (result.rows[0]?.present === true) {
      return {
        qualifiedName: `"${schemaName}"."${tableName}"`,
      }
    }
  }
  return null
}

function parseAgtype(raw: unknown): unknown {
  if (raw === null || raw === undefined) return null
  const text = String(raw).trim()
  const stripped = text.replace(/::[A-Za-z_][A-Za-z0-9_]*$/, '').trim()
  if (!stripped || stripped === 'null') return null
  try {
    return JSON.parse(stripped)
  } catch {
    return stripped.replace(/^"(.*)"$/, '$1')
  }
}

function escapeCypherString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/'/g, "\\'")
}

export async function loadDatabaseAdminPayload(): Promise<DatabaseAdminPayload> {
  const pool = await getPostgresPool()
  const client = await pool.connect()

  try {
    const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
    const schemaCandidates = [graphName, 'public']
    const accountsTableRef = await resolveTableRef(
      client,
      'auth_accounts',
      schemaCandidates,
    )
    const charactersTableRef = await resolveTableRef(
      client,
      'auth_characters',
      schemaCandidates,
    )
    const totpSecretsTableRef = await resolveTableRef(
      client,
      'auth_totp_secrets',
      schemaCandidates,
    )
    const scriptDocumentsTableRef = await resolveTableRef(
      client,
      'script_catalog_documents',
      schemaCandidates,
    )
    const scriptDraftsTableRef = await resolveTableRef(
      client,
      'script_catalog_drafts',
      schemaCandidates,
    )
    const hasCharactersTable = charactersTableRef !== null

    const characterDisplayNameByEntityId = new Map<string, string>()
    try {
      await client.query("LOAD 'age'")
      await client.query('SET search_path = ag_catalog, public')
      const displayNameRows = await client.query(
        `SELECT entity_id::text AS entity_id, display_name::text AS display_name
         FROM ag_catalog.cypher('${escapeCypherString(graphName)}', $$
           MATCH (e:Entity)-[:HAS_COMPONENT]->(c:Component {component_kind:'display_name'})
           RETURN e.entity_id, c.display_name
         $$) AS (entity_id agtype, display_name agtype);`,
      )
      for (const row of displayNameRows.rows) {
        const entityId = parseAgtype(row.entity_id)
        const displayName = parseAgtype(row.display_name)
        if (
          typeof entityId === 'string' &&
          entityId.length > 0 &&
          typeof displayName === 'string' &&
          displayName.length > 0
        ) {
          characterDisplayNameByEntityId.set(entityId, displayName)
        }
      }
    } catch {
      // Character names are optional in admin UI; keep payload available without AGE.
    } finally {
      try {
        await client.query('SET search_path = public')
      } catch {
        // no-op
      }
    }

    const charactersByAccountId = new Map<
      string,
      Array<{
        playerEntityId: string
        createdAtEpochS: number
        updatedAtEpochS: number
        displayName: string | null
        status: string
      }>
    >()
    if (charactersTableRef) {
      const characterRows = await client.query(
        `
          SELECT
            account_id::text AS account_id,
            player_entity_id,
            display_name,
            status,
            created_at_epoch_s,
            updated_at_epoch_s
          FROM ${charactersTableRef.qualifiedName}
          ORDER BY created_at_epoch_s DESC, display_name ASC, player_entity_id ASC
        `,
      )
      for (const row of characterRows.rows) {
        const accountId = String(row.account_id)
        const playerEntityId = String(row.player_entity_id)
        const authDisplayName = String(row.display_name ?? '').trim()
        const accountCharacters = charactersByAccountId.get(accountId) ?? []
        accountCharacters.push({
          playerEntityId,
          createdAtEpochS: Number(row.created_at_epoch_s ?? 0),
          updatedAtEpochS: Number(row.updated_at_epoch_s ?? 0),
          displayName:
            authDisplayName ||
            characterDisplayNameByEntityId.get(playerEntityId) ||
            null,
          status: String(row.status ?? 'active'),
        })
        charactersByAccountId.set(accountId, accountCharacters)
      }
    }

    const accounts = accountsTableRef
      ? (
          await client.query(
            `
              SELECT
                a.account_id::text AS account_id,
                a.email,
                a.player_entity_id,
                a.created_at_epoch_s,
                COALESCE(characters.character_count, 0) AS character_count,
                mfa.verified_at_epoch_s AS mfa_verified_at_epoch_s
              FROM ${accountsTableRef.qualifiedName} a
              ${
                charactersTableRef
                  ? `LEFT JOIN (
                       SELECT account_id, COUNT(*)::int AS character_count
                       FROM ${charactersTableRef.qualifiedName}
                       GROUP BY account_id
                     ) characters ON characters.account_id = a.account_id`
                  : 'LEFT JOIN (SELECT NULL::uuid AS account_id, 0::int AS character_count) characters ON FALSE'
              }
              ${
                totpSecretsTableRef
                  ? `LEFT JOIN (
                       SELECT account_id, MAX(verified_at_epoch_s)::bigint AS verified_at_epoch_s
                       FROM ${totpSecretsTableRef.qualifiedName}
                       WHERE disabled_at_epoch_s IS NULL
                       GROUP BY account_id
                     ) mfa ON mfa.account_id = a.account_id`
                  : 'LEFT JOIN (SELECT NULL::uuid AS account_id, NULL::bigint AS verified_at_epoch_s) mfa ON FALSE'
              }
              ORDER BY a.email ASC
            `,
          )
        ).rows.map((row) => ({
          accountId: String(row.account_id),
          email: String(row.email),
          primaryPlayerEntityId: String(row.player_entity_id),
          characterCount: Number(row.character_count ?? 0),
          mfaTotpEnabled: row.mfa_verified_at_epoch_s != null,
          mfaVerifiedAtEpochS:
            row.mfa_verified_at_epoch_s == null
              ? null
              : Number(row.mfa_verified_at_epoch_s),
          createdAtEpochS: Number(row.created_at_epoch_s ?? 0),
          characters: charactersByAccountId.get(String(row.account_id)) ?? [],
        }))
      : []

    const tables = (
      await client.query(
        `
          SELECT
            t.table_schema,
            t.table_name,
            t.table_type,
            COALESCE(s.n_live_tup::bigint, c.reltuples::bigint, NULL) AS row_estimate
          FROM information_schema.tables t
          LEFT JOIN pg_namespace n
            ON n.nspname = t.table_schema
          LEFT JOIN pg_class c
            ON c.relname = t.table_name
            AND c.relnamespace = n.oid
          LEFT JOIN pg_stat_user_tables s
            ON s.relid = c.oid
          WHERE t.table_schema NOT IN ('pg_catalog', 'information_schema')
          ORDER BY t.table_schema ASC, t.table_name ASC
        `,
      )
    ).rows.map((row) => ({
      schemaName: String(row.table_schema),
      tableName: String(row.table_name),
      tableType: String(row.table_type),
      rowEstimate: row.row_estimate == null ? null : Number(row.row_estimate),
    }))

    const scriptDocuments = scriptDocumentsTableRef
      ? (
          await client.query(
            `
              SELECT
                script_path,
                script_family AS family,
                active_revision,
                ${
                  scriptDraftsTableRef
                    ? `EXISTS (
                         SELECT 1
                         FROM ${scriptDraftsTableRef.qualifiedName} drafts
                         WHERE drafts.script_path = documents.script_path
                       )`
                    : 'FALSE'
                } AS has_draft
              FROM ${scriptDocumentsTableRef.qualifiedName} documents
              ORDER BY script_path ASC
            `,
          )
        ).rows.map((row) => ({
          scriptPath: String(row.script_path),
          family: String(row.family),
          activeRevision:
            row.active_revision == null ? null : Number(row.active_revision),
          hasDraft: row.has_draft === true,
        }))
      : []

    return {
      summary: {
        accountCount: accounts.length,
        characterCount: hasCharactersTable
          ? accounts.reduce(
              (total, account) => total + account.characterCount,
              0,
            )
          : 0,
        tableCount: tables.length,
        scriptDocumentCount: scriptDocuments.length,
      },
      accounts,
      tables,
      scriptDocuments,
    }
  } finally {
    client.release()
  }
}

export function createDatabaseAdminErrorPayload(
  error: unknown,
): DatabaseAdminPayload {
  return {
    ...EMPTY_DATABASE_ADMIN_PAYLOAD,
    error:
      error instanceof Error
        ? error.message
        : 'Failed to load database admin data',
  }
}
