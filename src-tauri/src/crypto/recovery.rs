//! Clé de récupération : 256 bits aléatoires, affichés une seule fois en base32
//! (groupes de 5 caractères, faciles à imprimer et à recopier).

use data_encoding::BASE32_NOPAD;
use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN, Key, random_key};

pub struct RecoveryKey(Key);

impl RecoveryKey {
    pub fn generate() -> Self {
        Self(random_key())
    }

    pub fn key(&self) -> &Key {
        &self.0
    }

    /// Représentation affichable : `ABCDE-FGHIJ-…`.
    pub fn to_display(&self) -> Zeroizing<String> {
        let raw = Zeroizing::new(BASE32_NOPAD.encode(self.0.as_ref()));
        let groups: Vec<&str> = raw
            .as_bytes()
            .chunks(5)
            .map(|c| std::str::from_utf8(c).expect("base32 ASCII"))
            .collect();
        Zeroizing::new(groups.join("-"))
    }

    /// Accepte tirets, espaces et minuscules.
    pub fn parse(input: &str) -> Result<Self, CryptoError> {
        let cleaned: Zeroizing<String> = Zeroizing::new(
            input
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '-')
                .map(|c| c.to_ascii_uppercase())
                .collect(),
        );
        let bytes = Zeroizing::new(
            BASE32_NOPAD
                .decode(cleaned.as_bytes())
                .map_err(|_| CryptoError::InvalidRecoveryKey)?,
        );
        if bytes.len() != KEY_LEN {
            return Err(CryptoError::InvalidRecoveryKey);
        }
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        key.copy_from_slice(&bytes);
        Ok(Self(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affichage_puis_lecture() {
        let k = RecoveryKey::generate();
        let shown = k.to_display();
        assert_eq!(shown.split('-').count(), 11);
        let back = RecoveryKey::parse(&shown.to_lowercase().replace('-', " ")).unwrap();
        assert_eq!(**back.key(), **k.key());
    }

    #[test]
    fn entrees_invalides() {
        assert!(RecoveryKey::parse("").is_err());
        assert!(RecoveryKey::parse("AAAAA-BBBBB").is_err());
        assert!(RecoveryKey::parse("!!!!!").is_err());
    }
}
