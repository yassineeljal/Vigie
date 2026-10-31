//! Coffre : base SQLCipher dont la clé (DEK) est enveloppée par le mot de passe maître
//! (Argon2id → KEK) et par la clé de récupération. Voir ADR-002 et PLAN §5.5.

mod header;
mod policy;

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};
use secrecy::SecretString;
use zeroize::Zeroizing;

use crate::crypto::{
    self, CryptoError, KdfParams, Key, RecoveryKey, derive_kek, random_bytes, random_key,
    unwrap_key, wrap_key,
};
use header::{AAD_PASSWORD, AAD_RECOVERY, Header};
pub use policy::check_master_password;

pub const DB_FILE: &str = "vault.db";
pub const HEADER_FILE: &str = "vault.header.json";
const SALT_LEN: usize = 16;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("un coffre existe déjà à cet emplacement")]
    AlreadyExists,
    #[error("aucun coffre à cet emplacement")]
    NotFound,
    #[error("mot de passe ou clé de récupération incorrect")]
    BadCredentials,
    #[error("mot de passe maître trop faible (score zxcvbn {0}/4, minimum 3)")]
    WeakPassword(u8),
    #[error("en-tête du coffre corrompu ou non supporté")]
    BadHeader,
    #[error("erreur crypto : {0}")]
    Crypto(CryptoError),
    #[error("erreur de base de données : {0}")]
    Db(#[from] rusqlite::Error),
    #[error("erreur de migration : {0}")]
    Migration(#[from] rusqlite_migration::Error),
    #[error("erreur d'E/S : {0}")]
    Io(#[from] std::io::Error),
}

impl From<CryptoError> for VaultError {
    fn from(e: CryptoError) -> Self {
        match e {
            CryptoError::Decrypt => VaultError::BadCredentials,
            CryptoError::Malformed => VaultError::BadHeader,
            other => VaultError::Crypto(other),
        }
    }
}

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("../../migrations/001_init.sql")),
        M::up(include_str!(
            "../../migrations/002_password_check_nullable.sql"
        )),
    ])
}

/// Coffre déverrouillé. Le verrouiller (= drop) ferme la connexion SQLCipher,
/// ce qui efface la clé de la mémoire du moteur ; Vigie ne garde pas la DEK en mémoire.
pub struct Vault {
    conn: Connection,
}

impl Vault {
    pub fn exists(dir: &Path) -> bool {
        dir.join(HEADER_FILE).exists()
    }

    /// Crée un coffre. Retourne la clé de récupération, à afficher une seule fois.
    pub fn create(
        dir: &Path,
        password: &SecretString,
        params: KdfParams,
    ) -> Result<(Self, RecoveryKey), VaultError> {
        if dir.join(HEADER_FILE).exists() || dir.join(DB_FILE).exists() {
            return Err(VaultError::AlreadyExists);
        }
        check_master_password(password)?;
        fs::create_dir_all(dir)?;

        let dek = random_key();
        let recovery = RecoveryKey::generate();
        let salt = random_bytes::<SALT_LEN>();
        let kek = derive_kek(password, &salt, params)?;

        let header = Header::new(
            params,
            &salt,
            &wrap_key(&kek, &dek, AAD_PASSWORD)?,
            &wrap_key(recovery.key(), &dek, AAD_RECOVERY)?,
        );

        let db_path = dir.join(DB_FILE);
        let conn = match open_and_migrate(&db_path, &dek) {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&db_path);
                return Err(e);
            }
        };
        // L'en-tête est écrit en dernier : sans lui, le coffre n'existe pas.
        if let Err(e) = header.write(&dir.join(HEADER_FILE)) {
            drop(conn);
            let _ = fs::remove_file(&db_path);
            return Err(e);
        }
        Ok((Self { conn }, recovery))
    }

    pub fn unlock(dir: &Path, password: &SecretString) -> Result<Self, VaultError> {
        let header = Header::read(&dir.join(HEADER_FILE))?;
        let kek = derive_kek(password, &header.salt()?, header.kdf)?;
        let dek = unwrap_key(&kek, &header.wrapped_password()?, AAD_PASSWORD)?;
        Self::open(dir, &dek)
    }

    pub fn unlock_with_recovery(dir: &Path, recovery: &RecoveryKey) -> Result<Self, VaultError> {
        let header = Header::read(&dir.join(HEADER_FILE))?;
        let dek = unwrap_key(recovery.key(), &header.wrapped_recovery()?, AAD_RECOVERY)?;
        Self::open(dir, &dek)
    }

    /// Change le mot de passe maître : seule la DEK est rechiffrée, pas la base (US-04).
    pub fn change_password(
        dir: &Path,
        old: &SecretString,
        new: &SecretString,
        params: KdfParams,
    ) -> Result<(), VaultError> {
        let header = Header::read(&dir.join(HEADER_FILE))?;
        let kek = derive_kek(old, &header.salt()?, header.kdf)?;
        let dek = unwrap_key(&kek, &header.wrapped_password()?, AAD_PASSWORD)?;
        rewrap_password(dir, header, &dek, new, params)
    }

    /// Mot de passe oublié : la clé de récupération permet d'en définir un nouveau.
    pub fn reset_password(
        dir: &Path,
        recovery: &RecoveryKey,
        new: &SecretString,
        params: KdfParams,
    ) -> Result<(), VaultError> {
        let header = Header::read(&dir.join(HEADER_FILE))?;
        let dek = unwrap_key(recovery.key(), &header.wrapped_recovery()?, AAD_RECOVERY)?;
        rewrap_password(dir, header, &dek, new, params)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn open(dir: &Path, dek: &Key) -> Result<Self, VaultError> {
        let path: PathBuf = dir.join(DB_FILE);
        if !path.exists() {
            return Err(VaultError::NotFound);
        }
        Ok(Self {
            conn: open_and_migrate(&path, dek)?,
        })
    }
}

fn rewrap_password(
    dir: &Path,
    mut header: Header,
    dek: &Key,
    new: &SecretString,
    params: KdfParams,
) -> Result<(), VaultError> {
    check_master_password(new)?;
    let salt = random_bytes::<SALT_LEN>();
    let kek = derive_kek(new, &salt, params)?;
    header.set_password_envelope(params, &salt, &wrap_key(&kek, dek, AAD_PASSWORD)?);
    header.write(&dir.join(HEADER_FILE))
}

fn open_and_migrate(path: &Path, dek: &Key) -> Result<Connection, VaultError> {
    let mut conn = Connection::open(path)?;
    let hex = Zeroizing::new(crypto::hex_encode(dek.as_ref()));
    let pragma = Zeroizing::new(format!("PRAGMA key = \"x'{}'\";", hex.as_str()));
    conn.execute_batch(&pragma)?;
    // Une mauvaise clé se manifeste ici : « file is not a database ».
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map_err(|_| VaultError::BadCredentials)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrations().to_latest(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
mod tests;
