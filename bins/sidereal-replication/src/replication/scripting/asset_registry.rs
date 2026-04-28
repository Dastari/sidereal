pub fn load_asset_registry_entries(root: &Path) -> Result<Vec<AssetRegistryEntry>, String> {
    let catalog = script_catalog_from_disk(root)?;
    load_asset_registry_entries_from_catalog(&catalog)
}

pub fn load_asset_registry_entries_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<Vec<AssetRegistryEntry>, String> {
    load_asset_registry_data_from_catalog(catalog).map(|(entries, _)| entries)
}

pub fn load_planet_registry_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<sidereal_game::PlanetRegistry, String> {
    let registry_entry = lookup_script_catalog_entry(catalog, PLANET_REGISTRY_SCRIPT_REL_PATH)?;
    let sources_by_script_path = catalog
        .entries
        .iter()
        .map(|entry| (entry.script_path.clone(), entry.source.clone()))
        .collect::<HashMap<_, _>>();
    load_planet_registry_from_sources(
        &registry_entry.source,
        Path::new(PLANET_REGISTRY_SCRIPT_REL_PATH),
        &sources_by_script_path,
    )
    .map_err(map_script_err)
}

pub fn load_ship_module_registry_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<sidereal_game::ShipModuleRegistry, String> {
    let registry_entry =
        lookup_script_catalog_entry(catalog, SHIP_MODULE_REGISTRY_SCRIPT_REL_PATH)?;
    let sources_by_script_path = catalog
        .entries
        .iter()
        .map(|entry| (entry.script_path.clone(), entry.source.clone()))
        .collect::<HashMap<_, _>>();
    load_ship_module_registry_from_sources(
        &registry_entry.source,
        Path::new(SHIP_MODULE_REGISTRY_SCRIPT_REL_PATH),
        &sources_by_script_path,
    )
    .map_err(map_script_err)
}

pub fn load_ship_registry_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<sidereal_game::ShipRegistry, String> {
    let registry_entry = lookup_script_catalog_entry(catalog, SHIP_REGISTRY_SCRIPT_REL_PATH)?;
    let asset_registry_entry =
        lookup_script_catalog_entry(catalog, ASSET_REGISTRY_SCRIPT_REL_PATH)?;
    let asset_registry = load_asset_registry_from_source(
        &asset_registry_entry.source,
        Path::new(ASSET_REGISTRY_SCRIPT_REL_PATH),
    )
    .map_err(map_script_err)?;
    let module_registry = load_ship_module_registry_from_catalog(catalog)?;
    let sources_by_script_path = catalog
        .entries
        .iter()
        .map(|entry| (entry.script_path.clone(), entry.source.clone()))
        .collect::<HashMap<_, _>>();
    load_ship_registry_from_sources(
        &registry_entry.source,
        Path::new(SHIP_REGISTRY_SCRIPT_REL_PATH),
        &sources_by_script_path,
        &module_registry,
        &asset_registry,
    )
    .map_err(map_script_err)
}

fn load_asset_registry_data_from_catalog(
    catalog: &ScriptCatalogResource,
) -> Result<
    (
        Vec<AssetRegistryEntry>,
        Vec<sidereal_game::ShaderEditorRegistryEntry>,
    ),
    String,
> {
    let entry = lookup_script_catalog_entry(catalog, ASSET_REGISTRY_SCRIPT_REL_PATH)?;
    let registry =
        load_asset_registry_from_source(&entry.source, Path::new(ASSET_REGISTRY_SCRIPT_REL_PATH))
            .map_err(map_script_err)?;
    let mut assets = registry
        .assets
        .iter()
        .map(map_script_asset_registry_entry)
        .collect::<Vec<_>>();
    assets.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    let shader_entries = shader_registry_entries_from_script_assets(&registry.assets)?;
    Ok((assets, shader_entries))
}

fn map_script_asset_registry_entry(entry: &ScriptAssetRegistryEntry) -> AssetRegistryEntry {
    AssetRegistryEntry {
        asset_id: entry.asset_id.clone(),
        shader_family: entry.shader_family.clone(),
        source_path: entry.source_path.clone(),
        content_type: entry.content_type.clone(),
        dependencies: entry.dependencies.clone(),
        bootstrap_required: entry.bootstrap_required,
    }
}

