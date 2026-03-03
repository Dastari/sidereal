mod bootstrap_dispatch;
mod config;
mod crypto;
mod error;
mod service;
mod starter_world;
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
pub use store::{AuthStore, InMemoryAuthStore, PostgresAuthStore};
pub use types::{
    Account, AccountCharacter, AuthMe, PasswordResetRequestResult, PasswordResetTokenRecord,
    RefreshTokenRecord,
};
