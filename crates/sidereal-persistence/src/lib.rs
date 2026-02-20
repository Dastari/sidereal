use postgres::{Client, NoTls};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};
use sidereal_net::WorldDeltaEntity;
use std::collections::HashMap;
use thiserror::Error;

const DEFAULT_GRAPH_NAME: &str = "sidereal";

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, PersistenceError>;

pub fn encode_reflect_component(type_path: &str, component_value: JsonValue) -> JsonValue {
    let mut envelope = JsonMap::new();
    envelope.insert(type_path.to_string(), component_value);
    JsonValue::Object(envelope)
}

pub fn decode_reflect_component<'a>(
    payload: &'a JsonValue,
    expected_type_path: &str,
) -> Option<&'a JsonValue> {
    payload.as_object()?.get(expected_type_path)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphComponentRecord {
    pub component_id: String,
    pub component_kind: String,
    pub properties: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphEntityRecord {
    pub entity_id: String,
    pub labels: Vec<String>,
    pub properties: JsonValue,
    pub components: Vec<GraphComponentRecord>,
}

pub struct GraphPersistence {
    client: Client,
    graph_name: String,
}

impl GraphPersistence {
    pub fn connect(database_url: &str) -> Result<Self> {
        Self::connect_with_graph(database_url, DEFAULT_GRAPH_NAME)
    }

    pub fn connect_with_graph(database_url: &str, graph_name: impl Into<String>) -> Result<Self> {
        let client = Client::connect(database_url, NoTls)
            .map_err(|err| PersistenceError::Database(format!("postgres connect failed: {err}")))?;
        Ok(Self {
            client,
            graph_name: graph_name.into(),
        })
    }

    pub fn graph_name(&self) -> &str {
        &self.graph_name
    }

    pub fn ensure_schema(&mut self) -> Result<()> {
        self.client
            .batch_execute("CREATE EXTENSION IF NOT EXISTS age;")
            .map_err(db_err("create age extension"))?;
        self.client
            .batch_execute("LOAD 'age';")
            .map_err(db_err("load age extension"))?;
        self.client
            .batch_execute("SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("set age search_path"))?;

        let graph_exists = self
            .client
            .query_opt(
                "SELECT 1 FROM ag_catalog.ag_graph WHERE name = $1 LIMIT 1",
                &[&self.graph_name],
            )
            .map_err(db_err("query graph existence"))?
            .is_some();
        if !graph_exists {
            let query = format!(
                "SELECT * FROM ag_catalog.create_graph('{}');",
                escape_cypher_string(&self.graph_name)
            );
            self.client
                .batch_execute(&query)
                .map_err(db_err("create graph"))?;
        }

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path"))?;

        self.client
            .batch_execute(
                "
                CREATE TABLE IF NOT EXISTS replication_snapshot_markers (
                    snapshot_id BIGSERIAL PRIMARY KEY,
                    snapshot_tick BIGINT NOT NULL,
                    entity_count BIGINT NOT NULL,
                    created_at_epoch_s BIGINT NOT NULL
                );
                ",
            )
            .map_err(db_err("create snapshot marker table"))?;

        Ok(())
    }

    pub fn persist_world_delta(&mut self, updates: &[WorldDeltaEntity], tick: u64) -> Result<()> {
        let removed_entity_ids = updates
            .iter()
            .filter(|u| u.removed)
            .map(|u| u.entity_id.clone())
            .collect::<Vec<_>>();

        let records = updates
            .iter()
            .filter(|u| !u.removed)
            .map(|u| GraphEntityRecord {
                entity_id: u.entity_id.clone(),
                labels: if u.labels.is_empty() {
                    vec!["Entity".to_string()]
                } else {
                    u.labels.clone()
                },
                properties: u.properties.clone(),
                components: u
                    .components
                    .iter()
                    .map(|c| GraphComponentRecord {
                        component_id: c.component_id.clone(),
                        component_kind: c.component_kind.clone(),
                        properties: c.properties.clone(),
                    })
                    .collect::<Vec<_>>(),
            })
            .collect::<Vec<_>>();

        self.persist_graph_records(&records, tick)?;
        self.remove_graph_entities(&removed_entity_ids)?;
        Ok(())
    }

    pub fn persist_graph_records(
        &mut self,
        records: &[GraphEntityRecord],
        tick: u64,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph persist"))?;

        for record in records {
            let labels = sanitize_labels(&record.labels);
            let mut set_parts = vec![format!("e.last_tick={tick}")];
            set_parts.push(format!(
                "e.sidereal_labels={}",
                cypher_literal(&JsonValue::Array(
                    labels
                        .iter()
                        .cloned()
                        .map(JsonValue::String)
                        .collect::<Vec<_>>()
                ))
            ));
            set_parts.extend(cypher_set_clauses("e", &record.properties));

            let query = format!(
                "MERGE (e:Entity {{entity_id:'{}'}}) SET {}",
                escape_cypher_string(&record.entity_id),
                set_parts.join(", "),
            );
            self.run_cypher(&query)?;

            let incoming_component_ids = JsonValue::Array(
                record
                    .components
                    .iter()
                    .map(|c| JsonValue::String(c.component_id.clone()))
                    .collect::<Vec<_>>(),
            );
            self.run_cypher(&format!(
                "MATCH (e:Entity {{entity_id:'{}'}}) \
                 OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) \
                 WHERE c IS NOT NULL AND NOT c.component_id IN {} \
                 DETACH DELETE c",
                escape_cypher_string(&record.entity_id),
                cypher_literal(&incoming_component_ids),
            ))?;

            for component in &record.components {
                let mut comp_set = vec![
                    format!("c.last_tick={tick}"),
                    format!(
                        "c.component_id={}",
                        cypher_literal(&JsonValue::String(component.component_id.clone()))
                    ),
                    format!(
                        "c.component_kind={}",
                        cypher_literal(&JsonValue::String(component.component_kind.clone()))
                    ),
                ];
                comp_set.extend(cypher_set_clauses("c", &component.properties));
                self.run_cypher(&format!(
                    "MERGE (c:Component {{component_id:'{}'}}) SET {}",
                    escape_cypher_string(&component.component_id),
                    comp_set.join(", ")
                ))?;
                self.run_cypher(&format!(
                    "MATCH (e:Entity {{entity_id:'{}'}}), (c:Component {{component_id:'{}'}}) MERGE (e)-[:HAS_COMPONENT]->(c)",
                    escape_cypher_string(&record.entity_id),
                    escape_cypher_string(&component.component_id),
                ))?;
            }

            self.persist_relationship_edges(record)?;
        }

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph persist"))?;

        Ok(())
    }

    pub fn remove_graph_entities(&mut self, entity_ids: &[String]) -> Result<()> {
        if entity_ids.is_empty() {
            return Ok(());
        }
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph remove"))?;

        for entity_id in entity_ids {
            self.run_cypher(&format!(
                "MATCH (e:Entity {{entity_id:'{}'}}) OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) DETACH DELETE c, e",
                escape_cypher_string(entity_id),
            ))?;
        }

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph remove"))?;
        Ok(())
    }

    pub fn persist_snapshot_marker(
        &mut self,
        snapshot_tick: u64,
        entity_count: usize,
    ) -> Result<()> {
        let now = now_epoch_s() as i64;
        self.client
            .execute(
                "INSERT INTO replication_snapshot_markers (snapshot_tick, entity_count, created_at_epoch_s) VALUES ($1, $2, $3)",
                &[&(snapshot_tick as i64), &(entity_count as i64), &now],
            )
            .map_err(db_err("insert snapshot marker"))?;
        Ok(())
    }

    pub fn drop_graph(mut self) -> Result<()> {
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph drop"))?;
        let sql = format!(
            "SELECT * FROM ag_catalog.drop_graph('{}', true);",
            escape_cypher_string(&self.graph_name)
        );
        self.client
            .batch_execute(&sql)
            .map_err(db_err("drop graph"))?;
        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph drop"))?;
        Ok(())
    }

    pub fn load_graph_records(&mut self) -> Result<Vec<GraphEntityRecord>> {
        self.client
            .batch_execute("LOAD 'age'; SET search_path = ag_catalog, \"$user\", public;")
            .map_err(db_err("prep age for graph load"))?;

        let query = format!(
            "SELECT entity_id::text AS entity_id, labels::text AS labels, props::text AS props, component_id::text AS component_id, component_kind::text AS component_kind, component_props::text AS component_props \
             FROM ag_catalog.cypher('{}', $$ \
                MATCH (e:Entity) \
                OPTIONAL MATCH (e)-[:HAS_COMPONENT]->(c:Component) \
                RETURN e.entity_id, labels(e), properties(e), c.component_id, c.component_kind, properties(c) \
             $$) AS (entity_id agtype, labels agtype, props agtype, component_id agtype, component_kind agtype, component_props agtype);",
            escape_cypher_string(&self.graph_name)
        );
        let rows = self
            .client
            .query(&query, &[])
            .map_err(db_err("load graph records"))?;

        self.client
            .batch_execute("SET search_path = public;")
            .map_err(db_err("reset search_path after graph load"))?;

        let mut by_entity = HashMap::<String, GraphEntityRecord>::new();
        for row in rows {
            let Some(entity_id) = parse_agtype_string(row.get::<_, String>("entity_id")) else {
                continue;
            };
            let mut labels = parse_agtype_json(row.get::<_, String>("labels"))
                .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                .unwrap_or_else(|| vec!["Entity".to_string()]);
            let properties = parse_agtype_json(row.get::<_, String>("props"))
                .unwrap_or(JsonValue::Object(JsonMap::new()));
            if let Some(extra_labels) = properties.get("sidereal_labels").and_then(|v| v.as_array())
            {
                labels.extend(
                    extra_labels
                        .iter()
                        .filter_map(|v| v.as_str().map(ToString::to_string)),
                );
                labels.sort();
                labels.dedup();
            }
            let entry = by_entity
                .entry(entity_id.clone())
                .or_insert_with(|| GraphEntityRecord {
                    entity_id: entity_id.clone(),
                    labels,
                    properties,
                    components: Vec::new(),
                });

            let component_id = row
                .try_get::<_, Option<String>>("component_id")
                .ok()
                .flatten()
                .and_then(parse_agtype_string);
            let component_kind = row
                .try_get::<_, Option<String>>("component_kind")
                .ok()
                .flatten()
                .and_then(parse_agtype_string);
            if let (Some(component_id), Some(component_kind)) = (component_id, component_kind) {
                let component_props = row
                    .try_get::<_, Option<String>>("component_props")
                    .ok()
                    .flatten()
                    .and_then(parse_agtype_json)
                    .unwrap_or(JsonValue::Object(JsonMap::new()));
                if !entry
                    .components
                    .iter()
                    .any(|c| c.component_id == component_id)
                {
                    entry.components.push(GraphComponentRecord {
                        component_id,
                        component_kind,
                        properties: component_props,
                    });
                }
            }
        }

        let mut out = by_entity.into_values().collect::<Vec<_>>();
        out.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
        Ok(out)
    }

    fn persist_relationship_edges(&mut self, record: &GraphEntityRecord) -> Result<()> {
        if let Some(parent_id) = record
            .properties
            .get("parent_entity_id")
            .and_then(JsonValue::as_str)
        {
            self.run_cypher(&format!(
                "MATCH (p:Entity {{entity_id:'{}'}}), (e:Entity {{entity_id:'{}'}}) MERGE (p)-[:HAS_CHILD]->(e)",
                escape_cypher_string(parent_id),
                escape_cypher_string(&record.entity_id),
            ))?;
        }

        if record.labels.iter().any(|l| l == "Hardpoint")
            && let Some(owner_id) = record
                .properties
                .get("owner_entity_id")
                .and_then(JsonValue::as_str)
        {
            self.run_cypher(&format!(
                "MATCH (s:Entity {{entity_id:'{}'}}), (h:Entity {{entity_id:'{}'}}) MERGE (s)-[:HAS_HARDPOINT]->(h)",
                escape_cypher_string(owner_id),
                escape_cypher_string(&record.entity_id),
            ))?;
        }

        if let Some(mounted_on) = record
            .properties
            .get("mounted_on_entity_id")
            .and_then(JsonValue::as_str)
        {
            self.run_cypher(&format!(
                "MATCH (m:Entity {{entity_id:'{}'}}), (h:Entity {{entity_id:'{}'}}) MERGE (m)-[:MOUNTED_ON]->(h)",
                escape_cypher_string(&record.entity_id),
                escape_cypher_string(mounted_on),
            ))?;
        }

        Ok(())
    }

    fn run_cypher(&mut self, cypher: &str) -> Result<()> {
        let sql = format!(
            "SELECT * FROM ag_catalog.cypher('{}', $$ {cypher} $$) AS (v agtype);",
            escape_cypher_string(&self.graph_name)
        );
        self.client.query(&sql, &[]).map_err(|err| {
            PersistenceError::Database(format!("cypher execution failed: {err}; query={cypher}"))
        })?;
        Ok(())
    }
}

fn sanitize_labels(labels: &[String]) -> Vec<String> {
    labels
        .iter()
        .filter_map(|label| {
            let cleaned = label
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>();
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        })
        .collect::<Vec<_>>()
}

fn cypher_set_clauses(prefix: &str, value: &JsonValue) -> Vec<String> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    obj.iter()
        .map(|(key, val)| {
            let clean_key = key
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>();
            format!("{prefix}.{clean_key}={}", cypher_literal(val))
        })
        .collect::<Vec<_>>()
}

fn cypher_literal(value: &JsonValue) -> String {
    match value {
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(v) => v.to_string(),
        JsonValue::Number(v) => v.to_string(),
        JsonValue::String(v) => format!("'{}'", escape_cypher_string(v)),
        JsonValue::Array(values) => {
            let rendered = values.iter().map(cypher_literal).collect::<Vec<_>>();
            format!("[{}]", rendered.join(","))
        }
        JsonValue::Object(map) => {
            let rendered = map
                .iter()
                .map(|(k, v)| {
                    let clean_key = k
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect::<String>();
                    format!("{clean_key}:{}", cypher_literal(v))
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", rendered.join(","))
        }
    }
}

fn parse_agtype_string(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if let Ok(parsed) = serde_json::from_str::<String>(trimmed) {
        return Some(parsed);
    }
    let stripped = strip_trailing_agtype_suffix(trimmed);
    if let Ok(parsed) = serde_json::from_str::<String>(stripped) {
        return Some(parsed);
    }
    if stripped.is_empty() || stripped == "null" {
        return None;
    }
    Some(stripped.trim_matches('"').to_string())
}

fn parse_agtype_json(raw: String) -> Option<JsonValue> {
    let trimmed = raw.trim();
    if let Ok(parsed) = serde_json::from_str::<JsonValue>(trimmed) {
        return Some(parsed);
    }
    let stripped = strip_trailing_agtype_suffix(trimmed);
    serde_json::from_str::<JsonValue>(stripped).ok()
}

fn strip_trailing_agtype_suffix(raw: &str) -> &str {
    let Some((left, suffix)) = raw.rsplit_once("::") else {
        return raw;
    };
    if matches!(suffix, "agtype" | "vertex" | "edge" | "path") {
        left
    } else {
        raw
    }
}

fn escape_cypher_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn now_epoch_s() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs()
}

fn db_err(action: &'static str) -> impl Fn(postgres::Error) -> PersistenceError {
    move |err| PersistenceError::Database(format!("{action} failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cypher_literal_renders_nested_maps_and_arrays() {
        let value = serde_json::json!({"a": 1, "b": [true, "x"], "c": {"k": "v"}});
        let out = cypher_literal(&value);
        assert!(out.contains("a:1"));
        assert!(out.contains("b:[true,'x']"));
        assert!(out.contains("c:{k:'v'}"));
    }

    #[test]
    fn parse_agtype_helpers_handle_suffix() {
        let s = parse_agtype_string("\"player:1\"::agtype".to_string()).expect("string");
        assert_eq!(s, "player:1");
        let json = parse_agtype_json("{\"x\":1}::agtype".to_string()).expect("json");
        assert_eq!(json["x"], 1);
    }

    #[test]
    fn reflect_envelope_roundtrip() {
        let payload = serde_json::json!({"fuel_kg": 42.0});
        let envelope = encode_reflect_component("sidereal_game::FuelTank", payload.clone());
        let decoded =
            decode_reflect_component(&envelope, "sidereal_game::FuelTank").expect("decode");
        assert_eq!(decoded, &payload);
    }
}
