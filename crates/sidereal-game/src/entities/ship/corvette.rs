//! Corvette ship archetype: bundle, defaults, spawn helper, and deterministic spawn position.
//! Canonical starter ship granted on registration.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

use crate::{
    BaseMassKg, CargoMassKg, CollisionAabbM, DisplayName, Engine, EntityGuid, FlightComputer,
    FlightTuning, FuelTank, Hardpoint, HealthPool, Inventory, MassDirty, MassKg, MaxVelocityMps,
    ModuleMassKg, MountedOn, OwnerId, ShardAssignment, ShipTag, SizeM, SpriteShaderAssetId,
    TotalMassKg, VisualAssetId,
};

// -----------------------------------------------------------------------------
// Defaults (single source for this archetype)
// -----------------------------------------------------------------------------

pub fn default_corvette_flight_computer() -> FlightComputer {
    FlightComputer {
        profile: "basic_fly_by_wire".to_string(),
        throttle: 0.0,
        yaw_input: 0.0,
        brake_active: false,
        turn_rate_deg_s: 90.0,
    }
}

pub fn default_corvette_mass_kg() -> f32 {
    15_000.0
}

pub fn default_corvette_size() -> SizeM {
    SizeM {
        length: 25.0,
        width: 12.0,
        height: 8.0,
    }
}

pub fn default_corvette_flight_tuning() -> FlightTuning {
    // Brake and auto-brake accel set so tuning does not limit decel; engine reverse thrust is the limit (same as forward).
    let forward_accel_mps2 = 300_000.0 / (default_corvette_mass_kg() + 50.0 + 500.0 * 2.0 + 1100.0 * 2.0);
    FlightTuning {
        max_linear_accel_mps2: 120.0,
        passive_brake_accel_mps2: forward_accel_mps2,
        active_brake_accel_mps2: forward_accel_mps2,
        drag_per_s: 0.4,
    }
}

pub fn default_corvette_max_velocity_mps() -> MaxVelocityMps {
    MaxVelocityMps(100.0)
}

pub fn default_corvette_health_pool() -> HealthPool {
    HealthPool {
        current: 1000.0,
        maximum: 1000.0,
    }
}

pub fn default_corvette_asset_id() -> &'static str {
    "corvette_01"
}

pub fn default_starfield_shader_asset_id() -> &'static str {
    "starfield_wgsl"
}

pub fn default_space_background_shader_asset_id() -> &'static str {
    "space_background_wgsl"
}

/// Default engine stats for corvette (used by bundle and graph records).
/// Forward thrust halved; reverse and braking use same magnitude as forward.
pub fn default_corvette_engine() -> Engine {
    let forward_thrust = 300_000.0; // half of previous 600_000
    Engine {
        thrust: forward_thrust,
        reverse_thrust: forward_thrust,
        torque_thrust: 1_500_000.0,
        burn_rate_kg_s: 0.8,
    }
}

/// Default fuel tank for corvette modules.
pub fn default_corvette_fuel_tank() -> FuelTank {
    FuelTank { fuel_kg: 1000.0 }
}

// -----------------------------------------------------------------------------
// Bundle
// -----------------------------------------------------------------------------

/// Complete component bundle for the Prospector-class corvette.
/// Single-entity hull; use `spawn_corvette` for hull + modules.
#[derive(Bundle, Debug, Clone)]
pub struct CorvetteBundle {
    pub entity_guid: EntityGuid,
    pub ship_tag: ShipTag,
    pub visual_asset_id: VisualAssetId,
    pub sprite_shader_asset_id: SpriteShaderAssetId,
    pub display_name: DisplayName,
    pub mass: MassKg,
    pub base_mass: BaseMassKg,
    pub cargo_mass: CargoMassKg,
    pub module_mass: ModuleMassKg,
    pub total_mass: TotalMassKg,
    pub mass_dirty: MassDirty,
    pub inventory: Inventory,
    pub size: SizeM,
    pub collision: CollisionAabbM,
    pub health: HealthPool,
    pub flight_tuning: FlightTuning,
    pub max_velocity_mps: MaxVelocityMps,
    pub owner_id: OwnerId,
    pub shard_assignment: ShardAssignment,
}

