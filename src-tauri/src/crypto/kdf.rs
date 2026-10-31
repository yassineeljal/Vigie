use argon2::{Algorithm, Argon2, Params, Version};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN, Key};

/// Paramètres Argon2id, stockés dans l'en-tête pour pouvoir les relever plus tard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl Default for KdfParams {
    /// Calibré le 2026-10-31 (~0,5 s sur le Mac de dev) : 256 MiB, t=6, p=1.
    fn default() -> Self {
        Self {
            m_kib: 256 * 1024,
            t: 6,
            p: 1,
        }
    }
}

/// Dérive la clé de chiffrement de clé (KEK) depuis le mot de passe maître.
pub fn derive_kek(
    password: &SecretString,
    salt: &[u8],
    params: KdfParams,
) -> Result<Key, CryptoError> {
    let p = Params::new(params.m_kib, params.t, params.p, Some(KEY_LEN))
        .map_err(|_| CryptoError::Kdf)?;
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
        .hash_password_into(password.expose_secret().as_bytes(), salt, out.as_mut())
        .map_err(|_| CryptoError::Kdf)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast() -> KdfParams {
        KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
        }
    }

    #[test]
    fn deterministe_pour_meme_sel() {
        let pw = SecretString::from("correct horse");
        let a = derive_kek(&pw, b"0123456789abcdef", fast()).unwrap();
        let b = derive_kek(&pw, b"0123456789abcdef", fast()).unwrap();
        assert_eq!(*a, *b);
    }

    #[test]
    fn differe_selon_sel_et_mot_de_passe() {
        let pw = SecretString::from("correct horse");
        let a = derive_kek(&pw, b"0123456789abcdef", fast()).unwrap();
        let b = derive_kek(&pw, b"fedcba9876543210", fast()).unwrap();
        let c = derive_kek(&SecretString::from("autre"), b"0123456789abcdef", fast()).unwrap();
        assert_ne!(*a, *b);
        assert_ne!(*a, *c);
    }

    #[test]
    fn parametres_invalides_refuses() {
        let pw = SecretString::from("x");
        let bad = KdfParams {
            m_kib: 0,
            t: 0,
            p: 0,
        };
        assert!(derive_kek(&pw, b"0123456789abcdef", bad).is_err());
    }

    /// Calibrage : `cargo test --release calibrate -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn calibrate() {
        let pw = SecretString::from("calibrage");
        for (m, t) in [(64, 3), (128, 3), (256, 2), (256, 3), (512, 2)] {
            let params = KdfParams {
                m_kib: m * 1024,
                t,
                p: 1,
            };
            let start = std::time::Instant::now();
            derive_kek(&pw, b"0123456789abcdef", params).unwrap();
            println!("m={m} MiB t={t} p=1 -> {:?}", start.elapsed());
        }
    }
}
