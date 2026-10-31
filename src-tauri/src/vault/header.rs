//! En-tête du coffre (JSON) : paramètres Argon2id, sel et enveloppes de la DEK.
//! Ne contient aucun secret en clair.

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use data_encoding::BASE64;
use serde::{Deserialize, Serialize};

use super::VaultError;
use crate::crypto::KdfParams;

pub const AAD_PASSWORD: &[u8] = b"vigie/v1/password";
pub const AAD_RECOVERY: &[u8] = b"vigie/v1/recovery";
const VERSION: u32 = 1;

// Bornes de sécurité : un en-tête altéré ne doit pas pouvoir provoquer un déni de service.
const MAX_M_KIB: u32 = 2 * 1024 * 1024;
const MAX_T: u32 = 64;
const MAX_P: u32 = 16;

#[derive(Debug, Serialize, Deserialize)]
pub struct Header {
    version: u32,
    pub kdf: KdfParams,
    salt: String,
    wrapped_password: String,
    wrapped_recovery: String,
}

impl Header {
    pub fn new(kdf: KdfParams, salt: &[u8], wrapped_pw: &[u8], wrapped_rec: &[u8]) -> Self {
        Self {
            version: VERSION,
            kdf,
            salt: BASE64.encode(salt),
            wrapped_password: BASE64.encode(wrapped_pw),
            wrapped_recovery: BASE64.encode(wrapped_rec),
        }
    }

    pub fn read(path: &Path) -> Result<Self, VaultError> {
        let bytes = fs::read(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => VaultError::NotFound,
            _ => VaultError::Io(e),
        })?;
        let h: Header = serde_json::from_slice(&bytes).map_err(|_| VaultError::BadHeader)?;
        if h.version != VERSION
            || h.kdf.m_kib == 0
            || h.kdf.m_kib > MAX_M_KIB
            || h.kdf.t == 0
            || h.kdf.t > MAX_T
            || h.kdf.p == 0
            || h.kdf.p > MAX_P
        {
            return Err(VaultError::BadHeader);
        }
        Ok(h)
    }

    /// Écriture atomique : fichier temporaire + fsync + rename.
    /// Un échec en cours d'écriture ne doit jamais détruire l'ancien en-tête.
    pub fn write(&self, path: &Path) -> Result<(), VaultError> {
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(self).map_err(|_| VaultError::BadHeader)?;
        let mut f = File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn set_password_envelope(&mut self, kdf: KdfParams, salt: &[u8], wrapped: &[u8]) {
        self.kdf = kdf;
        self.salt = BASE64.encode(salt);
        self.wrapped_password = BASE64.encode(wrapped);
    }

    fn decode(s: &str) -> Result<Vec<u8>, VaultError> {
        BASE64
            .decode(s.as_bytes())
            .map_err(|_| VaultError::BadHeader)
    }
    pub fn salt(&self) -> Result<Vec<u8>, VaultError> {
        Self::decode(&self.salt)
    }
    pub fn wrapped_password(&self) -> Result<Vec<u8>, VaultError> {
        Self::decode(&self.wrapped_password)
    }
    pub fn wrapped_recovery(&self) -> Result<Vec<u8>, VaultError> {
        Self::decode(&self.wrapped_recovery)
    }
}
