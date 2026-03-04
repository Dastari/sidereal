mod bootstrap_dispatch;
mod config;
mod crypto;
mod error;
mod service;
mod starter_world;
mod starter_world_scripts;
mod store;
mod types;

pub use bootstrap_dispatch::{
    BootstrapDispatcher, DirectBootstrapDispatcher, NoopBootstrapDispatcher,
    RecordingBootstrapDispatcher, UdpBootstrapDispatcher,
};
pub use config::AuthConfig;
pub use crypto::{
    hash_password, hash_token, normalize_email, now_epoch_s, validate_email, validate_password,
    verify_password,
};
pub use error::AuthError;
pub use service::AuthService;
pub use starter_world::{
    GraphStarterWorldPersister, NoopStarterWorldPersister, StarterWorldPersister,
    persist_starter_world_for_new_account,
};
pub use starter_world_scripts::{
    ScriptBundleRegistry, load_bundle_registry, load_new_account_config, load_world_init_config,
    scripts_root_dir,
};
pub use store::{AuthStore, InMemoryAuthStore, PostgresAuthStore};
pub use types::{
    Account, AccountCharacter, AuthMe, PasswordResetRequestResult, PasswordResetTokenRecord,
    RefreshTokenRecord,
};
