use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AudioRegistryError {
    #[error("audio registry contract violation: {0}")]
    Contract(String),
}
