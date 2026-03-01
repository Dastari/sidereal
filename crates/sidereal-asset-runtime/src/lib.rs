use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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
            relative_cache_path: "sprites/ships/corvette.png",
            content_type: "image/png",
        },
        StreamableAssetSource {
            asset_id: "starfield_wgsl",
            relative_cache_path: "shaders/starfield.wgsl",
            content_type: "text/plain; charset=utf-8",
        },
        StreamableAssetSource {
            asset_id: "space_background_wgsl",
            relative_cache_path: "shaders/space_background.wgsl",
            content_type: "text/plain; charset=utf-8",
        },
        StreamableAssetSource {
            asset_id: "sprite_pixel_effect_wgsl",
            relative_cache_path: "shaders/sprite_pixel_effect.wgsl",
            content_type: "text/plain; charset=utf-8",
        },
        StreamableAssetSource {
            asset_id: "space_bg_flare_white_png",
            relative_cache_path: "textures/spacescape/flare-white-small1.png",
            content_type: "image/png",
        },
        StreamableAssetSource {
            asset_id: "space_bg_flare_blue_png",
            relative_cache_path: "textures/spacescape/flare-blue-purple2.png",
            content_type: "image/png",
        },
        StreamableAssetSource {
            asset_id: "space_bg_flare_red_png",
            relative_cache_path: "textures/spacescape/flare-red-yellow1.png",
            content_type: "image/png",
        },
        StreamableAssetSource {
            asset_id: "space_bg_flare_sun_png",
            relative_cache_path: "textures/spacescape/sun.png",
            content_type: "image/png",
        },
    ]
}

pub fn default_asset_dependencies() -> HashMap<String, Vec<String>> {
    HashMap::from([(
        "space_background_wgsl".to_string(),
        vec![
            "space_bg_flare_white_png".to_string(),
            "space_bg_flare_blue_png".to_string(),
            "space_bg_flare_red_png".to_string(),
            "space_bg_flare_sun_png".to_string(),
        ],
    )])
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
