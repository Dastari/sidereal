import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
import { graphComponentUpdateSchema } from '@/lib/schemas/dashboard'
import { requireDashboardAdmin } from '@/server/dashboard-auth'
import { getPostgresPool, safeGraphName } from '@/server/postgres'

type GraphNode = {
  id: string
  label: string
  kind: string
  properties: Record<string, unknown>
}

type GraphEdge = {
  id: string
  from: string
  to: string
  label: string
  properties: Record<string, unknown>
}

type GraphPayload = {
  graph: string
  nodes: Array<GraphNode>
  edges: Array<GraphEdge>
}

function labelName(name: string): string {
  const parts = String(name).split('.')
  return parts[parts.length - 1] || name
}

function parseAgtype(raw: unknown): any {
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

function titleFromSnakeCase(value: string): string {
  return value
    .split('_')
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ')
}

function escapeCypherString(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/'/g, "''")
}

function toCypherLiteral(value: unknown): string {
  if (value === null || value === undefined) return 'null'
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new Error(
        'Non-finite numbers are not supported in component values',
      )
    }
    return String(value)
  }
  if (typeof value === 'string') {
    return `'${escapeCypherString(value)}'`
  }
  if (Array.isArray(value)) {
    return `[${value.map((entry) => toCypherLiteral(entry)).join(', ')}]`
  }
  if (typeof value === 'object') {
    const record = value as Record<string, unknown>
    const entries = Object.entries(record).map(([key, entryValue]) => {
      const cypherKey = /^[A-Za-z_][A-Za-z0-9_]*$/.test(key)
        ? key
        : `\`${key.replace(/`/g, '``')}\``
      return `${cypherKey}: ${toCypherLiteral(entryValue)}`
    })
    return `{${entries.join(', ')}}`
  }
  throw new Error(`Unsupported component payload value type: ${typeof value}`)
}

function sanitizePayloadKey(typePath: string): string {
  return typePath.replaceAll('::', '__')
}

export const Route = createFileRoute('/api/graph')({
  server: {
    handlers: {
      GET: async () => {
        const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
        const pool = await getPostgresPool()
        const client = await pool.connect()

        try {
          await client.query("LOAD 'age'")
          await client.query('SET search_path = ag_catalog, public')

          const nodeMap = new Map<string, GraphNode>()
          const nodeByGraphId = new Map<string, string>()
          const edges: Array<GraphEdge> = []
          const nodeRows = await client.query(
            `SELECT n_id::text AS n_id, labels::text AS labels, props::text AS props
             FROM ag_catalog.cypher('${graphName}', $$
               MATCH (n)
               RETURN id(n), labels(n), properties(n)
             $$) AS (n_id agtype, labels agtype, props agtype);`,
          )
          for (const row of nodeRows.rows) {
            const graphId = parseAgtype(row.n_id)
            const labels = parseAgtype(row.labels)
            const props = (parseAgtype(row.props) ?? {}) as Record<
              string,
              unknown
            >
            const kind =
              Array.isArray(labels) && labels.length > 0
                ? String(labels[0])
                : 'Node'
            const stableId = String(props.entity_id ?? `${kind}:${graphId}`)
            const componentKind =
              typeof props.component_kind === 'string'
                ? titleFromSnakeCase(props.component_kind)
                : null
            const labelCandidate =
              props.name ||
              componentKind ||
              props.component_type ||
              props.kind ||
              props.entity_id ||
              `${kind} ${graphId}`
            nodeMap.set(stableId, {
              id: stableId,
              label: String(labelCandidate),
              kind: labelName(kind),
              properties: props,
            })
            nodeByGraphId.set(String(graphId), stableId)
          }

          const edgeRows = await client.query(
            `SELECT e_id::text AS e_id, from_id::text AS from_id, to_id::text AS to_id, rel_type::text AS rel_type, props::text AS props
             FROM ag_catalog.cypher('${graphName}', $$
               MATCH (a)-[r]->(b)
               RETURN id(r), id(a), id(b), type(r), properties(r)
             $$) AS (e_id agtype, from_id agtype, to_id agtype, rel_type agtype, props agtype);`,
          )
          for (const row of edgeRows.rows) {
            const from = nodeByGraphId.get(String(parseAgtype(row.from_id)))
            const to = nodeByGraphId.get(String(parseAgtype(row.to_id)))
            if (!from || !to) continue
            const edgeId = String(parseAgtype(row.e_id))
            const relType = String(parseAgtype(row.rel_type) ?? 'REL')
            const props = (parseAgtype(row.props) ?? {}) as Record<
              string,
              unknown
            >
            edges.push({
              id: `${relType}:${edgeId}`,
              from,
              to,
              label: relType,
              properties: props,
            })
          }

          const payload: GraphPayload = {
            graph: graphName,
            nodes: Array.from(nodeMap.values()),
            edges,
          }

          return json(payload)
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown error'
          return json({ error: message }, { status: 500 })
        } finally {
          client.release()
        }
      },
      POST: async ({ request }) => {
        const authFailure = requireDashboardAdmin(request)
        if (authFailure) {
          return authFailure
        }

        let body: unknown
        try {
          body = await request.json()
        } catch {
          return json({ error: 'Invalid JSON body' }, { status: 400 })
        }
        const parsedBody = graphComponentUpdateSchema.safeParse(body)
        if (!parsedBody.success) {
          return json(
            {
              error:
                parsedBody.error.issues[0]?.message ?? 'Invalid request body',
            },
            { status: 400 },
          )
        }
        const { componentKind, entityId, typePath, value } = parsedBody.data

        const graphName = safeGraphName(process.env.GRAPH_NAME || 'sidereal')
        const pool = await getPostgresPool()
        const client = await pool.connect()

        try {
          await client.query("LOAD 'age'")
          await client.query('SET search_path = ag_catalog, public')

          const payloadKey = sanitizePayloadKey(typePath)
          const payloadLiteral = toCypherLiteral(value)
          const escapedEntityId = escapeCypherString(entityId)
          const escapedComponentKind = escapeCypherString(componentKind)

          const result = await client.query(
            `SELECT component_id::text AS component_id
             FROM ag_catalog.cypher('${escapeCypherString(graphName)}', $$
               MATCH (e:Entity {entity_id:'${escapedEntityId}'})-[:HAS_COMPONENT]->(c:Component {component_kind:'${escapedComponentKind}'})
               SET c.${payloadKey} = ${payloadLiteral}
               RETURN c.component_id
             $$) AS (component_id agtype);`,
          )

          if (result.rows.length === 0) {
            return json(
              {
                error: `Component not found for entity ${entityId} and kind ${componentKind}`,
              },
              { status: 404 },
            )
          }

          return json({
            success: true,
            entityId,
            componentKind,
            typePath,
            payloadKey,
          })
        } catch (error) {
          const message =
            error instanceof Error ? error.message : 'Unknown database error'
          return json({ error: message }, { status: 500 })
        } finally {
          client.release()
        }
      },
    },
  },
  component: () => null,
})
