fn reload_all_scripts_from_disk_system(
    time: Res<'_, Time>,
    mut control: ResMut<'_, ScriptCatalogControlResource>,
    mut catalog: ResMut<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, ScriptCatalogSyncState>,
) {
    if !control.reload_all_from_disk_requested {
        return;
    }
    let root = PathBuf::from(&catalog.root_dir);
    match load_script_catalog_entries_from_disk(&root) {
        Ok(entries) => {
            catalog.entries = entries;
            catalog.revision = catalog.revision.saturating_add(1);
            sync_state.fingerprints_by_path.clear();
            sync_state.next_revision = sync_state
                .next_revision
                .max(catalog.revision.saturating_add(1));
            control.reload_all_from_disk_requested = false;
            control.last_reload_succeeded = true;
            control.last_reload_message =
                format!("reloaded {} scripts from disk", catalog.entries.len());
            control.last_reload_at_s = time.elapsed_secs_f64();
            bevy::log::info!(
                "replication script catalog reloaded from disk root={} count={} revision={}",
                catalog.root_dir,
                catalog.entries.len(),
                catalog.revision
            );
        }
        Err(err) => {
            control.reload_all_from_disk_requested = false;
            control.last_reload_succeeded = false;
            control.last_reload_message = err.clone();
            control.last_reload_at_s = time.elapsed_secs_f64();
            bevy::log::warn!(
                "replication script catalog reload from disk failed: {}",
                err
            );
        }
    }
}

fn normalize_script_catalog_resource_system(
    mut catalog: ResMut<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, ScriptCatalogSyncState>,
) {
    let mut changed = false;
    catalog
        .entries
        .sort_by(|a, b| a.script_path.cmp(&b.script_path));
    let mut next_fingerprints = HashMap::new();
    for entry in &mut catalog.entries {
        let fingerprint = catalog_entry_fingerprint(entry);
        next_fingerprints.insert(entry.script_path.clone(), fingerprint);
        let known = sync_state
            .fingerprints_by_path
            .get(&entry.script_path)
            .copied();
        if known != Some(fingerprint) || entry.revision == 0 {
            entry.revision = sync_state.next_revision;
            sync_state.next_revision = sync_state.next_revision.saturating_add(1);
            changed = true;
        }
    }
    if next_fingerprints.len() != sync_state.fingerprints_by_path.len() {
        changed = true;
    }
    if changed {
        catalog.revision = sync_state.next_revision;
        sync_state.next_revision = sync_state.next_revision.saturating_add(1);
        sync_state.fingerprints_by_path = next_fingerprints;
    }
}

fn persist_script_catalog_resource_system(
    time: Res<'_, Time>,
    catalog: Res<'_, ScriptCatalogResource>,
    mut control: ResMut<'_, ScriptCatalogControlResource>,
    mut persistence_state: ResMut<'_, ScriptCatalogPersistenceState>,
) {
    if persistence_state.last_persisted_catalog_revision == catalog.revision {
        return;
    }
    let database_url = replication_database_url();
    let mut client = match postgres::Client::connect(&database_url, postgres::NoTls) {
        Ok(client) => client,
        Err(err) => {
            control.last_persist_succeeded = false;
            control.last_persist_message = format!("postgres connect failed: {err}");
            control.last_persist_at_s = time.elapsed_secs_f64();
            bevy::log::warn!(
                "replication script catalog persist connect failed revision={}: {}",
                catalog.revision,
                err
            );
            return;
        }
    };
    let records = catalog
        .entries
        .iter()
        .map(persisted_record_from_entry)
        .collect::<Vec<_>>();
    match replace_active_script_catalog(&mut client, &records) {
        Ok(()) => {
            persistence_state.last_persisted_catalog_revision = catalog.revision;
            control.last_persist_succeeded = true;
            control.last_persist_message = format!(
                "persisted {} scripts to database at catalog_revision={}",
                catalog.entries.len(),
                catalog.revision
            );
            control.last_persist_at_s = time.elapsed_secs_f64();
            bevy::log::info!(
                "replication persisted script catalog to database count={} catalog_revision={}",
                catalog.entries.len(),
                catalog.revision
            );
        }
        Err(err) => {
            control.last_persist_succeeded = false;
            control.last_persist_message = err.to_string();
            control.last_persist_at_s = time.elapsed_secs_f64();
            bevy::log::warn!(
                "replication script catalog persist failed revision={}: {}",
                catalog.revision,
                err
            );
        }
    }
}