fn shader_registry_entries_from_script_assets(
    asset_entries: &[ScriptAssetRegistryEntry],
) -> Result<Vec<sidereal_game::ShaderEditorRegistryEntry>, String> {
    let mut entries = asset_entries
        .iter()
        .filter(|entry| entry.source_path.ends_with(".wgsl"))
        .map(map_script_shader_registry_entry)
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    Ok(entries)
}

fn map_script_shader_registry_entry(
    entry: &ScriptAssetRegistryEntry,
) -> Result<sidereal_game::ShaderEditorRegistryEntry, String> {
    let uniform_schema = entry
        .editor_schema
        .as_ref()
        .map(|schema| {
            schema
                .uniforms
                .iter()
                .map(|field| {
                    Ok(sidereal_game::ShaderEditorFieldSchema {
                        field_path: field.field_path.clone(),
                        display_name: field
                            .label
                            .clone()
                            .unwrap_or_else(|| field.field_path.clone()),
                        description: field.description.clone(),
                        value_kind: shader_editor_value_kind(&field.kind)?,
                        min: field.min,
                        max: field.max,
                        step: field.step,
                        options: field
                            .options
                            .iter()
                            .map(|option| sidereal_game::ShaderEditorOption {
                                value: option.value.clone(),
                                label: option.label.clone(),
                            })
                            .collect(),
                        default_value_json: field
                            .default_value
                            .as_ref()
                            .map(serde_json::to_string)
                            .transpose()
                            .map_err(|err| {
                                format!(
                                    "asset {} uniform {} default json serialize failed: {}",
                                    entry.asset_id, field.field_path, err
                                )
                            })?,
                        group: field.group.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    let presets = entry
        .editor_schema
        .as_ref()
        .map(|schema| {
            schema
                .presets
                .iter()
                .map(|preset| {
                    Ok(sidereal_game::ShaderEditorPreset {
                        preset_id: preset.preset_id.clone(),
                        display_name: preset.label.clone(),
                        description: preset.description.clone(),
                        values_json: serde_json::to_string(&preset.values).map_err(|err| {
                            format!(
                                "asset {} preset {} json serialize failed: {}",
                                entry.asset_id, preset.preset_id, err
                            )
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        })
        .transpose()?
        .unwrap_or_default();
    Ok(sidereal_game::ShaderEditorRegistryEntry {
        asset_id: entry.asset_id.clone(),
        source_path: entry.source_path.clone(),
        shader_family: entry.shader_family.clone(),
        dependencies: entry.dependencies.clone(),
        bootstrap_required: entry.bootstrap_required,
        uniform_schema,
        presets,
    })
}

fn shader_editor_value_kind(kind: &str) -> Result<sidereal_game::ComponentEditorValueKind, String> {
    match kind {
        "Bool" => Ok(sidereal_game::ComponentEditorValueKind::Bool),
        "SignedInteger" => Ok(sidereal_game::ComponentEditorValueKind::SignedInteger),
        "UnsignedInteger" => Ok(sidereal_game::ComponentEditorValueKind::UnsignedInteger),
        "Float" => Ok(sidereal_game::ComponentEditorValueKind::Float),
        "String" => Ok(sidereal_game::ComponentEditorValueKind::String),
        "Vec2" => Ok(sidereal_game::ComponentEditorValueKind::Vec2),
        "Vec3" => Ok(sidereal_game::ComponentEditorValueKind::Vec3),
        "Vec4" => Ok(sidereal_game::ComponentEditorValueKind::Vec4),
        "ColorRgb" => Ok(sidereal_game::ComponentEditorValueKind::ColorRgb),
        "ColorRgba" => Ok(sidereal_game::ComponentEditorValueKind::ColorRgba),
        "Enum" => Ok(sidereal_game::ComponentEditorValueKind::Enum),
        "Sequence" => Ok(sidereal_game::ComponentEditorValueKind::Sequence),
        "Struct" => Ok(sidereal_game::ComponentEditorValueKind::Struct),
        "Tuple" => Ok(sidereal_game::ComponentEditorValueKind::Tuple),
        "Unknown" => Ok(sidereal_game::ComponentEditorValueKind::Unknown),
        other => Err(format!("unsupported shader editor value kind={other}")),
    }
}
