use bevy::ecs::reflect::AppTypeRegistry;
use bevy::ecs::world::CommandQueue;
use bevy::log::info;
use bevy::prelude::*;
use lightyear::prelude::Unlink;
use lightyear::prelude::server::ClientOf;
use postgres::NoTls;
use sidereal_game::{EntityGuid, GeneratedComponentRegistry};
use sidereal_net::{NotificationPayload, NotificationPlacement, NotificationSeverity};
use sidereal_persistence::GraphPersistence;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::control::ClientControlRequestOrder;
use crate::replication::input::{
    ClientInputTickTracker, InputActivityLogState, InputRateLimitState,
    LatestRealtimeInputsByPlayer, RealtimeInputActivityByPlayer,
};
use crate::replication::lifecycle::{ClientLastActivity, HydratedGraphEntity};
use crate::replication::notifications::{
    NotificationCommand, NotificationCommandQueue, enqueue_player_notification,
};
use crate::replication::persistence::{
    PersistenceDirtyState, PersistenceFingerprintState, PersistenceSchemaInitState,
    SimulationPersistenceTimer,
};
use crate::replication::scripting::{
    AssetRegistryResource, EntityRegistryResource, ScriptCatalogResource,
};
use crate::replication::simulation_entities::{
    PlayerControlledEntityMap, PlayerRuntimeEntityMap, reload_runtime_world_from_persistence,
};
use crate::replication::visibility::{
    ClientLocalViewModeRegistry, ClientObserverAnchorPositionMap, ClientVisibilityRegistry,
    VisibilityClientContextCache, VisibilityMembershipCache, VisibilitySpatialIndex,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminCommand {
    Help,
    Clear,
    Filter {
        level: String,
    },
    Player {
        player_entity_id: String,
    },
    Entity {
        entity_guid: String,
    },
    View {
        target: String,
    },
    Notify {
        player_entity_id: String,
        body: String,
    },
    Health,
    Reset {
        force: bool,
    },
    Quit,
    Raw {
        input: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminCommandRequest {
    pub raw: String,
    pub command: AdminCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminCommandScope {
    Shared,
    TuiLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdminCommandSpec {
    pub name: &'static str,
    pub usage: &'static str,
    pub summary: &'static str,
    pub parameters: &'static str,
    pub scope: AdminCommandScope,
    pub requires_confirmation: bool,
}

const COMMAND_SPECS: &[AdminCommandSpec] = &[
    AdminCommandSpec {
        name: "help",
        usage: "help",
        summary: "Show the indexed admin command catalog.",
        parameters: "",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "clear",
        usage: "clear",
        summary: "Clear the TUI log pane history from the current point forward.",
        parameters: "",
        scope: AdminCommandScope::TuiLocal,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "filter",
        usage: "filter <all|info|warn|error>",
        summary: "Set the active log level filter.",
        parameters: "level: all|info|warn|error",
        scope: AdminCommandScope::TuiLocal,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "player",
        usage: "player <player_entity_id>",
        summary: "Inspect a specific player entity identifier.",
        parameters: "player_entity_id: canonical UUID",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "entity",
        usage: "entity <entity_guid>",
        summary: "Inspect a specific world entity identifier.",
        parameters: "entity_guid: canonical UUID",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "view",
        usage: "view <target>",
        summary: "Request a named UI focus or viewport target.",
        parameters: "target: free-form label",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "notify",
        usage: "notify <player_entity_id> <message>",
        summary: "Send a server-authored test notification to one authenticated player.",
        parameters: "player_entity_id: canonical UUID, message: free-form text",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "health",
        usage: "health",
        summary: "Print the current summary health snapshot.",
        parameters: "",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
    AdminCommandSpec {
        name: "reset",
        usage: "reset [force]",
        summary: "Disconnect clients, reset persisted runtime world state, and rerun world_init.lua.",
        parameters: "force: optional bypass for the TUI confirmation dialog",
        scope: AdminCommandScope::Shared,
        requires_confirmation: true,
    },
    AdminCommandSpec {
        name: "quit",
        usage: "quit",
        summary: "Terminate the replication process.",
        parameters: "",
        scope: AdminCommandScope::Shared,
        requires_confirmation: false,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResetRequest {
    pub force: bool,
}

#[derive(Resource, Clone)]
pub struct AdminCommandBusSender {
    sender: SyncSender<AdminCommandRequest>,
}

impl AdminCommandBusSender {
    pub fn send(&self, request: AdminCommandRequest) -> Result<(), String> {
        self.sender
            .send(request)
            .map_err(|err| format!("admin command send failed: {err}"))
    }
}

#[derive(Resource)]
pub struct AdminCommandBusReceiver {
    receiver: Mutex<Receiver<AdminCommandRequest>>,
}

#[derive(Resource, Default)]
pub struct PendingAdminResetQueue {
    requests: Vec<AdminResetRequest>,
}

impl PendingAdminResetQueue {
    fn push(&mut self, request: AdminResetRequest) {
        self.requests.push(request);
    }

    fn drain(&mut self) -> Vec<AdminResetRequest> {
        self.requests.drain(..).collect()
    }
}

pub fn init_resources(app: &mut App) {
    let (sender, receiver) = sync_channel::<AdminCommandRequest>(256);
    app.insert_resource(AdminCommandBusSender { sender });
    app.insert_resource(AdminCommandBusReceiver {
        receiver: Mutex::new(receiver),
    });
    app.insert_resource(PendingAdminResetQueue::default());
}

pub fn command_specs() -> &'static [AdminCommandSpec] {
    COMMAND_SPECS
}

pub fn command_spec(name: &str) -> Option<&'static AdminCommandSpec> {
    COMMAND_SPECS
        .iter()
        .find(|spec| spec.name.eq_ignore_ascii_case(name))
}

pub fn format_command_catalog() -> String {
    let mut lines = vec!["admin command catalog:".to_string()];
    for spec in COMMAND_SPECS {
        let mut line = format!("{} - {}", spec.usage, spec.summary);
        if !spec.parameters.is_empty() {
            line.push_str(&format!(" | params: {}", spec.parameters));
        }
        if spec.scope == AdminCommandScope::TuiLocal {
            line.push_str(" | tui-local");
        }
        if spec.requires_confirmation {
            line.push_str(" | confirm");
        }
        lines.push(line);
    }
    lines.join("\n")
}

pub fn execute_admin_commands(
    receiver: Res<'_, AdminCommandBusReceiver>,
    mut reset_queue: ResMut<'_, PendingAdminResetQueue>,
    mut notification_queue: ResMut<'_, NotificationCommandQueue>,
    health_snapshot: Option<Res<'_, crate::replication::health::ReplicationHealthSnapshot>>,
    mut exit: MessageWriter<'_, AppExit>,
) {
    let receiver = receiver
        .receiver
        .lock()
        .expect("admin command receiver lock poisoned");
    loop {
        match receiver.try_recv() {
            Ok(request) => match &request.command {
                AdminCommand::Quit => {
                    info!("replication shutdown requested from admin command bus");
                    exit.write(AppExit::Success);
                }
                AdminCommand::Reset { force } => {
                    info!(
                        "replication admin reset requested force={force} raw={}",
                        request.raw
                    );
                    reset_queue.push(AdminResetRequest { force: *force });
                }
                AdminCommand::Notify {
                    player_entity_id,
                    body,
                } => {
                    if player_entity_id.trim().is_empty() || body.trim().is_empty() {
                        info!("admin notify rejected: usage notify <player_entity_id> <message>");
                    } else {
                        enqueue_player_notification(
                            &mut notification_queue,
                            NotificationCommand {
                                player_entity_id: player_entity_id.clone(),
                                title: "Server Notification Test".to_string(),
                                body: body.clone(),
                                severity: NotificationSeverity::Info,
                                placement: NotificationPlacement::BottomRight,
                                image: None,
                                payload: NotificationPayload::Generic {
                                    event_type: "server_admin_notify_test".to_string(),
                                    data: serde_json::json!({
                                        "source": "replication_admin",
                                    }),
                                },
                                auto_dismiss_after_s: None,
                            },
                        );
                        info!("queued server admin notification for player={player_entity_id}");
                    }
                }
                _ => info!(
                    "{}",
                    format_admin_command_result(&request, health_snapshot.as_deref())
                ),
            },
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                info!("replication admin command bus disconnected");
                break;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn apply_admin_resets(world: &mut World) {
    let requests = {
        let mut reset_queue = world.resource_mut::<PendingAdminResetQueue>();
        reset_queue.drain()
    };
    for request in requests {
        match perform_admin_reset(world) {
            Ok(hydrated_count) => info!(
                "replication admin reset complete force={} hydrated_entities={}",
                request.force, hydrated_count
            ),
            Err(err) => bevy::log::error!("replication admin reset failed: {err}"),
        }
    }
}

pub fn parse_admin_command(input: &str) -> AdminCommandRequest {
    let trimmed = input.trim();
    let mut parts = trimmed.split_whitespace();
    let command = match parts.next() {
        Some("help") => AdminCommand::Help,
        Some("clear") => AdminCommand::Clear,
        Some("filter") => AdminCommand::Filter {
            level: parts.next().unwrap_or("info").to_string(),
        },
        Some("player") => AdminCommand::Player {
            player_entity_id: parts.collect::<Vec<_>>().join(" "),
        },
        Some("entity") => AdminCommand::Entity {
            entity_guid: parts.collect::<Vec<_>>().join(" "),
        },
        Some("view") => AdminCommand::View {
            target: parts.collect::<Vec<_>>().join(" "),
        },
        Some("notify") => {
            let player_entity_id = parts.next().unwrap_or_default().to_string();
            AdminCommand::Notify {
                player_entity_id,
                body: parts.collect::<Vec<_>>().join(" "),
            }
        }
        Some("health") => AdminCommand::Health,
        Some("reset") => AdminCommand::Reset {
            force: parts
                .next()
                .is_some_and(|value| value.eq_ignore_ascii_case("force")),
        },
        Some("quit") | Some("exit") => AdminCommand::Quit,
        _ => AdminCommand::Raw {
            input: trimmed.to_string(),
        },
    };
    AdminCommandRequest {
        raw: trimmed.to_string(),
        command,
    }
}

fn format_admin_command_result(
    request: &AdminCommandRequest,
    health_snapshot: Option<&crate::replication::health::ReplicationHealthSnapshot>,
) -> String {
    match &request.command {
        AdminCommand::Help => format_command_catalog(),
        AdminCommand::Clear => "admin clear requested (handled by TUI surfaces)".to_string(),
        AdminCommand::Filter { level } => format!("admin log filter requested level={level}"),
        AdminCommand::Player { player_entity_id } => {
            format!("admin player inspect requested player_entity_id={player_entity_id}")
        }
        AdminCommand::Entity { entity_guid } => {
            format!("admin entity inspect requested entity_guid={entity_guid}")
        }
        AdminCommand::View { target } => format!("admin view requested target={target}"),
        AdminCommand::Notify {
            player_entity_id,
            body,
        } => format!(
            "admin notify requested player_entity_id={} body_len={}",
            player_entity_id,
            body.chars().count()
        ),
        AdminCommand::Health => match health_snapshot {
            Some(snapshot) => format!(
                "health status={} users_online={} sessions={} entities={} physics_bodies={} lua_errors={}",
                snapshot.status,
                snapshot.users_online,
                snapshot.session_count,
                snapshot.world_entity_count,
                snapshot.physics_body_count,
                snapshot.lua_runtime.error_count
            ),
            None => "health snapshot unavailable".to_string(),
        },
        AdminCommand::Reset { force } => format!(
            "replication reset requested force={} (disconnects clients and rebuilds world state)",
            force
        ),
        AdminCommand::Quit => "replication shutdown requested".to_string(),
        AdminCommand::Raw { input } => format!("admin command not implemented: {input}"),
    }
}

fn perform_admin_reset(world: &mut World) -> Result<usize, String> {
    let client_entities = {
        let mut query = world.query_filtered::<Entity, With<ClientOf>>();
        query.iter(world).collect::<Vec<_>>()
    };
    let world_entities = {
        let mut query = world.query_filtered::<Entity, With<EntityGuid>>();
        query.iter(world).collect::<Vec<_>>()
    };
    let hydrated_markers = {
        let mut query = world.query_filtered::<Entity, With<HydratedGraphEntity>>();
        query.iter(world).collect::<Vec<_>>()
    };
    let script_catalog = world.resource::<ScriptCatalogResource>().clone();
    let entity_registry = world.resource::<EntityRegistryResource>().clone();
    let asset_registry = world.resource::<AssetRegistryResource>().clone();
    let component_registry = world.resource::<GeneratedComponentRegistry>().clone();
    let app_type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut command_queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut command_queue, world);
        disconnect_all_clients(&mut commands, &client_entities);
        clear_live_runtime_state(&mut commands, &world_entities, &hydrated_markers);
    }
    command_queue.apply(world);

    {
        world
            .resource_mut::<AuthenticatedClientBindings>()
            .by_client_entity
            .clear();
        world
            .resource_mut::<AuthenticatedClientBindings>()
            .by_remote_id
            .clear();
        world
            .resource_mut::<ClientVisibilityRegistry>()
            .player_entity_id_by_client
            .clear();
        world
            .resource_mut::<ClientLocalViewModeRegistry>()
            .by_client_entity
            .clear();
        world.resource_mut::<VisibilityClientContextCache>().clear();
        world.resource_mut::<VisibilityMembershipCache>().clear();
        world.resource_mut::<VisibilitySpatialIndex>().clear();
        world
            .resource_mut::<ClientObserverAnchorPositionMap>()
            .position_by_player_entity_id
            .clear();
        world
            .resource_mut::<ClientControlRequestOrder>()
            .last_request_seq_by_player
            .clear();
        world.resource_mut::<ClientLastActivity>().0.clear();
        world
            .resource_mut::<ClientInputTickTracker>()
            .last_accepted_tick_by_stream
            .clear();
        world
            .resource_mut::<InputRateLimitState>()
            .current_window_index_by_player_entity_id
            .clear();
        world
            .resource_mut::<InputRateLimitState>()
            .message_count_in_window_by_player_entity_id
            .clear();
        world
            .resource_mut::<LatestRealtimeInputsByPlayer>()
            .by_player_entity_id
            .clear();
        world
            .resource_mut::<RealtimeInputActivityByPlayer>()
            .last_received_at_s_by_player_entity_id
            .clear();
        world
            .resource_mut::<InputActivityLogState>()
            .last_logged_at_s_by_player_entity_id
            .clear();
        world
            .resource_mut::<InputActivityLogState>()
            .last_logged_actions_by_player_entity_id
            .clear();
        world
            .resource_mut::<PersistenceDirtyState>()
            .initial_full_snapshot_pending = true;
        world
            .resource_mut::<PersistenceDirtyState>()
            .dirty_entity_ids
            .clear();
        world
            .resource_mut::<PersistenceFingerprintState>()
            .by_entity_id
            .clear();
        world
            .resource_mut::<SimulationPersistenceTimer>()
            .last_flush_at_s = None;
    }

    reset_persisted_runtime_world()?;
    let mut controlled_entity_map = world
        .remove_resource::<PlayerControlledEntityMap>()
        .unwrap_or_default();
    let mut player_entity_map = world
        .remove_resource::<PlayerRuntimeEntityMap>()
        .unwrap_or_default();
    let mut persistence_schema = world
        .remove_resource::<PersistenceSchemaInitState>()
        .unwrap_or_default();
    controlled_entity_map.by_player_entity_id.clear();
    player_entity_map.by_player_entity_id.clear();
    persistence_schema.0 = false;

    let mut command_queue = CommandQueue::default();
    let hydrated_count = {
        let mut commands = Commands::new(&mut command_queue, world);
        reload_runtime_world_from_persistence(
            &mut commands,
            &script_catalog,
            &entity_registry,
            &asset_registry,
            &component_registry,
            &app_type_registry,
            &mut controlled_entity_map,
            &mut player_entity_map,
            &mut persistence_schema,
        )?
    };
    command_queue.apply(world);
    world.insert_resource(controlled_entity_map);
    world.insert_resource(player_entity_map);
    world.insert_resource(persistence_schema);
    Ok(hydrated_count)
}

fn disconnect_all_clients(commands: &mut Commands<'_, '_>, clients: &[Entity]) {
    for client_entity in clients {
        commands.trigger(Unlink {
            entity: *client_entity,
            reason: "admin_reset".to_string(),
        });
    }
}

fn clear_live_runtime_state(
    commands: &mut Commands<'_, '_>,
    world_entities: &[Entity],
    hydrated_markers: &[Entity],
) {
    for entity in world_entities {
        commands.entity(*entity).despawn();
    }
    for marker in hydrated_markers {
        commands.entity(*marker).despawn();
    }
}

fn reset_persisted_runtime_world() -> Result<(), String> {
    let database_url = replication_database_url();
    let mut persistence = GraphPersistence::connect(&database_url)
        .map_err(|err| format!("admin reset connect failed: {err}"))?;
    persistence
        .ensure_schema()
        .map_err(|err| format!("admin reset ensure schema failed: {err}"))?;
    persistence
        .drop_graph()
        .map_err(|err| format!("admin reset drop graph failed: {err}"))?;

    let mut client = postgres::Client::connect(&database_url, NoTls)
        .map_err(|err| format!("admin reset postgres reconnect failed: {err}"))?;
    for relation in [
        "public.replication_snapshot_markers",
        "sidereal.replication_snapshot_markers",
        "public.script_world_init_state",
        "sidereal.script_world_init_state",
        "public.replication_player_bootstrap",
        "sidereal.replication_player_bootstrap",
        "public.replication_bootstrap_events",
        "sidereal.replication_bootstrap_events",
    ] {
        truncate_optional_reset_table(&mut client, relation)?;
    }
    Ok(())
}

fn truncate_optional_reset_table(
    client: &mut postgres::Client,
    relation: &'static str,
) -> Result<(), String> {
    let row = client
        .query_one("SELECT to_regclass($1)::text", &[&relation])
        .map_err(|err| format!("admin reset table lookup failed for {relation}: {err}"))?;
    let resolved: Option<String> = row.get(0);
    if resolved.is_none() {
        return Ok(());
    }

    let statement = format!("TRUNCATE TABLE {relation} RESTART IDENTITY;");
    client
        .batch_execute(&statement)
        .map_err(|err| format!("admin reset table cleanup failed for `{statement}`: {err}"))
}

fn replication_database_url() -> String {
    std::env::var("REPLICATION_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://sidereal:sidereal@127.0.0.1:5432/sidereal".to_string())
}

#[cfg(test)]
mod tests {
    use super::{AdminCommand, command_spec, format_command_catalog, parse_admin_command};

    #[test]
    fn parse_health_command() {
        let request = parse_admin_command("health");
        assert_eq!(request.command, AdminCommand::Health);
    }

    #[test]
    fn parse_reset_force_command() {
        let request = parse_admin_command("reset force");
        assert_eq!(request.command, AdminCommand::Reset { force: true });
    }

    #[test]
    fn parse_notify_command() {
        let request = parse_admin_command(
            "notify 11111111-1111-1111-1111-111111111111 Server side notification",
        );
        assert_eq!(
            request.command,
            AdminCommand::Notify {
                player_entity_id: "11111111-1111-1111-1111-111111111111".to_string(),
                body: "Server side notification".to_string(),
            }
        );
    }

    #[test]
    fn command_catalog_includes_reset() {
        let catalog = format_command_catalog();
        assert!(catalog.contains("reset [force]"));
        assert!(catalog.contains("notify <player_entity_id> <message>"));
        assert!(command_spec("reset").is_some());
        assert!(command_spec("notify").is_some());
    }

    #[test]
    fn parse_unknown_command_as_raw() {
        let request = parse_admin_command("foo bar");
        assert_eq!(
            request.command,
            AdminCommand::Raw {
                input: "foo bar".to_string()
            }
        );
    }
}
