use mlua::{Function, Table};
use sidereal_scripting::{
    LuaSandboxPolicy, load_asset_registry_from_root, load_lua_module_from_root,
    resolve_scripts_root, table_get_required_string, table_get_required_string_list,
};

fn shared_scripts_root() -> std::path::PathBuf {
    resolve_scripts_root(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn shared_assets_registry_loads_from_workspace_scripts() {
    let registry = load_asset_registry_from_root(&shared_scripts_root()).expect("asset registry");
    assert!(
        registry
            .assets
            .iter()
            .any(|asset| asset.asset_id == "starfield_wgsl")
    );
    assert!(
        registry
            .assets
            .iter()
            .any(|asset| asset.asset_id == "space_background_base_wgsl")
    );
}

#[test]
fn shared_world_init_module_exposes_expected_contract() {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(&shared_scripts_root(), "world/world_init.lua", &policy)
        .expect("world init module");
    let world_defaults = module
        .root()
        .get::<Table>("world_defaults")
        .expect("world_defaults table");
    let render_layer_definitions = world_defaults
        .get::<Table>("render_layer_definitions")
        .expect("render_layer_definitions table");
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .expect("build_graph_records function");

    assert!(render_layer_definitions.raw_len() >= 3);
    assert_eq!(build_graph_records.info().name, None);
}

#[test]
fn shared_bundle_registry_module_exposes_corvette_bundle_contract() {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(
        &shared_scripts_root(),
        "bundles/bundle_registry.lua",
        &policy,
    )
    .expect("bundle registry module");
    let bundles = module
        .root()
        .get::<Table>("bundles")
        .expect("bundles table");
    let corvette = bundles
        .get::<Table>("ship.corvette")
        .expect("corvette bundle entry");

    assert_eq!(
        table_get_required_string(&corvette, "bundle_class", "ship.corvette")
            .expect("bundle_class"),
        "ship"
    );
    assert_eq!(
        table_get_required_string(&corvette, "graph_records_script", "ship.corvette")
            .expect("graph_records_script"),
        "bundles/ship/corvette.lua"
    );
    assert!(
        table_get_required_string_list(&corvette, "required_component_kinds", "ship.corvette")
            .expect("required_component_kinds")
            .contains(&"visibility_range_buff_m".to_string())
    );
}

#[test]
fn shared_player_init_module_exposes_expected_entrypoint() {
    let policy = LuaSandboxPolicy::from_env();
    let module =
        load_lua_module_from_root(&shared_scripts_root(), "accounts/player_init.lua", &policy)
            .expect("player init module");
    let player_init = module
        .root()
        .get::<Function>("player_init")
        .expect("player_init function");

    assert_eq!(player_init.info().name, None);
}
