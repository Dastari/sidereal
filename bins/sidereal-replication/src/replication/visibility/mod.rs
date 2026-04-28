// This module is mechanically split across domain-named include files.
// Keep the include order stable; the included files share this module scope.
include!("config.rs");
include!("client_registry.rs");
include!("context_cache.rs");
include!("entity_cache.rs");
include!("diagnostics.rs");
include!("local_view.rs");
include!("landmarks.rs");
include!("spatial_index.rs");
include!("membership.rs");
include!("policy.rs");
include!("metrics.rs");
