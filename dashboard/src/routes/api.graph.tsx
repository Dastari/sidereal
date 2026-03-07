import { createFileRoute } from '@tanstack/react-router'
import { json } from '@tanstack/react-start'
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
    },
  },
  component: () => null,
})
