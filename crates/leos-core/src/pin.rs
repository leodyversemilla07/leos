//! Turning a wallet PIN into a secret the server can verify.
//!
//! The user types a four to six digit PIN. On its own that is far too
//! weak to send over a network, and too weak to store. So the device
//! stretches it into a 32 byte secret with Argon2id, salted with a random
//! per-device salt and mixed with a secret held in the device keystore.
//!
//! The server stores a second Argon2id hash of that secret and never sees
//! the PIN. Two properties follow:
//!
//! * a stolen database does not reveal the PIN, and
//! * a stolen verifier cannot be replayed against a different device,
//!   because every device mixes in its own secret and salt.

use argon2::{Algorithm, Argon2, Params, Version};
use rand::Rng;

use crate::error::LeosError;

/// The length of the derived secret, in bytes.
pub const SECRET_LEN: usize = 32;

/// The length of the device secret, in bytes.
pub const DEVICE_SECRET_LEN: usize = 32;

/// The length of the client salt, in bytes.
pub const SALT_LEN: usize = 16;

/// Argon2id parameters for the device side derivation.
///
/// These are the parameters a phone can afford: 64 MiB and three passes take
/// roughly a quarter of a second on a mid range handset, which is slow
/// enough to make an offline attack on a stolen phone expensive.
fn device_params() -> Params {
    Params::new(64 * 1024, 3, 1, None).expect("valid argon2 parameters")
}

/// Derives the secret that is sent to the server.
///
/// * `pin` is the PIN the user typed.
/// * `device_secret` is a random secret kept in the device keystore.
/// * `salt` is a random salt kept alongside it.
///
/// The result is hex encoded, so it can travel in JSON without ambiguity.
pub fn derive_secret(pin: &str, device_secret: &str, salt: &str) -> Result<String, LeosError> {
    if device_secret.len() < 16 {
        return Err(LeosError::InvalidPin(
            "the device secret is too short".to_string(),
        ));
    }
    if salt.len() < 8 {
        return Err(LeosError::InvalidPin(
            "the device salt is too short".to_string(),
        ));
    }

    let salt_bytes = salt.as_bytes();
    if salt_bytes.len() < 8 {
        return Err(LeosError::InvalidPin(
            "the device salt is too short".to_string(),
        ));
    }

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, device_params());
    let mut output = [0u8; SECRET_LEN];
    argon2
        .hash_password_into(
            format!("{pin}{device_secret}").as_bytes(),
            salt_bytes,
            &mut output,
        )
        .map_err(|_| LeosError::Internal("could not derive the PIN secret".to_string()))?;

    Ok(output.iter().map(|b| format!("{b:02x}")).collect())
}

/// Generates a fresh device secret.
pub fn generate_device_secret() -> String {
    random_hex(DEVICE_SECRET_LEN)
}

/// Generates a fresh client salt.
pub fn generate_salt() -> String {
    random_hex(SALT_LEN)
}

/// Encodes random bytes as lowercase hex.
pub fn random_hex(len: usize) -> String {
    let bytes: Vec<u8> = (0..len).map(|_| rand::rng().random()).collect();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn derivation_is_deterministic() {
        let secret =
            derive_secret("4821", "device-secret-value-0123456789", "saltsaltsalt").unwrap();
        let again =
            derive_secret("4821", "device-secret-value-0123456789", "saltsaltsalt").unwrap();
        assert_eq!(secret, again);
    }

    #[test]
    fn different_inputs_give_different_secrets() {
        let base = derive_secret("4821", "device-secret-value-0123456789", "saltsaltsalt").unwrap();
        let other_pin =
            derive_secret("4822", "device-secret-value-0123456789", "saltsaltsalt").unwrap();
        let other_device =
            derive_secret("4821", "different-device-secret-0123456789", "saltsaltsalt").unwrap();
        let other_salt =
            derive_secret("4821", "device-secret-value-0123456789", "othersalt12").unwrap();

        assert_ne!(base, other_pin);
        assert_ne!(base, other_device);
        assert_ne!(base, other_salt);
    }

    #[test]
    fn the_secret_is_the_expected_length() {
        let secret =
            derive_secret("4821", "device-secret-value-0123456789", "saltsaltsalt").unwrap();
        assert_eq!(secret.len(), SECRET_LEN * 2);
        assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
