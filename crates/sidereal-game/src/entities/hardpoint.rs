//! Hardpoint entity archetypes and corvette defaults.

use bevy::prelude::*;
use uuid::Uuid;

use crate::{DisplayName, EntityGuid, Hardpoint, OwnerId, ParentGuid, ShardAssignment};

#[derive(Bundle, Debug, Clone)]
pub struct HardpointBundle {
    pub entity_guid: EntityGuid,
    pub hardpoint: Hardpoint,
    pub display_name: DisplayName,
    pub parent_guid: ParentGuid,
    pub owner_id: OwnerId,
    pub shard_assignment: ShardAssignment,
}

#[derive(Debug, Clone, Copy)]
pub struct HardpointSpec {
    pub hardpoint_id: &'static str,
    pub display_name: &'static str,
    pub offset_m: Vec3,
    pub local_rotation: Quat,
}

pub fn default_corvette_hardpoint_specs() -> [HardpointSpec; 5] {
    [
        HardpointSpec {
            hardpoint_id: "computer_core",
            display_name: "Computer Core Hardpoint",
            offset_m: Vec3::new(0.0, 0.0, -5.0),
            local_rotation: Quat::IDENTITY,
        },
        HardpointSpec {
            hardpoint_id: "engine_main_aft",
            display_name: "Engine Main Aft Hardpoint",
            offset_m: Vec3::new(0.0, -1.0, -10.0),
            local_rotation: Quat::IDENTITY,
        },
        HardpointSpec {
            hardpoint_id: "fuel_left",
            display_name: "Fuel Tank Left Hardpoint",
            offset_m: Vec3::new(-3.0, 1.5, -8.0),
            local_rotation: Quat::IDENTITY,
        },
        HardpointSpec {
            hardpoint_id: "fuel_right",
            display_name: "Fuel Tank Right Hardpoint",
            offset_m: Vec3::new(3.0, 1.5, -8.0),
            local_rotation: Quat::IDENTITY,
        },
        HardpointSpec {
            hardpoint_id: "weapon_fore_center",
            display_name: "Weapon Fore Center Hardpoint",
            offset_m: Vec3::new(0.0, 10.0, -10.0),
            local_rotation: Quat::IDENTITY,
        },
    ]
}

pub fn spawn_hardpoint(
    commands: &mut Commands<'_, '_>,
    ship_guid: Uuid,
    owner_id: OwnerId,
    shard_assignment: ShardAssignment,
    spec: HardpointSpec,
) -> Uuid {
    let hardpoint_guid = Uuid::new_v4();
    commands.spawn(HardpointBundle {
        entity_guid: EntityGuid(hardpoint_guid),
        hardpoint: Hardpoint {
            hardpoint_id: spec.hardpoint_id.to_string(),
            offset_m: spec.offset_m,
            local_rotation: spec.local_rotation,
        },
        display_name: DisplayName(spec.display_name.to_string()),
        parent_guid: ParentGuid(ship_guid),
        owner_id,
        shard_assignment,
    });
    hardpoint_guid
}
