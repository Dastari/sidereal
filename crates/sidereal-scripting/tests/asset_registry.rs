use sidereal_scripting::{load_asset_registry_from_root, load_asset_registry_from_source};
use std::path::{Path, PathBuf};

fn write_registry_script(script_body: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sidereal_scripting_registry_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(root.join("assets")).expect("create scripts root");
    std::fs::write(root.join("assets/registry.lua"), script_body).expect("write registry");
    root
}

#[test]
fn loads_asset_registry_from_lua() {
    let root = write_registry_script(
        r#"
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "shader.main",
      source_path = "shaders/main.wgsl",
      content_type = "text/plain; charset=utf-8",
      dependencies = {},
      bootstrap_required = true,
      startup_required = false,
    },
    {
      asset_id = "texture.bg",
      source_path = "textures/bg.png",
      content_type = "image/png",
      dependencies = { "shader.main" },
      bootstrap_required = false,
      startup_required = true,
    },
  },
}
"#,
    );

    let registry = load_asset_registry_from_root(&root).expect("load registry");
    assert_eq!(registry.schema_version, 1);
    assert_eq!(registry.assets.len(), 2);
    assert!(
        registry
            .bootstrap_required_asset_ids()
            .contains("shader.main")
    );
    assert!(registry.startup_required_asset_ids().contains("texture.bg"));
    assert_eq!(
        registry
            .dependencies_by_asset_id()
            .get("texture.bg")
            .cloned()
            .unwrap_or_default(),
        vec!["shader.main".to_string()]
    );
}

#[test]
fn loads_asset_registry_from_source() {
    let registry = load_asset_registry_from_source(
        r#"
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "shader.main",
      shader_family = "world_sprite_generic",
      source_path = "shaders/main.wgsl",
      content_type = "text/wgsl",
      dependencies = {},
      bootstrap_required = true,
      startup_required = false,
    },
  },
}
"#,
        Path::new("assets/registry.lua"),
    )
    .expect("load registry from source");

    assert_eq!(registry.schema_version, 1);
    assert_eq!(registry.assets.len(), 1);
    assert_eq!(
        registry.assets[0].shader_family.as_deref(),
        Some("world_sprite_generic")
    );
}

#[test]
fn loads_asset_registry_editor_schema_from_source() {
    let registry = load_asset_registry_from_source(
        r#"
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "planet_visual_wgsl",
      shader_family = "world_polygon_planet",
      source_path = "shaders/planet_visual.wgsl",
      content_type = "text/wgsl",
      dependencies = {},
      bootstrap_required = true,
      startup_required = false,
      editor_schema = {
        uniforms = {
          atmosphere_alpha = {
            kind = "Float",
            label = "Atmosphere Alpha",
            min = 0.0,
            max = 1.0,
            step = 0.01,
            default = 0.48,
            group = "Atmosphere",
          },
          blend_mode = {
            kind = "Enum",
            options = {
              { value = "screen", label = "Screen" },
              { value = "add", label = "Add" },
            },
            default = "screen",
          },
        },
        presets = {
          {
            preset_id = "earth_like",
            label = "Earth-like",
            values = {
              atmosphere_alpha = 0.48,
            },
          },
        },
      },
    },
  },
}
"#,
        Path::new("assets/registry.lua"),
    )
    .expect("load registry from source");

    let asset = &registry.assets[0];
    let schema = asset.editor_schema.as_ref().expect("editor schema");
    assert_eq!(schema.uniforms.len(), 2);
    assert_eq!(schema.presets.len(), 1);
    assert_eq!(schema.uniforms[0].field_path, "atmosphere_alpha");
    assert_eq!(
        schema.uniforms[0].label.as_deref(),
        Some("Atmosphere Alpha")
    );
    assert_eq!(schema.uniforms[1].options.len(), 2);
    assert_eq!(schema.presets[0].preset_id, "earth_like");
}

#[test]
fn rejects_invalid_editor_schema_ranges() {
    let err = load_asset_registry_from_source(
        r#"
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "planet_visual_wgsl",
      source_path = "shaders/planet_visual.wgsl",
      content_type = "text/wgsl",
      dependencies = {},
      bootstrap_required = true,
      startup_required = false,
      editor_schema = {
        uniforms = {
          atmosphere_alpha = {
            kind = "Float",
            min = 2.0,
            max = 1.0,
          },
        },
      },
    },
  },
}
"#,
        Path::new("assets/registry.lua"),
    )
    .expect_err("expected validation error");
    assert!(err.to_string().contains("min > max"));
}

#[test]
fn rejects_unknown_dependencies() {
    let root = write_registry_script(
        r#"
return {
  schema_version = 1,
  assets = {
    {
      asset_id = "texture.bg",
      source_path = "textures/bg.png",
      content_type = "image/png",
      dependencies = { "shader.missing" },
      bootstrap_required = false,
      startup_required = false,
    },
  },
}
"#,
    );

    let err = load_asset_registry_from_root(&root).expect_err("expected validation error");
    assert!(err.to_string().contains("unknown dependency"));
}
