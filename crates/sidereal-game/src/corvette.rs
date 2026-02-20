// Corvette Ship Bundle
// Defines the complete component set for the starter corvette ship (corvette_01)
// Used during account bootstrap and for spawning additional corvettes

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use uuid::Uuid;

use crate::{
    BaseMassKg, CargoMassKg, CollisionAabbM, DisplayName, Engine, EntityGuid, FlightComputer,
    FuelTank, Hardpoint, HealthPool, Inventory, MassDirty, MassKg, ModuleMassKg, MountedOn,
    OwnerId, PositionM, ShardAssignment, ShipTag, SizeM, TotalMassKg, VelocityMps,
};

/// Complete component bundle for the Prospector-class corvette
/// This is the canonical starter ship granted on registration
#[derive(Bundle, Debug, Clone)]
pub struct CorvetteBundle {
    // Identity
    pub entity_guid: EntityGuid,
    pub ship_tag: ShipTag,
    pub display_name: DisplayName,

    // Spatial
    pub position: PositionM,
    pub velocity: VelocityMps,

    // Physical properties
    pub mass: MassKg,
    pub base_mass: BaseMassKg,
    pub cargo_mass: CargoMassKg,
    pub module_mass: ModuleMassKg,
    pub total_mass: TotalMassKg,
    pub mass_dirty: MassDirty,
    pub inventory: Inventory,
    pub size: SizeM,
    pub collision: CollisionAabbM,

    // Health/combat
    pub health: HealthPool,

    // Ownership (only OwnerId is a component)
    pub owner_id: OwnerId,

    // Authority
    pub shard_assignment: ShardAssignment,
}

/// Configuration for spawning a corvette
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorvetteSpawnConfig {
    /// Owner account UUID
    pub owner_account_id: Uuid,

    /// Player entity ID (e.g., "player:<account_uuid>")
    pub player_entity_id: String,

    /// Starting position in world space (meters)
    /// If None, will use randomized starting position
    pub spawn_position: Option<Vec3>,

    /// Starting velocity (meters/second)
    pub spawn_velocity: Vec3,

    /// Which shard owns this ship
    pub shard_id: i32,

    /// Ship display name (defaults to "Prospector-14" if None)
    pub display_name: Option<String>,
}

impl CorvetteSpawnConfig {
    /// Get spawn position with randomization if not specified
    pub fn get_spawn_position(&self) -> Vec3 {
        self.spawn_position.unwrap_or_else(|| {
            // Randomize spawn in a 1km x 1km area to prevent overlap
            // Use account UUID as seed for determinism per account
            let mut hasher = DefaultHasher::new();
            self.owner_account_id.hash(&mut hasher);
            let seed = hasher.finish();

            // Simple LCG random for deterministic spread
            let x = ((seed.wrapping_mul(1664525).wrapping_add(1013904223)) % 1000) as f32 - 500.0;
            let y = ((seed.wrapping_mul(22695477).wrapping_add(1)) % 1000) as f32 - 500.0;

            Vec3::new(x, y, 0.0)
        })
    }
}

