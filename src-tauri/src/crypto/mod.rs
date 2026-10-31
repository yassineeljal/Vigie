//! Primitives cryptographiques : dérivation de clé (Argon2id), enveloppe de clés
//! (ChaCha20-Poly1305) et clé de récupération. Tous les secrets sont effacés au drop.

mod envelope;
mod kdf;
mod recovery;

pub use envelope::{unwrap_key, wrap_key};
pub use kdf::{KdfParams, derive_kek};
pub use recovery::RecoveryKey;

use rand::Rng;
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;

/// Clé symétrique de 256 bits, effacée de la mémoire à la destruction.
pub type Key = Zeroizing<[u8; KEY_LEN]>;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("échec de la dérivation de clé")]
    Kdf,
    #[error("déchiffrement impossible (clé incorrecte ou données corrompues)")]
    Decrypt,
    #[error("enveloppe mal formée")]
    Malformed,
    #[error("clé de récupération invalide")]
    InvalidRecoveryKey,
}

/// Octets aléatoires issus du CSPRNG du système.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::rng().fill_bytes(&mut buf);
    buf
}

/// Génère une nouvelle clé de 256 bits (DEK).
pub fn random_key() -> Key {
    Zeroizing::new(random_bytes::<KEY_LEN>())
}

pub fn hex_encode(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(bytes)
}