// -----------------------------------------------------------------------------
// Overrides (minimal spawn-time parameters)
// -----------------------------------------------------------------------------

/// Minimal overrides when spawning a corvette. Unset fields use archetype defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorvetteOverrides {
    pub owner_account_id: Option<Uuid>,
    pub player_entity_id: Option<String>,
    pub shard_id: Option<i32>,
    pub position: Option<Vec3>,
    pub velocity: Option<Vec3>,
    pub display_name: Option<String>,
}

impl CorvetteOverrides {
    pub fn for_player(owner_account_id: Uuid, player_entity_id: String, shard_id: i32) -> Self {
        Self {
            owner_account_id: Some(owner_account_id),
            player_entity_id: Some(player_entity_id),
            shard_id: Some(shard_id),
            ..Default::default()
        }
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }
}

/// Deterministic spawn position for a given account (1km x 1km area, z = 0).
/// Used by gateway and replication for starter ship placement.
pub fn corvette_random_spawn_position(account_id: Uuid) -> Vec3 {
    let mut hasher = DefaultHasher::new();
    account_id.hash(&mut hasher);
    let seed = hasher.finish();
    let x = ((seed.wrapping_mul(1664525).wrapping_add(1013904223)) % 1000) as f32 - 500.0;
    let y = ((seed.wrapping_mul(22695477).wrapping_add(1)) % 1000) as f32 - 500.0;
    Vec3::new(x, y, 0.0)
}

// -----------------------------------------------------------------------------
// Spawn (ECS)
// -----------------------------------------------------------------------------

/// Spawns a complete corvette (hull + hardpoints + modules). Returns ship GUID and module GUIDs.
pub fn spawn_corvette(
    commands: &mut Commands,
    overrides: impl Into<CorvetteOverrides>,
) -> (Uuid, CorvetteModuleGuids) {
    let overrides = overrides.into();
    let ship_guid = Uuid::new_v4();
    let player_entity_id = overrides
        .player_entity_id
        .clone()
        .unwrap_or_else(|| "player:unknown".to_string());
    let shard_id = overrides.shard_id.unwrap_or(0);
    let display_name = overrides
        .display_name
        .clone()
        .unwrap_or_else(|| "Prospector-14".to_string());

    let size = default_corvette_size();
    let hull_mass = default_corvette_mass_kg();

    let ship_entity = commands
        .spawn(CorvetteBundle {
            entity_guid: EntityGuid(ship_guid),
            ship_tag: ShipTag,
            visual_asset_id: VisualAssetId(default_corvette_asset_id().to_string()),
            sprite_shader_asset_id: SpriteShaderAssetId(None),
            display_name: DisplayName(display_name),
            mass: MassKg(hull_mass),
            base_mass: BaseMassKg(hull_mass),
            cargo_mass: CargoMassKg(0.0),
            module_mass: ModuleMassKg(0.0),
            total_mass: TotalMassKg(hull_mass),
            mass_dirty: MassDirty,
            inventory: Inventory::default(),
            size,
            collision: CollisionAabbM {
                half_extents: Vec3::new(size.length * 0.5, size.width * 0.5, size.height * 0.5),
            },
            health: default_corvette_health_pool(),
            flight_tuning: default_corvette_flight_tuning(),
            max_velocity_mps: default_corvette_max_velocity_mps(),
            owner_id: OwnerId(player_entity_id.clone()),
            shard_assignment: ShardAssignment(shard_id),
        })
        .id();

    let hardpoints = vec![
        Hardpoint {
            hardpoint_id: "computer_core".to_string(),
            offset_m: Vec3::new(0.0, 0.0, -5.0),
        },
        Hardpoint {
            hardpoint_id: "engine_left_aft".to_string(),
            offset_m: Vec3::new(-4.0, -1.0, -10.0),
        },
        Hardpoint {
            hardpoint_id: "engine_right_aft".to_string(),
            offset_m: Vec3::new(4.0, -1.0, -10.0),
        },
    ];

    for hardpoint in hardpoints {
        commands.entity(ship_entity).with_children(|parent| {
            parent.spawn((
                EntityGuid(Uuid::new_v4()),
                hardpoint.clone(),
                DisplayName(format!("Hardpoint: {}", hardpoint.hardpoint_id)),
                OwnerId(player_entity_id.clone()),
            ));
        });
    }

    let module_guids = spawn_corvette_modules(commands, ship_guid, &player_entity_id, shard_id);
    (ship_guid, module_guids)
}

