use crate::auth::error::AuthError;
use mlua::{Function, Table, Value};
use sidereal_game::{default_corvette_collision_outline, generated_component_registry};
use sidereal_persistence::GraphEntityRecord;
use sidereal_scripting::{
    LuaSandboxPolicy, ScriptError, load_lua_module_from_root, lua_value_to_json,
    resolve_scripts_root, table_get_optional_string, table_get_required_string,
    table_get_required_string_list, validate_component_kinds,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldInitScriptConfig {
    pub space_background_shader_asset_id: String,
    pub starfield_shader_asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAccountScriptConfig {
    pub starter_bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBundleDefinition {
    pub bundle_id: String,
    pub spawn_template: Option<String>,
    pub graph_records_script: Option<String>,
    pub required_component_kinds: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptBundleRegistry {
    pub bundles: HashMap<String, ScriptBundleDefinition>,
}

#[derive(Debug, Clone)]
pub struct ScriptContext<'a> {
    pub account_id: Uuid,
    pub player_entity_id: &'a str,
    pub email: &'a str,
}

pub fn scripts_root_dir() -> PathBuf {
    let resolved = resolve_scripts_root(env!("CARGO_MANIFEST_DIR"));
    info!("gateway scripting root resolved to {}", resolved.display());
    resolved
}

pub fn load_world_init_config(root: &Path) -> Result<WorldInitScriptConfig, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "world/world_init.lua", &policy)
        .map_err(map_script_error)?;
    let world_defaults = module
        .root()
        .get::<Table>("world_defaults")
        .map_err(|err| AuthError::Internal(format!("world/world_init.lua: {err}")))?;
    let space_background_shader_asset_id = table_get_required_string(
        &world_defaults,
        "space_background_shader_asset_id",
        "world_defaults",
    )
    .map_err(map_script_error)?;
    let starfield_shader_asset_id = table_get_required_string(
        &world_defaults,
        "starfield_shader_asset_id",
        "world_defaults",
    )
    .map_err(map_script_error)?;
    Ok(WorldInitScriptConfig {
        space_background_shader_asset_id,
        starfield_shader_asset_id,
    })
    .inspect(|config| {
        info!(
            "gateway loaded world init config: space_background_shader_asset_id={} starfield_shader_asset_id={}",
            config.space_background_shader_asset_id, config.starfield_shader_asset_id
        );
    })
}

pub fn load_world_init_graph_records(root: &Path) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "world/world_init.lua", &policy)
        .map_err(map_script_error)?;
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .map_err(|err| {
            AuthError::Internal(format!(
                "{}: missing build_graph_records(ctx): {err}",
                module.script_path().display()
            ))
        })?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_script_context(ctx.clone(), &module, None)?;

    let lua_value = build_graph_records
        .call::<Value>(ctx)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_error)?;
    let records = serde_json::from_value::<Vec<GraphEntityRecord>>(json_value).map_err(|err| {
        AuthError::Internal(format!(
            "{}: build_graph_records(ctx) must return Vec<GraphEntityRecord>-compatible structure: {err}",
            module.script_path().display()
        ))
    })?;
    if records.is_empty() {
        return Err(AuthError::Internal(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        )));
    }
    Ok(records)
}

pub fn load_new_account_config(
    root: &Path,
    context: ScriptContext<'_>,
) -> Result<NewAccountScriptConfig, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "accounts/on_new_account.lua", &policy)
        .map_err(map_script_error)?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| AuthError::Internal(format!("accounts/on_new_account.lua: {err}")))?;
    inject_script_context(ctx.clone(), &module, Some(&context))?;

    let on_new_account = module
        .root()
        .get::<Function>("on_new_account")
        .map_err(|err| AuthError::Internal(format!("accounts/on_new_account.lua: {err}")))?;
    let response = on_new_account
        .call::<Value>(ctx)
        .map_err(|err| AuthError::Internal(format!("accounts/on_new_account.lua: {err}")))?;
    let table = match response {
        Value::Table(table) => table,
        _ => {
            return Err(AuthError::Internal(
                "accounts/on_new_account.lua: on_new_account(ctx) must return a table".to_string(),
            ));
        }
    };
    let starter_bundle_id =
        table_get_required_string(&table, "starter_bundle_id", "on_new_account")
            .map_err(map_script_error)?;
    Ok(NewAccountScriptConfig { starter_bundle_id }).inspect(|config| {
        info!(
            "gateway new-account script selected starter_bundle_id={} for account_id={} player_entity_id={}",
            config.starter_bundle_id, context.account_id, context.player_entity_id
        );
    })
}

