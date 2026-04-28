fn inject_render_authoring_api(lua: &Lua, ctx: Table) -> mlua::Result<()> {
    let render = lua.create_table()?;

    let define_layer = lua.create_function(|lua, (_render, layer): (Table, Value)| {
        let layer_json = lua_value_to_json(layer).map_err(mlua::Error::runtime)?;
        let Some(layer_object) = layer_json.as_object() else {
            return Err(mlua::Error::runtime(
                "render.define_layer expects a table payload",
            ));
        };
        let mut layer_object = layer_object.clone();
        remove_empty_array_like_field(&mut layer_object, "texture_bindings");
        let entity_id = layer_object
            .get("entity_id")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let display_name = layer_object
            .get("display_name")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .or_else(|| {
                layer_object
                    .get("layer_id")
                    .and_then(|value| value.as_str())
                    .map(|value| format!("RenderLayer:{value}"))
            })
            .unwrap_or_else(|| "RenderLayer".to_string());
        let record = serde_json::json!({
            "entity_id": entity_id,
            "labels": ["Entity", "RenderLayerDefinition"],
            "properties": {},
            "components": [
                {
                    "component_id": format!("{entity_id}:display_name"),
                    "component_kind": "display_name",
                    "properties": display_name,
                },
                {
                    "component_id": format!("{entity_id}:runtime_render_layer_definition"),
                    "component_kind": "runtime_render_layer_definition",
                    "properties": layer_object,
                }
            ]
        });
        json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
    })?;
    render.set("define_layer", define_layer)?;

    let define_rule = lua.create_function(|lua, (_render, rule): (Table, Value)| {
        let rule_json = lua_value_to_json(rule).map_err(mlua::Error::runtime)?;
        let Some(rule_object) = rule_json.as_object() else {
            return Err(mlua::Error::runtime(
                "render.define_rule expects a table payload",
            ));
        };
        let mut rule_object = rule_object.clone();
        remove_empty_array_like_field(&mut rule_object, "labels_any");
        remove_empty_array_like_field(&mut rule_object, "labels_all");
        remove_empty_array_like_field(&mut rule_object, "archetypes_any");
        remove_empty_array_like_field(&mut rule_object, "components_all");
        remove_empty_array_like_field(&mut rule_object, "components_any");
        let entity_id = rule_object
            .get("entity_id")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let display_name = rule_object
            .get("display_name")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .or_else(|| {
                rule_object
                    .get("rule_id")
                    .and_then(|value| value.as_str())
                    .map(|value| format!("RenderRule:{value}"))
            })
            .unwrap_or_else(|| "RenderRule".to_string());
        let record = serde_json::json!({
            "entity_id": entity_id,
            "labels": ["Entity", "RenderLayerRule"],
            "properties": {},
            "components": [
                {
                    "component_id": format!("{entity_id}:display_name"),
                    "component_kind": "display_name",
                    "properties": display_name,
                },
                {
                    "component_id": format!("{entity_id}:runtime_render_layer_rule"),
                    "component_kind": "runtime_render_layer_rule",
                    "properties": rule_object,
                }
            ]
        });
        json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
    })?;
    render.set("define_rule", define_rule)?;

    let define_post_process_stack =
        lua.create_function(|lua, (_render, stack): (Table, Value)| {
            let stack_json = lua_value_to_json(stack).map_err(mlua::Error::runtime)?;
            let Some(stack_object) = stack_json.as_object() else {
                return Err(mlua::Error::runtime(
                    "render.define_post_process_stack expects a table payload",
                ));
            };
            let mut stack_object = stack_object.clone();
            if let Some(serde_json::Value::Array(passes)) = stack_object.get_mut("passes") {
                for pass in passes {
                    if let Some(pass_object) = pass.as_object_mut() {
                        remove_empty_array_like_field(pass_object, "texture_bindings");
                    }
                }
            }
            remove_empty_array_like_field(&mut stack_object, "passes");
            let entity_id = stack_object
                .get("entity_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let display_name = stack_object
                .get("display_name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "PostProcessStack".to_string());
            let record = serde_json::json!({
                "entity_id": entity_id,
                "labels": ["Entity", "RuntimePostProcessStack"],
                "properties": {},
                "components": [
                    {
                        "component_id": format!("{entity_id}:display_name"),
                        "component_kind": "display_name",
                        "properties": display_name,
                    },
                    {
                        "component_id": format!("{entity_id}:runtime_post_process_stack"),
                        "component_kind": "runtime_post_process_stack",
                        "properties": stack_object,
                    }
                ]
            });
            json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
        })?;
    render.set("define_post_process_stack", define_post_process_stack)?;

    let define_world_visual_stack =
        lua.create_function(|lua, (_render, stack): (Table, Value)| {
            let stack_json = lua_value_to_json(stack).map_err(mlua::Error::runtime)?;
            let Some(stack_object) = stack_json.as_object() else {
                return Err(mlua::Error::runtime(
                    "render.define_world_visual_stack expects a table payload",
                ));
            };
            let mut stack_object = stack_object.clone();
            if let Some(serde_json::Value::Array(passes)) = stack_object.get_mut("passes") {
                for pass in passes {
                    if let Some(pass_object) = pass.as_object_mut() {
                        remove_empty_array_like_field(pass_object, "texture_bindings");
                    }
                }
            }
            remove_empty_array_like_field(&mut stack_object, "passes");
            let entity_id = stack_object
                .get("entity_id")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let display_name = stack_object
                .get("display_name")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| "WorldVisualStack".to_string());
            let record = serde_json::json!({
                "entity_id": entity_id,
                "labels": ["Entity", "RuntimeWorldVisualStack"],
                "properties": {},
                "components": [
                    {
                        "component_id": format!("{entity_id}:display_name"),
                        "component_kind": "display_name",
                        "properties": display_name,
                    },
                    {
                        "component_id": format!("{entity_id}:runtime_world_visual_stack"),
                        "component_kind": "runtime_world_visual_stack",
                        "properties": stack_object,
                    }
                ]
            });
            json_value_to_lua(lua, &record).map_err(mlua::Error::runtime)
        })?;
    render.set("define_world_visual_stack", define_world_visual_stack)?;

    ctx.set("render", render)?;
    Ok(())
}

#[allow(dead_code)]
fn inject_generate_collision_outline_fn(
    ctx: Table,
    lua: &Lua,
    scripts_root: &Path,
) -> mlua::Result<()> {
    let asset_entries =
        Arc::new(load_asset_registry_entries(scripts_root).map_err(mlua::Error::runtime)?);
    inject_generate_collision_outline_fn_cached(ctx, lua, scripts_root, asset_entries)
}

fn inject_generate_collision_outline_fn_cached(
    ctx: Table,
    lua: &Lua,
    scripts_root: &Path,
    asset_entries: Arc<Vec<AssetRegistryEntry>>,
) -> mlua::Result<()> {
    let scripts_root = scripts_root.to_path_buf();
    let scripts_root_for_half_extents = scripts_root.clone();
    let asset_entries_for_half_extents = asset_entries.clone();
    let compute_collision_half_extents_from_length =
        lua.create_function(move |lua, (visual_asset_id, length_m): (String, f32)| {
            let Some(asset) = asset_entries_for_half_extents
                .iter()
                .find(|entry| entry.asset_id == visual_asset_id)
            else {
                return Err(mlua::Error::runtime(format!(
                    "unknown visual asset id for collision half extents: {}",
                    visual_asset_id
                )));
            };
            let asset_root = std::env::var("ASSET_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    scripts_root_for_half_extents
                        .parent()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| scripts_root_for_half_extents.clone())
                });
            let sprite_path = asset_root.join(&asset.source_path);
            let sprite_png =
                std::fs::read(&sprite_path).map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let (half_x, half_y) =
                compute_collision_half_extents_from_sprite_length(&sprite_png, length_m)
                    .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            out.set(1, half_x)?;
            out.set(2, half_y)?;
            Ok(out)
        })?;
    ctx.set(
        "compute_collision_half_extents_from_length",
        compute_collision_half_extents_from_length,
    )?;
    let compute_collision_half_extents_from_procedural = lua.create_function(
        move |lua, (entity_id, procedural_sprite, length_m): (String, Value, f32)| {
            let procedural_sprite_json =
                lua_value_to_json(procedural_sprite).map_err(mlua::Error::runtime)?;
            let procedural_sprite =
                serde_json::from_value::<ProceduralSprite>(procedural_sprite_json)
                    .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let (half_x, half_y) = compute_collision_half_extents_from_procedural_sprite(
                &entity_id,
                &procedural_sprite,
                length_m,
            )
            .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            out.set(1, half_x)?;
            out.set(2, half_y)?;
            Ok(out)
        },
    )?;
    ctx.set(
        "compute_collision_half_extents_from_procedural",
        compute_collision_half_extents_from_procedural,
    )?;
    let asset_entries_for_outline = asset_entries.clone();
    let generate_collision_outline_rdp =
        lua.create_function(move |lua, (visual_asset_id, half_extents): (String, Value)| {
            let (half_x, half_y) = match half_extents {
                Value::Table(table) => {
                    let half_x = table.get::<f32>(1)?;
                    let half_y = table.get::<f32>(2)?;
                    (half_x, half_y)
                }
                _ => {
                    return Err(mlua::Error::runtime(
                        "generate_collision_outline_rdp expects half_extents table {half_x, half_y}",
                    ));
                }
            };
            let Some(asset) = asset_entries_for_outline
                .iter()
                .find(|entry| entry.asset_id == visual_asset_id)
            else {
                return Err(mlua::Error::runtime(format!(
                    "unknown visual asset id for collision outline: {}",
                    visual_asset_id
                )));
            };
            let asset_root = std::env::var("ASSET_ROOT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    scripts_root
                        .parent()
                        .map(PathBuf::from)
                        .unwrap_or_else(|| scripts_root.clone())
                });
            let sprite_path = asset_root.join(&asset.source_path);
            let sprite_png =
                std::fs::read(&sprite_path).map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let outline =
                generate_rdp_collision_outline_from_sprite_png(&sprite_png, half_x, half_y)
                    .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            for (idx, point) in outline.points.iter().enumerate() {
                let point_table = lua.create_table()?;
                point_table.set(1, point.x)?;
                point_table.set(2, point.y)?;
                out.set(idx + 1, point_table)?;
            }
            Ok(out)
        })?;
    ctx.set(
        "generate_collision_outline_rdp",
        generate_collision_outline_rdp,
    )?;
    let generate_collision_outline_rdp_from_procedural = lua.create_function(
        move |lua, (entity_id, procedural_sprite, half_extents): (String, Value, Value)| {
            let (half_x, half_y) = match half_extents {
                Value::Table(table) => {
                    let half_x = table.get::<f32>(1)?;
                    let half_y = table.get::<f32>(2)?;
                    (half_x, half_y)
                }
                _ => {
                    return Err(mlua::Error::runtime(
                        "generate_collision_outline_rdp_from_procedural expects half_extents table {half_x, half_y}",
                    ));
                }
            };
            let procedural_sprite_json =
                lua_value_to_json(procedural_sprite).map_err(mlua::Error::runtime)?;
            let procedural_sprite = serde_json::from_value::<ProceduralSprite>(procedural_sprite_json)
                .map_err(|err| mlua::Error::runtime(err.to_string()))?;
            let outline = generate_rdp_collision_outline_from_procedural_sprite(
                &entity_id,
                &procedural_sprite,
                half_x,
                half_y,
            )
            .map_err(mlua::Error::runtime)?;
            let out = lua.create_table()?;
            for (idx, point) in outline.points.iter().enumerate() {
                let point_table = lua.create_table()?;
                point_table.set(1, point.x)?;
                point_table.set(2, point.y)?;
                out.set(idx + 1, point_table)?;
            }
            Ok(out)
        },
    )?;
    ctx.set(
        "generate_collision_outline_rdp_from_procedural",
        generate_collision_outline_rdp_from_procedural,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        init_resources, load_script_catalog_from_database_or_disk_with_url, scripts_root_dir,
        spawn_bundle_graph_records,
    };
    use bevy::prelude::{App, MinimalPlugins};
    use sidereal_game::{GeneratedComponentRegistry, SiderealGameCorePlugin};

    #[test]
    fn script_catalog_falls_back_to_disk_when_database_is_unreachable() {
        let root = scripts_root_dir();
        let outcome = load_script_catalog_from_database_or_disk_with_url(
            &root,
            "postgres://sidereal:sidereal@127.0.0.1:1/sidereal",
        )
        .expect("disk fallback should succeed");
        assert!(outcome.startup_loaded_from_disk_fallback);
        assert_eq!(outcome.persisted_catalog_revision, 0);
        assert!(!outcome.catalog.entries.is_empty());
        assert!(
            outcome
                .catalog
                .entries
                .iter()
                .any(|entry| entry.script_path == "bundles/bundle_registry.lua")
        );
    }

    #[test]
    fn bundle_spawn_uses_host_provided_entity_id() {
        let root = scripts_root_dir();
        let entity_id = uuid::Uuid::new_v4().to_string();
        let owner_id = uuid::Uuid::new_v4().to_string();
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "entity_id".to_string(),
            serde_json::Value::String(entity_id.clone()),
        );
        overrides.insert(
            "owner_id".to_string(),
            serde_json::Value::String(owner_id.clone()),
        );
        let records =
            spawn_bundle_graph_records(&root, "ship.corvette", &overrides).expect("spawn");
        assert!(!records.is_empty());
        assert_eq!(records[0].entity_id, entity_id);
    }

    #[test]
    fn bundle_spawn_rejects_unknown_bundle_id() {
        let root = scripts_root_dir();
        let err = spawn_bundle_graph_records(&root, "unknown_bundle", &serde_json::Map::new())
            .expect_err("unknown bundle should fail");
        assert!(err.contains("unknown bundle_id"));
    }

    #[test]
    fn bundle_spawn_generates_nondeterministic_uuid_when_not_overridden() {
        let root = scripts_root_dir();
        let owner_id = uuid::Uuid::new_v4().to_string();
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "owner_id".to_string(),
            serde_json::Value::String(owner_id.clone()),
        );
        let first =
            spawn_bundle_graph_records(&root, "ship.corvette", &overrides).expect("spawn first");
        let second =
            spawn_bundle_graph_records(&root, "ship.corvette", &overrides).expect("spawn second");
        assert!(!first.is_empty());
        assert!(!second.is_empty());
        assert_ne!(
            first[0].entity_id, second[0].entity_id,
            "root entity IDs should be random UUIDs when no entity_id override is provided"
        );
    }

    #[test]
    fn asteroid_field_v2_bundle_spawns_root_and_linked_members() {
        let root = scripts_root_dir();
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "field_entity_id".to_string(),
            serde_json::Value::String("0012ebad-0000-0000-0000-000000000020".to_string()),
        );
        overrides.insert("field_count".to_string(), serde_json::json!(3));
        let records =
            spawn_bundle_graph_records(&root, "asteroid.field", &overrides).expect("spawn field");

        assert_eq!(records.len(), 4);
        assert!(
            records[0]
                .components
                .iter()
                .any(|component| component.component_kind == "asteroid_field")
        );
        assert!(records.iter().skip(1).all(|record| {
            record
                .components
                .iter()
                .any(|component| component.component_kind == "asteroid_field_member")
        }));
    }

    #[test]
    fn init_resources_preserves_inferred_component_editor_schema_entries() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, SiderealGameCorePlugin));

        init_resources(&mut app);

        let registry = app.world().resource::<GeneratedComponentRegistry>();
        let max_velocity = registry
            .entries
            .iter()
            .find(|entry| entry.component_kind == "max_velocity_mps")
            .expect("max_velocity_mps mapping should exist");
        assert!(
            !max_velocity.editor_schema.fields.is_empty(),
            "replication scripting init should preserve inferred editor schema fields"
        );
        assert!(
            !registry.shader_entries.is_empty(),
            "replication scripting init should still populate shader entries"
        );
    }
}