/// GUIDs for all spawned corvette modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorvetteModuleGuids {
    pub flight_computer: Uuid,
    pub engine_left: Uuid,
    pub engine_right: Uuid,
    pub fuel_tank_left: Uuid,
    pub fuel_tank_right: Uuid,
}

fn spawn_corvette_modules(
    commands: &mut Commands,
    ship_guid: Uuid,
    player_entity_id: &str,
    shard_id: i32,
) -> CorvetteModuleGuids {
    let owner = OwnerId(player_entity_id.to_string());
    let shard = ShardAssignment(shard_id);
    let engine = default_corvette_engine();
    let fuel_tank = default_corvette_fuel_tank();

    let flight_computer_guid = Uuid::new_v4();
    commands.spawn((
        EntityGuid(flight_computer_guid),
        DisplayName("Flight Computer MK1".to_string()),
        default_corvette_flight_computer(),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "computer_core".to_string(),
        },
        MassKg(50.0),
        owner.clone(),
        shard,
    ));

    let engine_left_guid = Uuid::new_v4();
    let fuel_tank_left_guid = Uuid::new_v4();
    commands.spawn((
        EntityGuid(engine_left_guid),
        DisplayName("Engine Port".to_string()),
        engine.clone(),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_left_aft".to_string(),
        },
        MassKg(500.0),
        owner.clone(),
        shard,
    ));
    commands.spawn((
        EntityGuid(fuel_tank_left_guid),
        DisplayName("Fuel Tank Port".to_string()),
        fuel_tank.clone(),
        MountedOn {
            parent_entity_id: engine_left_guid,
            hardpoint_id: "fuel_supply".to_string(),
        },
        MassKg(1100.0),
        owner.clone(),
        shard,
    ));

    let engine_right_guid = Uuid::new_v4();
    let fuel_tank_right_guid = Uuid::new_v4();
    commands.spawn((
        EntityGuid(engine_right_guid),
        DisplayName("Engine Starboard".to_string()),
        engine.clone(),
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_right_aft".to_string(),
        },
        MassKg(500.0),
        owner.clone(),
        shard,
    ));
    commands.spawn((
        EntityGuid(fuel_tank_right_guid),
        DisplayName("Fuel Tank Starboard".to_string()),
        fuel_tank,
        MountedOn {
            parent_entity_id: engine_right_guid,
            hardpoint_id: "fuel_supply".to_string(),
        },
        MassKg(1100.0),
        owner,
        shard,
    ));

    CorvetteModuleGuids {
        flight_computer: flight_computer_guid,
        engine_left: engine_left_guid,
        engine_right: engine_right_guid,
        fuel_tank_left: fuel_tank_left_guid,
        fuel_tank_right: fuel_tank_right_guid,
    }
}

// -----------------------------------------------------------------------------
// Back-compat: CorvetteSpawnConfig (delegate to CorvetteOverrides)
// -----------------------------------------------------------------------------

/// Legacy spawn config; prefer `CorvetteOverrides` and `spawn_corvette(commands, overrides)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorvetteSpawnConfig {
    pub owner_account_id: Uuid,
    pub player_entity_id: String,
    pub spawn_position: Option<Vec3>,
    pub spawn_velocity: Vec3,
    pub shard_id: i32,
    pub display_name: Option<String>,
}

impl CorvetteSpawnConfig {
    pub fn get_spawn_position(&self) -> Vec3 {
        self.spawn_position
            .unwrap_or_else(|| corvette_random_spawn_position(self.owner_account_id))
    }
}

impl From<CorvetteSpawnConfig> for CorvetteOverrides {
    fn from(c: CorvetteSpawnConfig) -> Self {
        Self {
            owner_account_id: Some(c.owner_account_id),
            player_entity_id: Some(c.player_entity_id),
            shard_id: Some(c.shard_id),
            position: c.spawn_position,
            velocity: Some(c.spawn_velocity),
            display_name: c.display_name,
        }
    }
}
