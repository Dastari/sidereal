import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { getPostgresPool, safeGraphName } from '@/server/postgres'

type RenameBody = {
  displayName?: unknown
}

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

function looksLikeUuid(value: string): boolean {
  return UUID_REGEX.test(value.trim())
}

function escapeCypherString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/'/g, "\\'")
}

function isSafeIdentifier(value: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(value)
}

async function resolveCharactersQualifiedName(
  client: { query: (sql: string, params?: Array<unknown>) => Promise<{ rows: Array<any> }> },
): Promise<string | null> {
  const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
  for (const schemaName of [graphName, 'public']) {
    if (!isSafeIdentifier(schemaName)) continue
    const qualified = `${schemaName}.auth_characters`
    const result = await client.query(
      'SELECT to_regclass($1) IS NOT NULL AS present',
      [qualified],
    )
    if (result.rows[0]?.present === true) {
      return `"${schemaName}"."auth_characters"`
    }
  }
  return null
}

export const Route = createFileRoute(
  '/api/database/characters/$playerEntityId/display-name',
)({
  server: {
    handlers: {
      POST: async ({ request, params }) => {
        const playerEntityId = params.playerEntityId?.trim()
        if (!playerEntityId || !looksLikeUuid(playerEntityId)) {
          return json(
            { error: 'playerEntityId must be a UUID' },
            { status: 400 },
          )
        }

        let body: RenameBody
        try {
          body = (await request.json()) as RenameBody
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }
        if (typeof body.displayName !== 'string') {
          return json({ error: 'displayName is required' }, { status: 400 })
        }
        const displayName = body.displayName.trim()
        if (displayName.length < 2 || displayName.length > 64) {
          return json(
            { error: 'displayName must be between 2 and 64 characters' },
            { status: 400 },
          )
        }

        const pool = await getPostgresPool()
        const client = await pool.connect()

        try {
          const charactersQualifiedName = await resolveCharactersQualifiedName(
            client,
          )
          if (!charactersQualifiedName) {
            return json({ error: 'auth_characters table not found' }, { status: 404 })
          }
          const characterRow = await client.query(
            `
              SELECT 1
              FROM ${charactersQualifiedName}
              WHERE player_entity_id = $1
              LIMIT 1
            `,
            [playerEntityId],
          )
          if (characterRow.rows.length === 0) {
            return json({ error: 'character not found' }, { status: 404 })
          }

          const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
          await client.query("LOAD 'age'")
          await client.query('SET search_path = ag_catalog, public')

          const cypher = `
            MATCH (e:Entity {entity_id:'${escapeCypherString(playerEntityId)}'})
            MERGE (c:Component {component_id:'${escapeCypherString(playerEntityId)}:display_name'})
            SET c = {
              component_id:'${escapeCypherString(playerEntityId)}:display_name',
              component_kind:'display_name',
              display_name:'${escapeCypherString(displayName)}'
            }
            MERGE (e)-[:HAS_COMPONENT]->(c)
            RETURN c.display_name
          `
          const result = await client.query(
            `SELECT display_name::text AS display_name
             FROM ag_catalog.cypher('${escapeCypherString(graphName)}', $$${cypher}$$)
             AS (display_name agtype);`,
          )
          if (result.rows.length === 0) {
            return json(
              { error: 'character entity does not exist in graph' },
              { status: 404 },
            )
          }

          return json({
            playerEntityId,
            displayName,
          })
        } catch (error) {
          return json(
            {
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to rename character',
            },
            { status: 500 },
          )
        } finally {
          try {
            await client.query('SET search_path = public')
          } catch {
            // no-op
          }
          client.release()
        }
      },
    },
  },
  component: () => null,
})