pub fn load_bundle_registry(root: &Path) -> Result<ScriptBundleRegistry, AuthError> {
    let policy = LuaSandboxPolicy::from_env();
    let module = load_lua_module_from_root(root, "bundles/bundle_registry.lua", &policy)
        .map_err(map_script_error)?;
    let bundles_table = module
        .root()
        .get::<Table>("bundles")
        .map_err(|err| AuthError::Internal(format!("bundles/bundle_registry.lua: {err}")))?;

    let mut bundles = HashMap::<String, ScriptBundleDefinition>::new();
    for pair in bundles_table.pairs::<String, Table>() {
        let (bundle_id, bundle_table) =
            pair.map_err(|err| AuthError::Internal(format!("bundle registry read failed: {err}")))?;
        let spawn_template = table_get_optional_string(&bundle_table, "spawn_template", &bundle_id)
            .map_err(map_script_error)?;
        let graph_records_script =
            table_get_optional_string(&bundle_table, "graph_records_script", &bundle_id)
                .map_err(map_script_error)?;
        let required_component_kinds =
            table_get_required_string_list(&bundle_table, "required_component_kinds", &bundle_id)
                .map_err(map_script_error)?;
        if spawn_template.is_none() && graph_records_script.is_none() {
            return Err(AuthError::Internal(format!(
                "bundles/bundle_registry.lua: bundle={} must define either spawn_template or graph_records_script",
                bundle_id
            )));
        }
        if spawn_template.is_some() && graph_records_script.is_some() {
            return Err(AuthError::Internal(format!(
                "bundles/bundle_registry.lua: bundle={} must not define both spawn_template and graph_records_script",
                bundle_id
            )));
        }
        bundles.insert(
            bundle_id.clone(),
            ScriptBundleDefinition {
                bundle_id,
                spawn_template,
                graph_records_script,
                required_component_kinds,
            },
        );
    }

    if bundles.is_empty() {
        return Err(AuthError::Internal(
            "bundles/bundle_registry.lua: bundles table must not be empty".to_string(),
        ));
    }

    validate_bundle_registry_component_kinds(&bundles)?;
    info!(
        "gateway loaded bundle registry with {} bundle(s)",
        bundles.len()
    );
    Ok(ScriptBundleRegistry { bundles })
}

