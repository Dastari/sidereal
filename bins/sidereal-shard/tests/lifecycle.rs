use avian3d::collision::CollisionDiagnostics;
use avian3d::dynamics::solver::SolverDiagnostics;
use avian3d::picking::PhysicsPickingDiagnostics;
use avian3d::prelude::{LinearVelocity, PhysicsPlugins, Position, RigidBody};
use avian3d::spatial_query::SpatialQueryDiagnostics;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::prelude::*;
use bevy::scene::ScenePlugin;
use sidereal_game::SiderealGamePlugin;
use sidereal_game::generated::components::{
    DisplayName, Engine, FlightComputer, Hardpoint, HealthPool,
};
use sidereal_net::{WorldComponentDelta, WorldDeltaEntity};
use sidereal_persistence::GraphPersistence;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Component, Clone)]
struct SidEntityId(String);

fn test_database_url() -> String {
    std::env::var("SIDEREAL_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("REPLICATION_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

fn unique_graph_name(prefix: &str) -> String {
    format!("{}_{}", prefix, Uuid::new_v4().simple())
}

fn world_to_deltas(app: &mut App) -> Vec<WorldDeltaEntity> {
    let mut out = Vec::<WorldDeltaEntity>::new();
    let entity_id_by_entity = app
        .world_mut()
        .query::<(Entity, &SidEntityId)>()
        .iter(app.world())
        .map(|(entity, sid)| (entity, sid.0.clone()))
        .collect::<HashMap<_, _>>();

    let mut query = app.world_mut().query::<(
        Entity,
        &SidEntityId,
        Option<&DisplayName>,
        Option<&Hardpoint>,
        Option<&Engine>,
        Option<&FlightComputer>,
        Option<&HealthPool>,
        Option<&Position>,
        Option<&LinearVelocity>,
        Option<&ChildOf>,
    )>();
    for (
        entity,
        sid,
        display_name,
        hardpoint,
        engine,
        flight_computer,
        health_pool,
        position,
        velocity,
        child_of,
    ) in query.iter(app.world())
    {
        let mut labels = vec!["Entity".to_string()];
        let mut props = serde_json::json!({});
        let mut components = Vec::<WorldComponentDelta>::new();

        if let Some(position) = position {
            props["position_m"] = serde_json::json!([position.x, position.y, position.z]);
        }
        if let Some(velocity) = velocity {
            props["velocity_mps"] = serde_json::json!([velocity.x, velocity.y, velocity.z]);
            labels.push("Ship".to_string());
        }
        if let Some(child_of) = child_of
            && let Some(parent_id) = entity_id_by_entity.get(&child_of.parent())
        {
            props["parent_entity_id"] = serde_json::json!(parent_id);
        }
        if let Some(display_name) = display_name {
            components.push(WorldComponentDelta {
                component_id: format!("{}:display_name", sid.0),
                component_kind: "display_name".to_string(),
                properties: serde_json::json!({"value": display_name.0}),
            });
        }
        if let Some(hardpoint) = hardpoint {
            labels.push("Hardpoint".to_string());
            props["hardpoint_id"] = serde_json::json!(hardpoint.hardpoint_id);
            props["offset_m"] = serde_json::json!([
                hardpoint.offset_m.x,
                hardpoint.offset_m.y,
                hardpoint.offset_m.z
            ]);
            components.push(WorldComponentDelta {
                component_id: format!("{}:hardpoint", sid.0),
                component_kind: "hardpoint".to_string(),
                properties: serde_json::json!({
                    "hardpoint_id": hardpoint.hardpoint_id,
                    "offset_m": [hardpoint.offset_m.x, hardpoint.offset_m.y, hardpoint.offset_m.z]
                }),
            });
        }
        if let Some(engine) = engine {
            labels.push("Engine".to_string());
            props["thrust_n"] = serde_json::json!(engine.thrust_n);
            components.push(WorldComponentDelta {
                component_id: format!("{}:engine", sid.0),
                component_kind: "engine".to_string(),
                properties: serde_json::json!({
                    "thrust_n": engine.thrust_n,
                    "burn_rate_kg_s": engine.burn_rate_kg_s,
                    "thrust_dir": [engine.thrust_dir.x, engine.thrust_dir.y, engine.thrust_dir.z]
                }),
            });
        }
        if let Some(flight_computer) = flight_computer {
            components.push(WorldComponentDelta {
                component_id: format!("{}:flight_computer", sid.0),
                component_kind: "flight_computer".to_string(),
                properties: serde_json::json!({
                    "profile": flight_computer.profile,
                    "throttle": flight_computer.throttle,
                    "yaw_input": flight_computer.yaw_input,
                    "turn_rate_deg_s": flight_computer.turn_rate_deg_s
                }),
            });
        }
        if let Some(health_pool) = health_pool {
            components.push(WorldComponentDelta {
                component_id: format!("{}:health_pool", sid.0),
                component_kind: "health_pool".to_string(),
                properties: serde_json::json!({"current": health_pool.current, "maximum": health_pool.maximum}),
            });
        }

        let _ = entity;
        out.push(WorldDeltaEntity {
            entity_id: sid.0.clone(),
            labels,
            properties: props,
            components,
            removed: false,
        });
    }
    out
}

fn hydrate_world_from_graph(app: &mut App, persistence: &mut GraphPersistence) {
    let records = persistence
        .load_graph_records()
        .expect("graph load for hydration should succeed");
    let mut by_id = HashMap::<String, Entity>::new();

    for record in &records {
        let mut entity_commands = app.world_mut().spawn((
            SidEntityId(record.entity_id.clone()),
            Name::new(record.entity_id.clone()),
        ));
        let has_ship_motion = record.properties.get("velocity_mps").is_some();
        if has_ship_motion {
            let position = record
                .properties
                .get("position_m")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let velocity = record
                .properties
                .get("velocity_mps")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if position.len() == 3 && velocity.len() == 3 {
                entity_commands.insert((
                    RigidBody::Dynamic,
                    Position::from_xyz(
                        position[0].as_f64().unwrap_or_default() as f32,
                        position[1].as_f64().unwrap_or_default() as f32,
                        position[2].as_f64().unwrap_or_default() as f32,
                    ),
                    LinearVelocity(Vec3::new(
                        velocity[0].as_f64().unwrap_or_default() as f32,
                        velocity[1].as_f64().unwrap_or_default() as f32,
                        velocity[2].as_f64().unwrap_or_default() as f32,
                    )),
                ));
            }
        }

        for component in &record.components {
            match component.component_kind.as_str() {
                "display_name" => {
                    if let Some(name) = component.properties.get("value").and_then(|v| v.as_str()) {
                        entity_commands.insert(DisplayName(name.to_string()));
                    }
                }
                "hardpoint" => {
                    let hardpoint_id = component
                        .properties
                        .get("hardpoint_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("hp")
                        .to_string();
                    let offset = component
                        .properties
                        .get("offset_m")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    if offset.len() == 3 {
                        entity_commands.insert(Hardpoint {
                            hardpoint_id,
                            offset_m: Vec3::new(
                                offset[0].as_f64().unwrap_or_default() as f32,
                                offset[1].as_f64().unwrap_or_default() as f32,
                                offset[2].as_f64().unwrap_or_default() as f32,
                            ),
                        });
                    }
                }
                "engine" => {
                    let thrust_dir = component
                        .properties
                        .get("thrust_dir")
                        .and_then(|v| v.as_array())
                        .cloned()
                        .unwrap_or_default();
                    if thrust_dir.len() == 3 {
                        entity_commands.insert(Engine {
                            thrust_n: component
                                .properties
                                .get("thrust_n")
                                .and_then(|v| v.as_f64())
                                .unwrap_or_default() as f32,
                            burn_rate_kg_s: component
                                .properties
                                .get("burn_rate_kg_s")
                                .and_then(|v| v.as_f64())
                                .unwrap_or_default()
                                as f32,
                            thrust_dir: Vec3::new(
                                thrust_dir[0].as_f64().unwrap_or_default() as f32,
                                thrust_dir[1].as_f64().unwrap_or_default() as f32,
                                thrust_dir[2].as_f64().unwrap_or_default() as f32,
                            ),
                        });
                    }
                }
                "flight_computer" => {
                    entity_commands.insert(FlightComputer {
                        profile: component
                            .properties
                            .get("profile")
                            .and_then(|v| v.as_str())
                            .unwrap_or("ManualAssist")
                            .to_string(),
                        throttle: component
                            .properties
                            .get("throttle")
                            .and_then(|v| v.as_f64())
                            .unwrap_or_default() as f32,
                        yaw_input: component
                            .properties
                            .get("yaw_input")
                            .and_then(|v| v.as_f64())
                            .unwrap_or_default() as f32,
                        turn_rate_deg_s: component
                            .properties
                            .get("turn_rate_deg_s")
                            .and_then(|v| v.as_f64())
                            .unwrap_or_default() as f32,
                    });
                }
                "health_pool" => {
                    entity_commands.insert(HealthPool {
                        current: component
                            .properties
                            .get("current")
                            .and_then(|v| v.as_f64())
                            .unwrap_or_default() as f32,
                        maximum: component
                            .properties
                            .get("maximum")
                            .and_then(|v| v.as_f64())
                            .unwrap_or_default() as f32,
                    });
                }
                _ => {}
            }
        }
        by_id.insert(record.entity_id.clone(), entity_commands.id());
    }

    for record in &records {
        let Some(child) = by_id.get(&record.entity_id).copied() else {
            continue;
        };
        let Some(parent_id) = record
            .properties
            .get("parent_entity_id")
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        if let Some(parent) = by_id.get(parent_id).copied() {
            app.world_mut().entity_mut(parent).add_child(child);
        }
    }
}

#[test]
fn shard_avian_hydration_persistence_roundtrip() {
    let database_url = test_database_url();
    let graph_name = unique_graph_name("sidereal_shard_lifecycle");
    let mut persistence = match GraphPersistence::connect_with_graph(&database_url, &graph_name) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("skipping shard lifecycle test; postgres unavailable: {err}");
            return;
        }
    };
    if let Err(err) = persistence.ensure_schema() {
        eprintln!("skipping shard lifecycle test; AGE schema unavailable: {err}");
        return;
    }

    let ship_id = format!("ship:{}", Uuid::new_v4());
    let hardpoint_id = format!("hardpoint:{}", Uuid::new_v4());
    let engine_id = format!("engine:{}", Uuid::new_v4());
    let flight_computer_id = format!("flight_computer:{}", Uuid::new_v4());

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        SiderealGamePlugin,
        PhysicsPlugins::new(Update),
    ));
    app.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app.init_asset::<Mesh>();
    app.insert_resource(CollisionDiagnostics::default());
    app.insert_resource(SolverDiagnostics::default());
    app.insert_resource(SpatialQueryDiagnostics::default());
    app.insert_resource(PhysicsPickingDiagnostics::default());

    let ship = app.world_mut().spawn((
        SidEntityId(ship_id.clone()),
        DisplayName("ISS Shard Lifecycle".to_string()),
        FlightComputer {
            profile: "CruiseAssist".to_string(),
            throttle: 0.62,
            yaw_input: 0.0,
            turn_rate_deg_s: 45.0,
        },
        HealthPool {
            current: 100.0,
            maximum: 100.0,
        },
        RigidBody::Dynamic,
        Position::from_xyz(0.0, 0.0, 0.0),
        LinearVelocity(Vec3::new(3.0, 0.0, 0.0)),
    ));
    let ship_entity = ship.id();
    let hardpoint_entity = app
        .world_mut()
        .spawn((
            SidEntityId(hardpoint_id.clone()),
            Hardpoint {
                hardpoint_id: "engine_main".to_string(),
                offset_m: Vec3::new(0.0, 0.0, -3.0),
            },
        ))
        .id();
    let engine_entity = app
        .world_mut()
        .spawn((
            SidEntityId(engine_id.clone()),
            Engine {
                thrust_n: 250_000.0,
                burn_rate_kg_s: 16.0,
                thrust_dir: Vec3::Z,
            },
        ))
        .id();
    let flight_computer_entity = app
        .world_mut()
        .spawn((SidEntityId(flight_computer_id.clone()),))
        .id();

    app.world_mut()
        .entity_mut(ship_entity)
        .add_child(hardpoint_entity);
    app.world_mut()
        .entity_mut(hardpoint_entity)
        .add_child(engine_entity);
    app.world_mut()
        .entity_mut(ship_entity)
        .add_child(flight_computer_entity);

    for _ in 0..12 {
        app.update();
    }

    let first_batch = world_to_deltas(&mut app);
    persistence
        .persist_world_delta(&first_batch, 300)
        .expect("first persistence should succeed");

    let first_records = persistence
        .load_graph_records()
        .expect("first graph load should succeed");
    assert!(first_records.iter().any(|r| r.entity_id == ship_id));
    assert!(first_records.iter().any(|r| r.entity_id == hardpoint_id));
    assert!(first_records.iter().any(|r| r.entity_id == engine_id));

    let mut app_hydrated = App::new();
    app_hydrated.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        SiderealGamePlugin,
        PhysicsPlugins::new(Update),
    ));
    app_hydrated.add_message::<bevy::asset::AssetEvent<Mesh>>();
    app_hydrated.init_asset::<Mesh>();
    app_hydrated.insert_resource(CollisionDiagnostics::default());
    app_hydrated.insert_resource(SolverDiagnostics::default());
    app_hydrated.insert_resource(SpatialQueryDiagnostics::default());
    app_hydrated.insert_resource(PhysicsPickingDiagnostics::default());
    hydrate_world_from_graph(&mut app_hydrated, &mut persistence);
    for _ in 0..12 {
        app_hydrated.update();
    }

    let second_batch = world_to_deltas(&mut app_hydrated);
    persistence
        .persist_world_delta(&second_batch, 301)
        .expect("second persistence should succeed");
    let second_records = persistence
        .load_graph_records()
        .expect("second graph load should succeed");

    let ship_record = second_records
        .iter()
        .find(|r| r.entity_id == ship_id)
        .expect("ship should still exist");
    let pos = ship_record
        .properties
        .get("position_m")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(pos.len(), 3);
    assert!(pos[0].as_f64().unwrap_or_default() > 0.0);

    persistence.drop_graph().expect("test graph should drop");
}
