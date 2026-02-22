use bevy::prelude::*;
use bevy::reflect::Reflect;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, Default, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Hash,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EntityGuid(pub Uuid);

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct DisplayName(pub String);

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ShardAssignment(pub i32);

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct Hardpoint {
    pub hardpoint_id: String,
    pub offset_m: Vec3,
}

#[derive(Debug, Clone, Default, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct MountedOn {
    pub parent_entity_id: Uuid,
    pub hardpoint_id: String,
}

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid, MountedOn)]
pub struct Engine {
    #[serde(alias = "thrust_n")]
    pub thrust: f32,
    #[serde(default, alias = "reverse_thrust_n")]
    pub reverse_thrust: f32,
    #[serde(default, alias = "torque_thrust_nm")]
    pub torque_thrust: f32,
    pub burn_rate_kg_s: f32,
}

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FuelTank {
    pub fuel_kg: f32,
}

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FlightComputer {
    /// Control profile (e.g., "basic_fly_by_wire", "combat_agile", "missile_guidance")
    pub profile: String,
    /// Current throttle setting (-1.0 to 1.0)
    pub throttle: f32,
    /// Current yaw input (-1.0 to 1.0)
    pub yaw_input: f32,
    /// Explicit brake intent; avoids overloading throttle with sentinel values.
    #[serde(default)]
    pub brake_active: bool,
    /// Turn rate in degrees per second
    pub turn_rate_deg_s: f32,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FlightTuning {
    pub max_linear_accel_mps2: f32,
    pub passive_brake_accel_mps2: f32,
    pub active_brake_accel_mps2: f32,
    pub drag_per_s: f32,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct MaxVelocityMps(pub f32);

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct HealthPool {
    pub current: f32,
    pub maximum: f32,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct MassKg(pub f32);

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct SizeM {
    pub length: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct CollisionAabbM {
    pub half_extents: Vec3,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ShipTag;

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ModuleTag;

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FullscreenLayer {
    pub layer_kind: String,
    pub shader_asset_id: String,
    pub layer_order: i32,
}

#[derive(Debug, Clone, Copy, Default, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Serialize, Deserialize)]
pub enum OwnerKind {
    Player,
    Faction,
    World,
    #[default]
    Unowned,
}

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct OwnerId(pub String);

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Eq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FactionId(pub String);

#[derive(
    Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default,
)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct FactionVisibility;

#[derive(
    Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default,
)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct PublicVisibility;

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ScannerRangeM(pub f32);

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ScannerComponent {
    pub base_range_m: f32,
    pub level: u8,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ScannerRangeBuff {
    pub additive_m: f32,
    pub multiplier: f32,
}

#[derive(Debug, Clone, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Serialize, Deserialize)]
pub struct InventoryEntry {
    pub item_entity_id: Uuid,
    pub quantity: u32,
    pub unit_mass_kg: f32,
}

#[derive(Debug, Clone, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct Inventory {
    pub entries: Vec<InventoryEntry>,
}

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct BaseMassKg(pub f32);

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct CargoMassKg(pub f32);

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Default)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct ModuleMassKg(pub f32);

#[derive(Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct TotalMassKg(pub f32);

#[derive(
    Debug, Clone, Copy, Component, Reflect, Serialize, Deserialize, PartialEq, Eq, Default,
)]
#[reflect(Component, Serialize, Deserialize)]
#[require(EntityGuid)]
pub struct MassDirty;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentRegistryEntry {
    pub component_kind: &'static str,
    pub type_path: &'static str,
    pub replication_visibility: ReplicationVisibility,
}

