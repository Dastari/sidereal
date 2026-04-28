// This module is mechanically split across domain-named include files.
// Keep the include order stable; the included files share this module scope.
include!("paths.rs");
include!("resources.rs");
include!("catalog.rs");
include!("asset_registry.rs");
include!("persistence.rs");
include!("world_init.rs");
include!("bundle_spawn.rs");
include!("lua_context.rs");
include!("render_authoring.rs");
