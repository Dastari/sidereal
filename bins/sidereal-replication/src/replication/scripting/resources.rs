#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct ScriptCatalogResource {
    pub entries: Vec<ScriptCatalogEntry>,
    pub revision: u64,
    pub root_dir: String,
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct ScriptCatalogEntry {
    pub script_path: String,
    pub source: String,
    pub revision: u64,
    pub origin: String,
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct ScriptCatalogControlResource {
    pub reload_all_from_disk_requested: bool,
    pub last_reload_succeeded: bool,
    pub last_reload_message: String,
    pub last_reload_at_s: f64,
    pub last_persist_succeeded: bool,
    pub last_persist_message: String,
    pub last_persist_at_s: f64,
    pub startup_loaded_from_disk_fallback: bool,
    pub startup_status_message: String,
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct EntityRegistryResource {
    pub entries: Vec<EntityRegistryEntry>,
    pub revision: u64,
    pub script_path: String,
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct EntityRegistryEntry {
    pub entity_id: String,
    pub entity_class: String,
    pub graph_records_script: String,
    pub required_component_kinds: Vec<String>,
}

#[derive(Resource, Reflect, Debug, Clone, Default)]
#[reflect(Resource)]
pub struct AssetRegistryResource {
    pub entries: Vec<AssetRegistryEntry>,
    pub revision: u64,
    pub script_path: String,
}

#[derive(Reflect, Debug, Clone, Default)]
pub struct AssetRegistryEntry {
    pub asset_id: String,
    pub shader_family: Option<String>,
    pub source_path: String,
    pub content_type: String,
    pub dependencies: Vec<String>,
    pub bootstrap_required: bool,
}

#[derive(Resource, Debug, Clone, Default)]
struct ScriptCatalogSyncState {
    fingerprints_by_path: HashMap<String, u64>,
    next_revision: u64,
}

#[derive(Resource, Debug, Clone)]
struct AssetRegistrySyncState {
    registry_script_path: PathBuf,
    last_catalog_revision: u64,
}

#[derive(Resource, Debug, Clone)]
struct PlanetRegistrySyncState {
    registry_script_path: PathBuf,
    last_catalog_revision: u64,
}

#[derive(Resource, Debug, Clone)]
struct ShipModuleRegistrySyncState {
    registry_script_path: PathBuf,
    last_catalog_revision: u64,
}

#[derive(Resource, Debug, Clone)]
struct ShipRegistrySyncState {
    registry_script_path: PathBuf,
    last_catalog_revision: u64,
}

#[derive(Resource, Debug, Clone, Default)]
struct ScriptCatalogPersistenceState {
    last_persisted_catalog_revision: u64,
}

#[derive(Debug, Clone)]
struct ScriptCatalogLoadOutcome {
    catalog: ScriptCatalogResource,
    persisted_catalog_revision: u64,
    startup_loaded_from_disk_fallback: bool,
    startup_status_message: String,
}

const ENTITY_REGISTRY_SCRIPT_REL_PATH: &str = "bundles/bundle_registry.lua";
const ASSET_REGISTRY_SCRIPT_REL_PATH: &str = "assets/registry.lua";
