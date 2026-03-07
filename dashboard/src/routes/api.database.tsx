import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import type { DatabaseAdminPayload } from '@/features/database/types'
import { getPostgresPool } from '@/server/postgres'

async function tableExists(
  client: Awaited<ReturnType<ReturnType<typeof getPostgresPool>['connect']>>,
  tableName: string,
) {
  const result = await client.query('SELECT to_regclass($1) IS NOT NULL AS present', [
    tableName,
  ])
  return result.rows[0]?.present === true
}

export const Route = createFileRoute('/api/database')({
  server: {
    handlers: {
      GET: async () => {
        const pool = await getPostgresPool()
        const client = await pool.connect()

        try {
          const hasAccountsTable = await tableExists(client, 'public.auth_accounts')
          const hasCharactersTable = await tableExists(client, 'public.auth_characters')
          const hasScriptDocumentsTable = await tableExists(
            client,
            'public.script_catalog_documents',
          )
          const hasScriptDraftsTable = await tableExists(
            client,
            'public.script_catalog_drafts',
          )

          const accounts = hasAccountsTable
            ? (
                await client.query(
                  `
                    SELECT
                      a.account_id::text AS account_id,
                      a.email,
                      a.player_entity_id,
                      a.created_at_epoch_s,
                      COALESCE(characters.character_count, 0) AS character_count
                    FROM auth_accounts a
                    LEFT JOIN (
                      SELECT account_id, COUNT(*)::int AS character_count
                      FROM auth_characters
                      GROUP BY account_id
                    ) characters ON characters.account_id = a.account_id
                    ORDER BY a.email ASC
                  `,
                )
              ).rows.map((row) => ({
                accountId: String(row.account_id),
                email: String(row.email),
                primaryPlayerEntityId: String(row.player_entity_id),
                characterCount: Number(row.character_count ?? 0),
                createdAtEpochS: Number(row.created_at_epoch_s ?? 0),
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
                LEFT JOIN pg_class c
                  ON c.relname = t.table_name
                LEFT JOIN pg_namespace n
                  ON n.oid = c.relnamespace
                  AND n.nspname = t.table_schema
                LEFT JOIN pg_stat_user_tables s
                  ON s.relname = t.table_name
                  AND s.schemaname = t.table_schema
                WHERE t.table_schema NOT IN ('pg_catalog', 'information_schema')
                ORDER BY t.table_schema ASC, t.table_name ASC
              `,
            )
          ).rows.map((row) => ({
            schemaName: String(row.table_schema),
            tableName: String(row.table_name),
            tableType: String(row.table_type),
            rowEstimate:
              row.row_estimate == null ? null : Number(row.row_estimate),
          }))

          const scriptDocuments = hasScriptDocumentsTable
            ? (
                await client.query(
                  `
                    SELECT
                      script_path,
                      script_family AS family,
                      active_revision,
                      ${
                        hasScriptDraftsTable
                          ? `EXISTS (
                              SELECT 1
                              FROM script_catalog_drafts drafts
                              WHERE drafts.script_path = documents.script_path
                            )`
                          : 'FALSE'
                      } AS has_draft
                    FROM script_catalog_documents documents
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

          const payload: DatabaseAdminPayload = {
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

          return json(payload)
        } catch (error) {
          return json(
            {
              summary: {
                accountCount: 0,
                characterCount: 0,
                tableCount: 0,
                scriptDocumentCount: 0,
              },
              accounts: [],
              tables: [],
              scriptDocuments: [],
              error:
                error instanceof Error ? error.message : 'Unknown database error',
            } satisfies DatabaseAdminPayload,
            { status: 500 },
          )
        } finally {
          client.release()
        }
      },
    },
  },
  component: () => null,
})
