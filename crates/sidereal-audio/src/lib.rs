mod catalog;
mod defaults;
mod error;
mod validate;

pub use catalog::*;
pub use defaults::apply_clip_defaults;
pub use error::AudioRegistryError;
pub use validate::validate_audio_registry;

use sha2::{Digest, Sha256};

pub fn audio_registry_version(registry: &AudioRegistry) -> Result<String, serde_json::Error> {
    let encoded = serde_json::to_vec(registry)?;
    let mut digest = Sha256::new();
    digest.update(b"sidereal-audio-registry-v1");
    digest.update(encoded);
    Ok(format!("{:x}", digest.finalize()))
}