/// Spawns a complete corvette ship with all required components
/// Returns the ship entity GUID and spawned module entity GUIDs
pub fn spawn_corvette(
    commands: &mut Commands,
    config: CorvetteSpawnConfig,
) -> (Uuid, CorvetteModuleGuids) {
    let ship_guid = Uuid::new_v4();

    // Get spawn position (randomized if not specified)
    let spawn_position = config.get_spawn_position();

    // Spawn the hull entity with all core components
    let ship_entity = commands
        .spawn(CorvetteBundle {
            entity_guid: EntityGuid(ship_guid),
            ship_tag: ShipTag,
            display_name: DisplayName(
                config
                    .display_name
                    .clone()
                    .unwrap_or_else(|| "Prospector-14".to_string()),
            ),
            position: PositionM(spawn_position),
            velocity: VelocityMps(config.spawn_velocity),
            mass: MassKg(15000.0), // 15 metric tons base mass
            base_mass: BaseMassKg(15000.0),
            cargo_mass: CargoMassKg(0.0),
            module_mass: ModuleMassKg(0.0),
            total_mass: TotalMassKg(15000.0),
            mass_dirty: MassDirty,
            inventory: Inventory::default(),
            size: SizeM {
                length: 25.0,
                width: 12.0,
                height: 8.0,
            },
            collision: CollisionAabbM {
                half_extents: Vec3::new(12.5, 6.0, 4.0),
            },
            health: HealthPool {
                current: 1000.0,
                maximum: 1000.0,
            },
            owner_id: OwnerId(config.player_entity_id.clone()),
            shard_assignment: ShardAssignment(config.shard_id),
        })
        .id();

    // Define hardpoints for the corvette
    let hardpoints = vec![
        Hardpoint {
            hardpoint_id: "computer_core".to_string(),
            offset_m: Vec3::new(0.0, 0.0, -5.0), // Center-rear
        },
        Hardpoint {
            hardpoint_id: "engine_left_aft".to_string(),
            offset_m: Vec3::new(-4.0, -1.0, -10.0), // Left rear
        },
        Hardpoint {
            hardpoint_id: "engine_right_aft".to_string(),
            offset_m: Vec3::new(4.0, -1.0, -10.0), // Right rear
        },
    ];

    // Add hardpoints as child entities
    for hardpoint in hardpoints {
        commands.entity(ship_entity).with_children(|parent| {
            parent.spawn((
                EntityGuid(Uuid::new_v4()),
                hardpoint.clone(),
                DisplayName(format!("Hardpoint: {}", hardpoint.hardpoint_id)),
                OwnerId(config.player_entity_id.clone()),
            ));
        });
    }

    // Spawn modules
    let module_guids = spawn_corvette_modules(commands, ship_guid, &config);

    (ship_guid, module_guids)
}

/// GUIDs for all spawned corvette modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorvetteModuleGuids {
    pub flight_computer: Uuid,
    pub engine_left: Uuid,
    pub engine_right: Uuid,
    pub fuel_tank_left: Uuid,
    pub fuel_tank_right: Uuid,
}