fn sync_entity_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut last_catalog_revision: Local<'_, u64>,
    mut registry: ResMut<'_, EntityRegistryResource>,
) {
    if *last_catalog_revision == catalog.revision {
        return;
    }
    *last_catalog_revision = catalog.revision;
    match load_entity_registry_entries_from_catalog(&catalog) {
        Ok(entries) => {
            registry.entries = entries;
            registry.revision = registry.revision.saturating_add(1);
            bevy::log::info!(
                "replication entity registry reloaded from script catalog script={} catalog_revision={} revision={} entries={}",
                ENTITY_REGISTRY_SCRIPT_REL_PATH,
                catalog.revision,
                registry.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication entity registry reload failed script={}: {}",
                ENTITY_REGISTRY_SCRIPT_REL_PATH,
                err
            );
        }
    }
}

fn sync_asset_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, AssetRegistrySyncState>,
    mut registry: ResMut<'_, AssetRegistryResource>,
    generated_registry: Option<ResMut<'_, sidereal_game::GeneratedComponentRegistry>>,
) {
    if sync_state.last_catalog_revision == catalog.revision {
        return;
    }
    sync_state.last_catalog_revision = catalog.revision;
    match load_asset_registry_data_from_catalog(&catalog) {
        Ok((entries, shader_entries)) => {
            registry.entries = entries;
            registry.revision = registry.revision.saturating_add(1);
            if let Some(mut generated_registry) = generated_registry {
                generated_registry.shader_entries = shader_entries;
            }
            bevy::log::info!(
                "replication asset registry reloaded from script catalog script={} catalog_revision={} revision={} entries={}",
                sync_state.registry_script_path.display(),
                catalog.revision,
                registry.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication asset registry reload failed script={}: {}",
                sync_state.registry_script_path.display(),
                err
            );
        }
    }
}

fn sync_planet_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, PlanetRegistrySyncState>,
    mut registry: ResMut<'_, sidereal_game::PlanetRegistry>,
) {
    if sync_state.last_catalog_revision == catalog.revision {
        return;
    }
    sync_state.last_catalog_revision = catalog.revision;
    match load_planet_registry_from_catalog(&catalog) {
        Ok(next_registry) => {
            *registry = next_registry;
            bevy::log::info!(
                "replication planet registry reloaded from script catalog script={} catalog_revision={} entries={}",
                sync_state.registry_script_path.display(),
                catalog.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication planet registry reload failed script={}: {}",
                sync_state.registry_script_path.display(),
                err
            );
        }
    }
}

fn sync_ship_module_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, ShipModuleRegistrySyncState>,
    mut registry: ResMut<'_, sidereal_game::ShipModuleRegistry>,
) {
    if sync_state.last_catalog_revision == catalog.revision {
        return;
    }
    sync_state.last_catalog_revision = catalog.revision;
    match load_ship_module_registry_from_catalog(&catalog) {
        Ok(next_registry) => {
            *registry = next_registry;
            bevy::log::info!(
                "replication ship module registry reloaded from script catalog script={} catalog_revision={} entries={}",
                sync_state.registry_script_path.display(),
                catalog.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication ship module registry reload failed script={}: {}",
                sync_state.registry_script_path.display(),
                err
            );
        }
    }
}

fn sync_ship_registry_resource_system(
    catalog: Res<'_, ScriptCatalogResource>,
    mut sync_state: ResMut<'_, ShipRegistrySyncState>,
    mut registry: ResMut<'_, sidereal_game::ShipRegistry>,
) {
    if sync_state.last_catalog_revision == catalog.revision {
        return;
    }
    sync_state.last_catalog_revision = catalog.revision;
    match load_ship_registry_from_catalog(&catalog) {
        Ok(next_registry) => {
            *registry = next_registry;
            bevy::log::info!(
                "replication ship registry reloaded from script catalog script={} catalog_revision={} entries={}",
                sync_state.registry_script_path.display(),
                catalog.revision,
                registry.entries.len()
            );
        }
        Err(err) => {
            bevy::log::warn!(
                "replication ship registry reload failed script={}: {}",
                sync_state.registry_script_path.display(),
                err
            );
        }
    }
}