pub fn load_graph_records_for_bundle(
    root: &Path,
    bundle: &ScriptBundleDefinition,
    context: ScriptContext<'_>,
) -> Result<Vec<GraphEntityRecord>, AuthError> {
    let Some(script_rel_path) = bundle.graph_records_script.as_ref() else {
        return Err(AuthError::Internal(format!(
            "bundle {} has no graph_records_script",
            bundle.bundle_id
        )));
    };
    info!(
        "gateway loading graph records script={} for bundle_id={} account_id={} player_entity_id={}",
        script_rel_path, bundle.bundle_id, context.account_id, context.player_entity_id
    );

    let policy = LuaSandboxPolicy::from_env();
    let module =
        load_lua_module_from_root(root, script_rel_path, &policy).map_err(map_script_error)?;
    let build_graph_records = module
        .root()
        .get::<Function>("build_graph_records")
        .map_err(|err| {
            AuthError::Internal(format!(
                "{}: missing build_graph_records(ctx): {err}",
                module.script_path().display()
            ))
        })?;
    let ctx = module
        .root()
        .get::<Table>("context")
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    inject_script_context(ctx.clone(), &module, Some(&context))?;

    let lua_value = build_graph_records
        .call::<Value>(ctx)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    let json_value = lua_value_to_json(lua_value).map_err(map_script_error)?;
    let records = serde_json::from_value::<Vec<GraphEntityRecord>>(json_value).map_err(|err| {
        AuthError::Internal(format!(
            "{}: build_graph_records(ctx) must return Vec<GraphEntityRecord>-compatible structure: {err}",
            module.script_path().display()
        ))
    })?;
    if records.is_empty() {
        return Err(AuthError::Internal(format!(
            "{}: build_graph_records(ctx) returned empty records",
            module.script_path().display()
        )));
    }
    validate_graph_records_component_kinds(bundle, &records)?;
    Ok(records)
}

fn validate_bundle_registry_component_kinds(
    bundles: &HashMap<String, ScriptBundleDefinition>,
) -> Result<(), AuthError> {
    let known_component_kinds = generated_component_registry()
        .into_iter()
        .map(|entry| entry.component_kind.to_string())
        .collect::<HashSet<_>>();

    for (bundle_id, bundle) in bundles {
        validate_component_kinds(
            &known_component_kinds,
            &bundle.required_component_kinds,
            &format!("bundles/bundle_registry.lua: bundle={bundle_id}"),
        )
        .map_err(map_script_error)?;
    }
    Ok(())
}

fn validate_graph_records_component_kinds(
    bundle: &ScriptBundleDefinition,
    records: &[GraphEntityRecord],
) -> Result<(), AuthError> {
    let allowed = bundle
        .required_component_kinds
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    for record in records {
        for component in &record.components {
            if !allowed.contains(&component.component_kind) {
                return Err(AuthError::Internal(format!(
                    "bundle {} graph records contain component_kind={} not listed in required_component_kinds",
                    bundle.bundle_id, component.component_kind
                )));
            }
        }
    }
    Ok(())
}

fn inject_script_context(
    ctx: Table,
    module: &sidereal_scripting::LoadedLuaModule,
    context: Option<&ScriptContext<'_>>,
) -> Result<(), AuthError> {
    if let Some(context) = context {
        ctx.set("account_id", context.account_id.to_string())
            .map_err(|err| {
                AuthError::Internal(format!("{}: {err}", module.script_path().display()))
            })?;
        ctx.set("player_entity_id", context.player_entity_id)
            .map_err(|err| {
                AuthError::Internal(format!("{}: {err}", module.script_path().display()))
            })?;
        ctx.set("email", context.email).map_err(|err| {
            AuthError::Internal(format!("{}: {err}", module.script_path().display()))
        })?;
    }
    let new_uuid = module
        .lua()
        .create_function(|_, ()| Ok(Uuid::new_v4().to_string()))
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    ctx.set("new_uuid", new_uuid)
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    let default_corvette_outline_points = module
        .lua()
        .create_function(|lua, ()| {
            let points = default_corvette_collision_outline().points;
            let out = lua.create_table()?;
            for (idx, point) in points.iter().enumerate() {
                let pair = lua.create_table()?;
                pair.set(1, point.x)?;
                pair.set(2, point.y)?;
                out.set(idx + 1, pair)?;
            }
            Ok(out)
        })
        .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    ctx.set(
        "default_corvette_collision_outline_points",
        default_corvette_outline_points,
    )
    .map_err(|err| AuthError::Internal(format!("{}: {err}", module.script_path().display())))?;
    Ok(())
}