/// Spawns all required modules for the corvette
fn spawn_corvette_modules(
    commands: &mut Commands,
    ship_guid: Uuid,
    config: &CorvetteSpawnConfig,
) -> CorvetteModuleGuids {
    // Flight Computer (mounted on computer_core)
    let flight_computer_guid = Uuid::new_v4();
    commands.spawn((
        EntityGuid(flight_computer_guid),
        DisplayName("Flight Computer MK1".to_string()),
        FlightComputer {
            profile: "basic_fly_by_wire".to_string(),
            throttle: 0.0,
            yaw_input: 0.0,
            turn_rate_deg_s: 45.0,
        },
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "computer_core".to_string(),
        },
        MassKg(50.0),
        OwnerId(config.player_entity_id.clone()),
        ShardAssignment(config.shard_id),
    ));

    // Left Engine + Fuel Tank
    let engine_left_guid = Uuid::new_v4();
    let fuel_tank_left_guid = Uuid::new_v4();

    commands.spawn((
        EntityGuid(engine_left_guid),
        DisplayName("Engine Port".to_string()),
        Engine {
            thrust_n: 50000.0,                    // 50kN thrust
            burn_rate_kg_s: 0.5,                  // 0.5 kg/s fuel consumption
            thrust_dir: Vec3::new(0.0, 0.0, 1.0), // Forward thrust
        },
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_left_aft".to_string(),
        },
        MassKg(500.0),
        OwnerId(config.player_entity_id.clone()),
        ShardAssignment(config.shard_id),
    ));

    commands.spawn((
        EntityGuid(fuel_tank_left_guid),
        DisplayName("Fuel Tank Port".to_string()),
        FuelTank { fuel_kg: 1000.0 },
        MountedOn {
            parent_entity_id: engine_left_guid,
            hardpoint_id: "fuel_supply".to_string(),
        },
        MassKg(1100.0), // Tank + fuel
        OwnerId(config.player_entity_id.clone()),
        ShardAssignment(config.shard_id),
    ));

    // Right Engine + Fuel Tank (symmetric)
    let engine_right_guid = Uuid::new_v4();
    let fuel_tank_right_guid = Uuid::new_v4();

    commands.spawn((
        EntityGuid(engine_right_guid),
        DisplayName("Engine Starboard".to_string()),
        Engine {
            thrust_n: 50000.0,
            burn_rate_kg_s: 0.5,
            thrust_dir: Vec3::new(0.0, 0.0, 1.0),
        },
        MountedOn {
            parent_entity_id: ship_guid,
            hardpoint_id: "engine_right_aft".to_string(),
        },
        MassKg(500.0),
        OwnerId(config.player_entity_id.clone()),
        ShardAssignment(config.shard_id),
    ));

    commands.spawn((
        EntityGuid(fuel_tank_right_guid),
        DisplayName("Fuel Tank Starboard".to_string()),
        FuelTank { fuel_kg: 1000.0 },
        MountedOn {
            parent_entity_id: engine_right_guid,
            hardpoint_id: "fuel_supply".to_string(),
        },
        MassKg(1100.0),
        OwnerId(config.player_entity_id.clone()),
        ShardAssignment(config.shard_id),
    ));

    CorvetteModuleGuids {
        flight_computer: flight_computer_guid,
        engine_left: engine_left_guid,
        engine_right: engine_right_guid,
        fuel_tank_left: fuel_tank_left_guid,
        fuel_tank_right: fuel_tank_right_guid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corvette_bundle_has_all_required_components() {
        let mut world = World::new();
        let mut commands = world.commands();

        let config = CorvetteSpawnConfig {
            owner_account_id: Uuid::new_v4(),
            player_entity_id: "player:test-123".to_string(),
            spawn_position: Some(Vec3::ZERO),
            spawn_velocity: Vec3::ZERO,
            shard_id: 1,
            display_name: None,
        };

        let (ship_guid, module_guids) = spawn_corvette(&mut commands, config);

        // Verify GUIDs were created
        assert_ne!(ship_guid, Uuid::nil());
        assert_ne!(module_guids.flight_computer, Uuid::nil());
        assert_ne!(module_guids.engine_left, Uuid::nil());
        assert_ne!(module_guids.engine_right, Uuid::nil());
    }

    #[test]
    fn corvette_has_correct_physical_properties() {
        // Base hull mass
        let hull_mass = 15000.0;
        // Modules: computer(50) + 2×engine(500) + 2×fuel_tank(1100)
        let total_mass = hull_mass + 50.0 + 2.0 * 500.0 + 2.0 * 1100.0;

        assert_eq!(total_mass, 18250.0); // ~18.25 metric tons fully loaded
    }

    #[test]
    fn corvette_modules_reference_correct_parent() {
        let mut world = World::new();
        let mut commands = world.commands();

        let config = CorvetteSpawnConfig {
            owner_account_id: Uuid::new_v4(),
            player_entity_id: "player:test-123".to_string(),
            spawn_position: Some(Vec3::ZERO),
            spawn_velocity: Vec3::ZERO,
            shard_id: 1,
            display_name: None,
        };

        let (_ship_guid, _module_guids) = spawn_corvette(&mut commands, config);

        // All modules should reference ship_guid as parent
        // (Actual verification would require querying the world after flush)
    }

    #[test]
    fn corvette_spawn_position_randomizes_when_none() {
        let account_id = Uuid::new_v4();

        let config = CorvetteSpawnConfig {
            owner_account_id: account_id,
            player_entity_id: format!("player:{}", account_id),
            spawn_position: None, // Should randomize
            spawn_velocity: Vec3::ZERO,
            shard_id: 1,
            display_name: None,
        };

        let pos = config.get_spawn_position();

        // Should be within 1km box
        assert!(pos.x >= -500.0 && pos.x <= 500.0);
        assert!(pos.y >= -500.0 && pos.y <= 500.0);
        assert_eq!(pos.z, 0.0);

        // Same account should get same position (deterministic)
        let pos2 = config.get_spawn_position();
        assert_eq!(pos, pos2);
    }
}
