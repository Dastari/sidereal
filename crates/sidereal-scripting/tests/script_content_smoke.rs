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
fn shared_bundle_registry_module_exposes_asteroid_field_v2_contract() {
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
    let asteroid_field = bundles
        .get::<Table>("asteroid.field")
        .expect("asteroid field bundle entry");

    assert_eq!(
        table_get_required_string(&asteroid_field, "bundle_class", "asteroid.field")
            .expect("bundle_class"),
        "world"
    );
    assert_eq!(
        table_get_required_string(&asteroid_field, "graph_records_script", "asteroid.field")
            .expect("graph_records_script"),
        "bundles/starter/asteroid_field.lua"
    );
    let required = table_get_required_string_list(
        &asteroid_field,
        "required_component_kinds",
        "asteroid.field",
    )
    .expect("required_component_kinds");
    assert!(required.contains(&"asteroid_field".to_string()));
    assert!(required.contains(&"asteroid_field_member".to_string()));
    assert!(required.contains(&"asteroid_resource_profile".to_string()));
}

#[test]
fn shared_asteroid_registry_exposes_starter_profiles() {
    let policy = LuaSandboxPolicy::from_env();
    let module =
        load_lua_module_from_root(&shared_scripts_root(), "asteroids/registry.lua", &policy)
            .expect("asteroid registry module");
    let field_profiles = module
        .root()
        .get::<Table>("field_profiles")
        .expect("field_profiles table");
    let first = field_profiles
        .get::<Table>(1)
        .expect("starter field profile");

    assert_eq!(
        table_get_required_string(&first, "field_profile_id", "asteroids/registry.lua")
            .expect("field_profile_id"),
        "asteroid.field.starter_belt"
    );
    assert_eq!(
        table_get_required_string(&first, "resource_profile_id", "asteroids/registry.lua")
            .expect("resource_profile_id"),
        "asteroid.resource.common_ore"
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
