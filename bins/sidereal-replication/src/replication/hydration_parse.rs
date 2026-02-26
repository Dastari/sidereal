use sidereal_game::{
    BaseMassKg, CargoMassKg, Engine, FactionId, FlightComputer, FlightTuning, FuelTank, Hardpoint,
    HealthPool, Inventory, MassKg, MaxVelocityMps, ModuleMassKg, MountedOn, OwnerId,
    ScannerComponent, ScannerRangeBuff, ScannerRangeM, SizeM, SpriteShaderAssetId, TotalMassKg,
    VisualAssetId,
};
use sidereal_runtime_sync::{component_record, decode_graph_component_payload};
use std::collections::HashMap;

pub fn owner_id_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<OwnerId> {
    let component = component_record(&record.components, "owner_id")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<OwnerId>(payload.clone()).ok()
}

pub fn faction_id_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<FactionId> {
    let component = component_record(&record.components, "faction_id")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<FactionId>(payload.clone()).ok()
}

pub fn has_marker_component_record(
    record: &sidereal_persistence::GraphEntityRecord,
    component_kind: &str,
) -> bool {
    component_record(&record.components, component_kind).is_some()
}

pub fn health_pool_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<HealthPool> {
    let component = component_record(&record.components, "health_pool")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<HealthPool>(payload.clone()).ok()
}

pub fn flight_computer_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<FlightComputer> {
    let component = component_record(&record.components, "flight_computer")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<FlightComputer>(payload.clone()).ok()
}

pub fn flight_tuning_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<FlightTuning> {
    let component = component_record(&record.components, "flight_tuning")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<FlightTuning>(payload.clone()).ok()
}

pub fn max_velocity_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<MaxVelocityMps> {
    let component = component_record(&record.components, "max_velocity_mps")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<MaxVelocityMps>(payload.clone()).ok()
}

pub fn mounted_on_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<MountedOn> {
    let component = component_record(&record.components, "mounted_on")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<MountedOn>(payload.clone()).ok()
}

pub fn hardpoint_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<Hardpoint> {
    let component = component_record(&record.components, "hardpoint")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<Hardpoint>(payload.clone()).ok()
}

pub fn engine_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<Engine> {
    let component = component_record(&record.components, "engine")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<Engine>(payload.clone()).ok()
}

pub fn fuel_tank_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<FuelTank> {
    let component = component_record(&record.components, "fuel_tank")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<FuelTank>(payload.clone()).ok()
}

pub fn mass_kg_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<MassKg> {
    let component = component_record(&record.components, "mass_kg")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<MassKg>(payload.clone()).ok()
}

pub fn base_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<BaseMassKg> {
    let component = component_record(&record.components, "base_mass_kg")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<BaseMassKg>(payload.clone()).ok()
}

pub fn cargo_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<CargoMassKg> {
    let component = component_record(&record.components, "cargo_mass_kg")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<CargoMassKg>(payload.clone()).ok()
}

pub fn module_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ModuleMassKg> {
    let component = component_record(&record.components, "module_mass_kg")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<ModuleMassKg>(payload.clone()).ok()
}

pub fn total_mass_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<TotalMassKg> {
    let component = component_record(&record.components, "total_mass_kg")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<TotalMassKg>(payload.clone()).ok()
}

pub fn inventory_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<Inventory> {
    let component = component_record(&record.components, "inventory")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<Inventory>(payload.clone()).ok()
}

pub fn scanner_range_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ScannerRangeM> {
    if let Some(component) = component_record(&record.components, "scanner_range_m") {
        let payload = decode_graph_component_payload(component, type_paths)?;
        if let Ok(value) = serde_json::from_value::<ScannerRangeM>(payload.clone()) {
            return Some(value);
        }
        if let Some(value) = payload.as_f64() {
            return Some(ScannerRangeM(value as f32));
        }
    }
    record
        .properties
        .get("scanner_range_m")
        .and_then(|v| v.as_f64())
        .map(|v| ScannerRangeM(v as f32))
}

pub fn scanner_component_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ScannerComponent> {
    let component = component_record(&record.components, "scanner_component")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<ScannerComponent>(payload.clone()).ok()
}

pub fn scanner_range_buff_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<ScannerRangeBuff> {
    let component = component_record(&record.components, "scanner_range_buff")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<ScannerRangeBuff>(payload.clone()).ok()
}

pub fn size_m_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<SizeM> {
    let component = component_record(&record.components, "size_m")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<SizeM>(payload.clone()).ok()
}

pub fn visual_asset_id_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<VisualAssetId> {
    let component = component_record(&record.components, "visual_asset_id")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<VisualAssetId>(payload.clone()).ok()
}

pub fn sprite_shader_asset_id_from_record(
    record: &sidereal_persistence::GraphEntityRecord,
    type_paths: &HashMap<String, String>,
) -> Option<SpriteShaderAssetId> {
    let component = component_record(&record.components, "sprite_shader_asset_id")?;
    let payload = decode_graph_component_payload(component, type_paths)?;
    serde_json::from_value::<SpriteShaderAssetId>(payload.clone()).ok()
}