fn map_script_error(err: ScriptError) -> AuthError {
    AuthError::Internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ScriptContext, load_bundle_registry, load_graph_records_for_bundle,
        load_new_account_config, load_world_init_config, load_world_init_graph_records,
        scripts_root_dir,
    };
    use uuid::Uuid;

    #[test]
    fn default_world_init_script_loads() {
        let root = scripts_root_dir();
        let config = load_world_init_config(&root).expect("load world init");
        assert!(!config.space_background_shader_asset_id.is_empty());
        assert!(!config.starfield_shader_asset_id.is_empty());
    }

    #[test]
    fn default_new_account_script_loads() {
        let root = scripts_root_dir();
        let config = load_new_account_config(
            &root,
            ScriptContext {
                account_id: Uuid::new_v4(),
                player_entity_id: &Uuid::new_v4().to_string(),
                email: "pilot@example.com",
            },
        )
        .expect("load new account");
        assert_eq!(config.starter_bundle_id, "starter_corvette");
    }

    #[test]
    fn default_bundle_registry_loads_and_validates_component_kinds() {
        let root = scripts_root_dir();
        let registry = load_bundle_registry(&root).expect("load bundle registry");
        let starter_corvette = registry
            .bundles
            .get("starter_corvette")
            .expect("starter_corvette bundle");
        assert!(starter_corvette.spawn_template.is_none());
        assert_eq!(
            starter_corvette.graph_records_script.as_deref(),
            Some("bundles/starter_corvette.lua")
        );
        assert!(
            starter_corvette
                .required_component_kinds
                .contains(&"display_name".to_string())
        );
        assert!(
            starter_corvette
                .required_component_kinds
                .contains(&"scanner_range_m".to_string())
        );
        assert!(
            starter_corvette
                .required_component_kinds
                .contains(&"scanner_component".to_string())
        );
    }

    #[test]
    fn graph_records_script_bundle_payload_decodes_when_present() {
        let root = scripts_root_dir();
        let registry = load_bundle_registry(&root).expect("load bundle registry");
        let dynamic_bundle = registry
            .bundles
            .get("debug_minimal_dynamic")
            .expect("dynamic bundle");
        let records = load_graph_records_for_bundle(
            &root,
            dynamic_bundle,
            ScriptContext {
                account_id: Uuid::new_v4(),
                player_entity_id: &Uuid::new_v4().to_string(),
                email: "pilot@example.com",
            },
        )
        .expect("load graph records for dynamic bundle");
        assert!(!records.is_empty());
    }

    #[test]
    fn starter_corvette_bundle_includes_scanner_components() {
        let root = scripts_root_dir();
        let registry = load_bundle_registry(&root).expect("load bundle registry");
        let starter_bundle = registry
            .bundles
            .get("starter_corvette")
            .expect("starter bundle");
        let records = load_graph_records_for_bundle(
            &root,
            starter_bundle,
            ScriptContext {
                account_id: Uuid::new_v4(),
                player_entity_id: &Uuid::new_v4().to_string(),
                email: "pilot@example.com",
            },
        )
        .expect("load graph records for starter bundle");
        let component_kinds = records
            .iter()
            .flat_map(|record| {
                record
                    .components
                    .iter()
                    .map(|component| component.component_kind.as_str())
            })
            .collect::<std::collections::HashSet<_>>();
        assert!(component_kinds.contains("scanner_range_m"));
        assert!(component_kinds.contains("scanner_component"));
    }

    #[test]
    fn default_world_init_graph_records_script_loads() {
        let root = scripts_root_dir();
        let records = load_world_init_graph_records(&root).expect("load world init graph records");
        assert!(!records.is_empty());
        assert!(
            records
                .iter()
                .any(|record| record.entity_id == "0012ebad-0000-0000-0000-000000000001")
        );
        let component_kinds = records
            .iter()
            .flat_map(|record| {
                record
                    .components
                    .iter()
                    .map(|component| component.component_kind.as_str())
            })
            .collect::<std::collections::HashSet<_>>();
        assert!(component_kinds.contains("scanner_range_m"));
        assert!(component_kinds.contains("scanner_component"));
    }
}
