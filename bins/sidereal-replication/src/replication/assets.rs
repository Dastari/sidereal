use bevy::log::{info, warn};
use bevy::prelude::*;
use lightyear::prelude::client::Connected;
use lightyear::prelude::server::{ClientOf, LinkOf};
use lightyear::prelude::{NetworkTarget, RemoteId, Server, ServerMultiMessageSender};
use sidereal_asset_runtime::{
    build_runtime_asset_catalog, catalog_version, hot_reload_poll_interval,
};
use sidereal_net::{ManifestChannel, ServerAssetCatalogVersionMessage};
use sidereal_scripting::load_asset_registry_from_source;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::replication::auth::AuthenticatedClientBindings;
use crate::replication::scripting::{
    ScriptCatalogControlResource, ScriptCatalogEntry, ScriptCatalogResource,
    load_script_catalog_entries_from_disk, lookup_script_catalog_entry,
};

#[derive(Debug, Resource, Default)]
pub struct AssetHotReloadState {
    pub last_disk_scan_at_s: f64,
    pub last_catalog_scan_at_s: f64,
    pub current_catalog_version: Option<String>,
    pub pending_broadcast: bool,
    pub generated_at_tick: u64,
    pub last_sent_catalog_version_by_client: HashMap<Entity, String>,
}

pub fn init_resources(app: &mut App) {
    app.insert_resource(AssetHotReloadState::default());
}

fn asset_root_dir() -> PathBuf {
    PathBuf::from(std::env::var("ASSET_ROOT").unwrap_or_else(|_| "./data".to_string()))
}

fn script_entries_match_disk(
    catalog_entries: &[ScriptCatalogEntry],
    disk_entries: &[ScriptCatalogEntry],
) -> bool {
    if catalog_entries.len() != disk_entries.len() {
        return false;
    }
    catalog_entries
        .iter()
        .zip(disk_entries.iter())
        .all(|(current, disk)| {
            current.script_path == disk.script_path
                && current.source == disk.source
                && current.origin == disk.origin
        })
}

pub fn request_script_catalog_reload_on_disk_changes_system(
    time: Res<'_, Time>,
    catalog: Res<'_, ScriptCatalogResource>,
    mut control: ResMut<'_, ScriptCatalogControlResource>,
    mut state: ResMut<'_, AssetHotReloadState>,
) {
    let now_s = time.elapsed_secs_f64();
    let poll_interval_s = hot_reload_poll_interval().as_secs_f64();
    if now_s - state.last_disk_scan_at_s < poll_interval_s {
        return;
    }
    state.last_disk_scan_at_s = now_s;

    if control.reload_all_from_disk_requested {
        return;
    }

    let scripts_root = PathBuf::from(&catalog.root_dir);
    match load_script_catalog_entries_from_disk(&scripts_root) {
        Ok(disk_entries) => {
            if !script_entries_match_disk(&catalog.entries, &disk_entries) {
                control.reload_all_from_disk_requested = true;
                info!(
                    "replication asset hot reload requested script catalog reload root={} entries={}",
                    scripts_root.display(),
                    disk_entries.len()
                );
            }
        }
        Err(err) => {
            warn!(
                "replication asset hot reload script scan failed root={} err={}",
                scripts_root.display(),
                err
            );
        }
    }
}

fn load_runtime_catalog_from_script_catalog(
    catalog: &ScriptCatalogResource,
    asset_root: &Path,
) -> Result<Vec<sidereal_asset_runtime::RuntimeAssetCatalogEntry>, String> {
    let registry_entry = lookup_script_catalog_entry(catalog, "assets/registry.lua")?;
    let registry =
        load_asset_registry_from_source(&registry_entry.source, Path::new("assets/registry.lua"))
            .map_err(|err| format!("asset registry decode failed: {err}"))?;
    build_runtime_asset_catalog(asset_root, &registry.assets)
        .map_err(|err| format!("runtime asset catalog build failed: {err}"))
}

pub fn poll_runtime_asset_catalog_changes_system(
    time: Res<'_, Time>,
    catalog: Res<'_, ScriptCatalogResource>,
    mut state: ResMut<'_, AssetHotReloadState>,
) {
    let now_s = time.elapsed_secs_f64();
    let poll_interval_s = hot_reload_poll_interval().as_secs_f64();
    if now_s - state.last_catalog_scan_at_s < poll_interval_s {
        return;
    }
    state.last_catalog_scan_at_s = now_s;

    let asset_root = asset_root_dir();
    let next_catalog = match load_runtime_catalog_from_script_catalog(&catalog, &asset_root) {
        Ok(catalog) => catalog,
        Err(err) => {
            warn!(
                "replication asset hot reload catalog scan failed asset_root={} err={}",
                asset_root.display(),
                err
            );
            return;
        }
    };
    let next_version = catalog_version(&next_catalog);

    match state.current_catalog_version.clone() {
        None => {
            state.current_catalog_version = Some(next_version.clone());
            info!(
                "replication asset catalog initialized version={} entries={}",
                next_version,
                next_catalog.len()
            );
        }
        Some(current_version) if current_version == next_version => {}
        Some(current_version) => {
            state.current_catalog_version = Some(next_version.clone());
            state.pending_broadcast = true;
            state.generated_at_tick = state.generated_at_tick.saturating_add(1);
            info!(
                "replication asset catalog changed old_version={} new_version={} entries={}",
                current_version,
                next_version,
                next_catalog.len()
            );
        }
    }
}

pub fn stream_asset_catalog_version_messages(
    server_query: Query<'_, '_, &'_ Server>,
    mut sender: ServerMultiMessageSender<'_, '_, With<Connected>>,
    bindings: Res<'_, AuthenticatedClientBindings>,
    client_remotes: Query<'_, '_, (&'_ LinkOf, &'_ RemoteId), With<ClientOf>>,
    mut state: ResMut<'_, AssetHotReloadState>,
) {
    let Some(catalog_version) = state.current_catalog_version.clone() else {
        return;
    };

    let mut active_clients = HashSet::<Entity>::new();
    for client_entity in bindings.by_client_entity.keys() {
        active_clients.insert(*client_entity);
        let Ok((link_of, remote_id)) = client_remotes.get(*client_entity) else {
            continue;
        };
        let Ok(server) = server_query.get(link_of.server) else {
            continue;
        };
        let already_sent = state
            .last_sent_catalog_version_by_client
            .get(client_entity)
            .is_some_and(|last_sent| last_sent == &catalog_version);
        if already_sent && !state.pending_broadcast {
            continue;
        }

        let message = ServerAssetCatalogVersionMessage {
            catalog_version: catalog_version.clone(),
            generated_at_tick: state.generated_at_tick,
        };
        let target = NetworkTarget::Single(remote_id.0);
        let _ = sender
            .send::<ServerAssetCatalogVersionMessage, ManifestChannel>(&message, server, &target);
        state
            .last_sent_catalog_version_by_client
            .insert(*client_entity, catalog_version.clone());
    }

    state.pending_broadcast = false;
    state
        .last_sent_catalog_version_by_client
        .retain(|client_entity, _| active_clients.contains(client_entity));
}