#[derive(Debug, Resource, Clone)]
pub struct GeneratedComponentRegistry {
    pub entries: Vec<ComponentRegistryEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplicationVisibility {
    Public,
    OwnerOnly,
}

pub fn register_generated_components(app: &mut App) {
    app.register_type::<EntityGuid>()
        .register_type::<DisplayName>()
        .register_type::<ShardAssignment>()
        .register_type::<Hardpoint>()
        .register_type::<MountedOn>()
        .register_type::<Engine>()
        .register_type::<FuelTank>()
        .register_type::<FlightComputer>()
        .register_type::<FlightTuning>()
        .register_type::<MaxVelocityMps>()
        .register_type::<HealthPool>()
        .register_type::<MassKg>()
        .register_type::<SizeM>()
        .register_type::<CollisionAabbM>()
        .register_type::<ShipTag>()
        .register_type::<ModuleTag>()
        .register_type::<FullscreenLayer>()
        .register_type::<OwnerKind>()
        .register_type::<ScannerRangeM>()
        .register_type::<ScannerComponent>()
        .register_type::<ScannerRangeBuff>()
        .register_type::<InventoryEntry>()
        .register_type::<Inventory>()
        .register_type::<BaseMassKg>()
        .register_type::<CargoMassKg>()
        .register_type::<ModuleMassKg>()
        .register_type::<TotalMassKg>()
        .register_type::<MassDirty>()
        .register_type::<OwnerId>()
        .register_type::<FactionId>()
        .register_type::<FactionVisibility>()
        .register_type::<PublicVisibility>()
        .insert_resource(GeneratedComponentRegistry {
            entries: generated_component_registry(),
        });
}

pub fn generated_component_registry() -> Vec<ComponentRegistryEntry> {
    vec![
        entry::<EntityGuid>("entity_guid", ReplicationVisibility::Public),
        entry::<DisplayName>("display_name", ReplicationVisibility::Public),
        entry::<ShardAssignment>("shard_assignment", ReplicationVisibility::OwnerOnly),
        entry::<Hardpoint>("hardpoint", ReplicationVisibility::Public),
        entry::<MountedOn>("mounted_on", ReplicationVisibility::Public),
        entry::<Engine>("engine", ReplicationVisibility::OwnerOnly),
        entry::<FuelTank>("fuel_tank", ReplicationVisibility::OwnerOnly),
        entry::<FlightComputer>("flight_computer", ReplicationVisibility::OwnerOnly),
        entry::<FlightTuning>("flight_tuning", ReplicationVisibility::OwnerOnly),
        entry::<MaxVelocityMps>("max_velocity_mps", ReplicationVisibility::Public),
        entry::<HealthPool>("health_pool", ReplicationVisibility::OwnerOnly),
        entry::<MassKg>("mass_kg", ReplicationVisibility::Public),
        entry::<SizeM>("size_m", ReplicationVisibility::Public),
        entry::<CollisionAabbM>("collision_aabb_m", ReplicationVisibility::Public),
        entry::<ShipTag>("ship_tag", ReplicationVisibility::Public),
        entry::<ModuleTag>("module_tag", ReplicationVisibility::Public),
        entry::<FullscreenLayer>("fullscreen_layer", ReplicationVisibility::Public),
        entry::<ScannerRangeM>("scanner_range_m", ReplicationVisibility::Public),
        entry::<ScannerComponent>("scanner_component", ReplicationVisibility::OwnerOnly),
        entry::<ScannerRangeBuff>("scanner_range_buff", ReplicationVisibility::OwnerOnly),
        entry::<Inventory>("inventory", ReplicationVisibility::OwnerOnly),
        entry::<BaseMassKg>("base_mass_kg", ReplicationVisibility::Public),
        entry::<CargoMassKg>("cargo_mass_kg", ReplicationVisibility::OwnerOnly),
        entry::<ModuleMassKg>("module_mass_kg", ReplicationVisibility::Public),
        entry::<TotalMassKg>("total_mass_kg", ReplicationVisibility::Public),
        entry::<MassDirty>("mass_dirty", ReplicationVisibility::OwnerOnly),
        entry::<OwnerId>("owner_id", ReplicationVisibility::OwnerOnly),
        entry::<FactionId>("faction_id", ReplicationVisibility::Public),
        entry::<FactionVisibility>("faction_visibility", ReplicationVisibility::Public),
        entry::<PublicVisibility>("public_visibility", ReplicationVisibility::Public),
    ]
}

fn entry<T>(
    component_kind: &'static str,
    replication_visibility: ReplicationVisibility,
) -> ComponentRegistryEntry {
    ComponentRegistryEntry {
        component_kind,
        type_path: std::any::type_name::<T>(),
        replication_visibility,
    }
}
