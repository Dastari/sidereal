use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[cfg(feature = "scripting_catalog")]
use sidereal_scripting::ScriptAssetRegistryEntry;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AssetCatalogEntry {
    pub asset_id: String,
    pub relative_cache_path: String,
    pub content_type: String,
    pub byte_len: u64,
    pub chunk_count: u32,
    pub asset_version: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAssetCatalogEntry {
    pub asset_id: String,
    pub asset_guid: String,
    pub shader_family: Option<String>,
    pub dependencies: Vec<String>,
    pub relative_cache_path: String,
    pub source_path: String,
    pub content_type: String,
    pub byte_len: u64,
    pub sha256_hex: String,
    pub bootstrap_required: bool,
    pub startup_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializedRuntimeAsset {
    pub full_path: PathBuf,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetCacheIndexRecord {
    pub asset_version: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AssetCacheIndex {
    pub by_asset_id: HashMap<String, AssetCacheIndexRecord>,
}

pub fn expand_required_assets(
    required_asset_ids: &std::collections::HashSet<String>,
    dependencies_by_asset_id: &HashMap<String, Vec<String>>,
) -> std::collections::HashSet<String> {
    let mut expanded = required_asset_ids.clone();
    for asset_id in required_asset_ids {
        if let Some(dependencies) = dependencies_by_asset_id.get(asset_id) {
            for dependency in dependencies {
                expanded.insert(dependency.clone());
            }
        }
    }
    expanded
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    format!("{:x}", digest)
}

pub fn asset_version_from_sha256_hex(sha256: &str) -> u64 {
    let prefix = sha256.as_bytes().get(0..16).unwrap_or_default();
    let as_str = std::str::from_utf8(prefix).unwrap_or("0");
    u64::from_str_radix(as_str, 16).unwrap_or(0)
}

pub fn generated_asset_guid(asset_id: &str, sha256_hex: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"sidereal-asset-guid-v1:");
    digest.update(asset_id.as_bytes());
    digest.update(b":");
    digest.update(sha256_hex.as_bytes());
    let hex = format!("{:x}", digest.finalize());
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

pub fn generated_relative_cache_path(
    asset_guid: &str,
    source_path: &str,
    content_type: &str,
) -> String {
    let extension = Path::new(source_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .filter(|ext| !ext.is_empty())
        .or_else(|| content_type_extension(content_type).map(str::to_string))
        .unwrap_or_else(|| "bin".to_string());
    let directory = cache_directory_for_content_type(content_type, extension.as_str());
    format!("{directory}/{asset_guid}.{extension}")
}

#[cfg(feature = "scripting_catalog")]
pub fn build_runtime_asset_catalog(
    asset_root: &Path,
    assets: &[ScriptAssetRegistryEntry],
) -> io::Result<Vec<RuntimeAssetCatalogEntry>> {
    let mut out = Vec::with_capacity(assets.len());
    for asset in assets {
        let full_path = asset_root.join(&asset.source_path);
        let bytes = std::fs::read(&full_path)?;
        let sha256_hex = sha256_hex(&bytes);
        let asset_guid = generated_asset_guid(&asset.asset_id, &sha256_hex);
        out.push(RuntimeAssetCatalogEntry {
            asset_id: asset.asset_id.clone(),
            asset_guid: asset_guid.clone(),
            shader_family: asset.shader_family.clone(),
            dependencies: asset.dependencies.clone(),
            relative_cache_path: generated_relative_cache_path(
                &asset_guid,
                asset.source_path.as_str(),
                asset.content_type.as_str(),
            ),
            source_path: asset.source_path.clone(),
            content_type: asset.content_type.clone(),
            byte_len: bytes.len() as u64,
            sha256_hex,
            bootstrap_required: asset.bootstrap_required,
            startup_required: asset.startup_required,
        });
    }
    Ok(out)
}

pub fn catalog_version(entries: &[RuntimeAssetCatalogEntry]) -> String {
    let mut records = entries
        .iter()
        .map(|entry| {
            let mut dependencies = entry.dependencies.clone();
            dependencies.sort();
            (
                entry.asset_id.as_str(),
                entry.asset_guid.as_str(),
                entry.shader_family.as_deref().unwrap_or(""),
                entry.sha256_hex.as_str(),
                entry.bootstrap_required,
                entry.startup_required,
                dependencies,
            )
        })
        .collect::<Vec<_>>();
    records.sort_by(|a, b| a.0.cmp(b.0));

    let mut digest = Sha256::new();
    digest.update(b"sidereal-catalog-v1");
    for (
        asset_id,
        asset_guid,
        shader_family,
        sha256_hex,
        bootstrap_required,
        startup_required,
        dependencies,
    ) in records
    {
        digest.update(b"\nasset:");
        digest.update(asset_id.as_bytes());
        digest.update(b":");
        digest.update(asset_guid.as_bytes());
        digest.update(b":");
        digest.update(shader_family.as_bytes());
        digest.update(b":");
        digest.update(sha256_hex.as_bytes());
        digest.update(b":");
        digest.update(if bootstrap_required { b"1" } else { b"0" });
        digest.update(b":");
        digest.update(if startup_required { b"1" } else { b"0" });
        for dependency in dependencies {
            digest.update(b":dep:");
            digest.update(dependency.as_bytes());
        }
    }
    format!("{:x}", digest.finalize())
}

pub fn published_runtime_asset_path(
    asset_root: &Path,
    entry: &RuntimeAssetCatalogEntry,
) -> PathBuf {
    asset_root
        .join("published_assets")
        .join(&entry.relative_cache_path)
}

pub fn source_runtime_asset_path(asset_root: &Path, entry: &RuntimeAssetCatalogEntry) -> PathBuf {
    asset_root.join(&entry.source_path)
}

pub fn materialize_runtime_asset(
    asset_root: &Path,
    entry: &RuntimeAssetCatalogEntry,
) -> io::Result<MaterializedRuntimeAsset> {
    let source_path = source_runtime_asset_path(asset_root, entry);
    let published_path = published_runtime_asset_path(asset_root, entry);
    let source_bytes = std::fs::read(&source_path)?;
    let source_sha256 = sha256_hex(&source_bytes);
    if source_sha256 != entry.sha256_hex {
        return Err(io::Error::other(format!(
            "asset checksum mismatch for {} from {}",
            entry.asset_id,
            source_path.display()
        )));
    }
    let published_is_current = std::fs::read(&published_path)
        .ok()
        .map(|bytes| sha256_hex(&bytes) == entry.sha256_hex)
        .unwrap_or(false);
    if !published_is_current {
        if let Some(parent) = published_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&published_path, &source_bytes)?;
    }
    Ok(MaterializedRuntimeAsset {
        full_path: published_path,
        content_type: entry.content_type.clone(),
    })
}

fn content_type_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/png" => Some("png"),
        "image/svg+xml" => Some("svg"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "text/wgsl" | "text/plain+wgsl" | "application/wgsl" => Some("wgsl"),
        "audio/ogg" => Some("ogg"),
        "audio/wav" => Some("wav"),
        _ => None,
    }
}

fn cache_directory_for_content_type(content_type: &str, extension: &str) -> &'static str {
    if extension == "wgsl" || content_type.contains("wgsl") {
        "shaders"
    } else if extension == "svg" || content_type == "image/svg+xml" {
        "icons"
    } else if content_type.starts_with("image/") {
        "textures"
    } else if content_type.starts_with("audio/") {
        "audio"
    } else {
        "data"
    }
}

pub fn cache_index_path(asset_root: &str) -> PathBuf {
    PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join("index.json")
}

pub fn hot_reload_poll_interval() -> Duration {
    let seconds = std::env::var("SIDEREAL_ASSET_HOT_RELOAD_INTERVAL_S")
        .ok()
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(1.0, 30.0))
        .unwrap_or(5.0);
    Duration::from_secs_f64(seconds)
}

pub fn load_cache_index(path: &Path) -> io::Result<AssetCacheIndex> {
    let bytes = std::fs::read(path)?;
    let index = serde_json::from_slice::<AssetCacheIndex>(&bytes).map_err(io::Error::other)?;
    Ok(index)
}

pub fn save_cache_index(path: &Path, index: &AssetCacheIndex) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(index).map_err(io::Error::other)?;
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generated_relative_cache_path_does_not_leak_source_layout() {
        let path = generated_relative_cache_path(
            "12345678-1234-1234-1234-123456789abc",
            "sprites/ships/corvette.png",
            "image/png",
        );
        assert_eq!(path, "textures/12345678-1234-1234-1234-123456789abc.png");
    }

    #[test]
    fn generated_relative_cache_path_routes_shaders_to_shader_cache() {
        let path = generated_relative_cache_path(
            "12345678-1234-1234-1234-123456789abc",
            "shaders/starfield.wgsl",
            "text/wgsl",
        );
        assert_eq!(path, "shaders/12345678-1234-1234-1234-123456789abc.wgsl");
    }

    #[test]
    fn materialize_runtime_asset_publishes_generated_payload_copy() {
        let asset_root = std::env::temp_dir().join(format!(
            "sidereal-asset-runtime-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        let source_path = asset_root.join("textures/red.png");
        std::fs::create_dir_all(source_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&source_path, b"payload").expect("write source");
        let entry = RuntimeAssetCatalogEntry {
            asset_id: "texture.red".to_string(),
            asset_guid: generated_asset_guid("texture.red", &sha256_hex(b"payload")),
            shader_family: None,
            dependencies: Vec::new(),
            relative_cache_path: generated_relative_cache_path(
                "12345678-1234-1234-1234-123456789abc",
                "textures/red.png",
                "image/png",
            ),
            source_path: "textures/red.png".to_string(),
            content_type: "image/png".to_string(),
            byte_len: 7,
            sha256_hex: sha256_hex(b"payload"),
            bootstrap_required: true,
            startup_required: false,
        };

        let materialized = materialize_runtime_asset(&asset_root, &entry).expect("materialize");
        assert!(
            materialized
                .full_path
                .starts_with(asset_root.join("published_assets"))
        );
        assert_eq!(
            std::fs::read(materialized.full_path).expect("read"),
            b"payload"
        );
        let _ = std::fs::remove_dir_all(&asset_root);
    }

    #[test]
    fn catalog_version_changes_when_dependencies_change() {
        let base_entry = RuntimeAssetCatalogEntry {
            asset_id: "shader.main".to_string(),
            asset_guid: "guid-1".to_string(),
            shader_family: Some("effect".to_string()),
            dependencies: vec!["texture.a".to_string()],
            relative_cache_path: "shaders/guid-1.wgsl".to_string(),
            source_path: "shaders/main.wgsl".to_string(),
            content_type: "text/wgsl".to_string(),
            byte_len: 3,
            sha256_hex: "abc".to_string(),
            bootstrap_required: true,
            startup_required: false,
        };
        let mut changed_entry = base_entry.clone();
        changed_entry.dependencies.push("texture.b".to_string());

        assert_ne!(
            catalog_version(&[base_entry]),
            catalog_version(&[changed_entry])
        );
    }

    #[test]
    fn catalog_version_changes_when_startup_policy_changes() {
        let base_entry = RuntimeAssetCatalogEntry {
            asset_id: "audio.music.menu_loop".to_string(),
            asset_guid: "guid-1".to_string(),
            shader_family: None,
            dependencies: Vec::new(),
            relative_cache_path: "audio/guid-1.ogg".to_string(),
            source_path: "music/menu-loop.ogg".to_string(),
            content_type: "audio/ogg".to_string(),
            byte_len: 3,
            sha256_hex: "abc".to_string(),
            bootstrap_required: true,
            startup_required: false,
        };
        let mut changed_entry = base_entry.clone();
        changed_entry.startup_required = true;

        assert_ne!(
            catalog_version(&[base_entry]),
            catalog_version(&[changed_entry])
        );
    }
}
