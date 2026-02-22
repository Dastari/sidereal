use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

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
pub struct AssetCacheIndexRecord {
    pub asset_version: u64,
    pub sha256_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AssetCacheIndex {
    pub by_asset_id: HashMap<String, AssetCacheIndexRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamableAssetSource {
    pub asset_id: &'static str,
    pub relative_cache_path: &'static str,
    pub content_type: &'static str,
}

pub fn default_streamable_asset_sources() -> &'static [StreamableAssetSource] {
    &[
        StreamableAssetSource {
            asset_id: "corvette_01",
            relative_cache_path: "models/corvette_01/corvette_01.gltf",
            content_type: "model/gltf+json",
        },
        StreamableAssetSource {
            asset_id: "corvette_01_gltf",
            relative_cache_path: "models/corvette_01/corvette_01.gltf",
            content_type: "model/gltf+json",
        },
        StreamableAssetSource {
            asset_id: "corvette_01_bin",
            relative_cache_path: "models/corvette_01/corvette_01.bin",
            content_type: "application/octet-stream",
        },
        StreamableAssetSource {
            asset_id: "corvette_01_png",
            relative_cache_path: "models/corvette_01/corvette_01.png",
            content_type: "image/png",
        },
        StreamableAssetSource {
            asset_id: "starfield_wgsl",
            relative_cache_path: "shaders/starfield.wgsl",
            content_type: "text/plain; charset=utf-8",
        },
        StreamableAssetSource {
            asset_id: "space_background_wgsl",
            relative_cache_path: "shaders/simple_space_background.wgsl",
            content_type: "text/plain; charset=utf-8",
        },
    ]
}

pub fn default_asset_dependencies() -> HashMap<String, Vec<String>> {
    HashMap::from([(
        "corvette_01".to_string(),
        vec![
            "corvette_01_gltf".to_string(),
            "corvette_01_bin".to_string(),
            "corvette_01_png".to_string(),
        ],
    )])
}

fn normalize_joined_relative_path(base_relative_path: &str, uri: &str) -> Option<String> {
    if uri.starts_with("data:") || uri.contains("://") || uri.starts_with('/') {
        return None;
    }
    let base_dir = Path::new(base_relative_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let joined = base_dir.join(uri);
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => return None,
        }
    }
    let as_string = normalized.to_string_lossy().replace('\\', "/");
    if as_string.is_empty() {
        None
    } else {
        Some(as_string)
    }
}

pub fn gltf_dependency_relative_paths(
    gltf_relative_path: &str,
    gltf_bytes: &[u8],
) -> HashSet<String> {
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(gltf_bytes) else {
        return HashSet::new();
    };
    let mut deps = HashSet::new();
    for section_name in ["buffers", "images"] {
        let Some(section) = json.get(section_name).and_then(|v| v.as_array()) else {
            continue;
        };
        for entry in section {
            let Some(uri) = entry.get("uri").and_then(|v| v.as_str()) else {
                continue;
            };
            if let Some(path) = normalize_joined_relative_path(gltf_relative_path, uri) {
                deps.insert(path);
            }
        }
    }
    deps
}

pub fn expand_required_assets(
    required_asset_ids: &HashSet<String>,
    dependencies_by_asset_id: &HashMap<String, Vec<String>>,
) -> HashSet<String> {
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

pub fn cache_index_path(asset_root: &str) -> PathBuf {
    PathBuf::from(asset_root)
        .join("data/cache_stream")
        .join("index.json")
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

    #[test]
    fn gltf_dependency_paths_extracts_buffers_and_images() {
        let gltf = br#"{
            "buffers":[{"uri":"corvette_01.bin"},{"uri":"../shared/mesh.bin"}],
            "images":[{"uri":"textures/corvette_01.png"},{"uri":"data:image/png;base64,AAAA"}]
        }"#;
        let deps = gltf_dependency_relative_paths("models/corvette_01/corvette_01.gltf", gltf);
        assert!(deps.contains("models/corvette_01/corvette_01.bin"));
        assert!(deps.contains("models/shared/mesh.bin"));
        assert!(deps.contains("models/corvette_01/textures/corvette_01.png"));
        assert_eq!(deps.len(), 3);
    }
}
