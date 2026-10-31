//! Enveloppe de clés : chiffre une clé (DEK) avec une autre (KEK ou clé de récupération).
//! Format : nonce (12 octets) || chiffré+tag. L'AAD lie l'enveloppe à son usage.

use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN, Key, random_bytes};

const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

pub fn wrap_key(kek: &Key, dek: &Key, aad: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher =
        ChaCha20Poly1305::new_from_slice(kek.as_ref()).map_err(|_| CryptoError::Malformed)?;
    let nonce_bytes = random_bytes::<NONCE_LEN>();
    let ct = cipher
        .encrypt(
            &Nonce::from(nonce_bytes),
            Payload {
                msg: dek.as_ref(),
                aad,
            },
        )
        .map_err(|_| CryptoError::Decrypt)?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

pub fn unwrap_key(kek: &Key, wrapped: &[u8], aad: &[u8]) -> Result<Key, CryptoError> {
    if wrapped.len() != NONCE_LEN + KEY_LEN + TAG_LEN {
        return Err(CryptoError::Malformed);
    }
    let (nonce, ct) = wrapped.split_at(NONCE_LEN);
    let nonce = Nonce::try_from(nonce).map_err(|_| CryptoError::Malformed)?;
    let cipher =
        ChaCha20Poly1305::new_from_slice(kek.as_ref()).map_err(|_| CryptoError::Malformed)?;
    let pt = Zeroizing::new(
        cipher
            .decrypt(&nonce, Payload { msg: ct, aad })
            .map_err(|_| CryptoError::Decrypt)?,
    );
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    key.copy_from_slice(&pt);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::random_key;

    #[test]
    fn aller_retour() {
        let (kek, dek) = (random_key(), random_key());
        let w = wrap_key(&kek, &dek, b"pw").unwrap();
        assert_eq!(*unwrap_key(&kek, &w, b"pw").unwrap(), *dek);
    }

    #[test]
    fn mauvaise_cle_refusee() {
        let (kek, dek) = (random_key(), random_key());
        let w = wrap_key(&kek, &dek, b"pw").unwrap();
        assert!(matches!(
            unwrap_key(&random_key(), &w, b"pw"),
            Err(CryptoError::Decrypt)
        ));
    }

    #[test]
    fn mauvais_aad_refuse() {
        let (kek, dek) = (random_key(), random_key());
        let w = wrap_key(&kek, &dek, b"pw").unwrap();
        assert!(unwrap_key(&kek, &w, b"recovery").is_err());
    }

    #[test]
    fn donnees_alterees_refusees() {
        let (kek, dek) = (random_key(), random_key());
        let mut w = wrap_key(&kek, &dek, b"pw").unwrap();
        let last = w.len() - 1;
        w[last] ^= 1;
        assert!(unwrap_key(&kek, &w, b"pw").is_err());
        assert!(matches!(
            unwrap_key(&kek, &w[..10], b"pw"),
            Err(CryptoError::Malformed)
        ));
    }

    #[test]
    fn nonces_distincts() {
        let (kek, dek) = (random_key(), random_key());
        assert_ne!(
            wrap_key(&kek, &dek, b"a").unwrap(),
            wrap_key(&kek, &dek, b"a").unwrap()
        );
    }
}
