//! Commandes IPC du coffre (US-01 à US-04). Les mots de passe arrivent en `String`
//! (contrainte IPC) et sont convertis immédiatement en `SecretString`.

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use super::state::AppState;
use crate::crypto::RecoveryKey;
use crate::vault::{Vault, VaultError, check_master_password};

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    AlreadyExists,
    NotFound,
    BadCredentials,
    WeakPassword,
    InvalidRecoveryKey,
    InvalidInput,
    Network,
    OfflineMode,
    Locked,
    Internal,
}

#[derive(Debug, Serialize, Type)]
pub struct CommandError {
    pub kind: ErrorKind,
    pub message: String,
}

impl From<VaultError> for CommandError {
    fn from(e: VaultError) -> Self {
        let kind = match &e {
            VaultError::AlreadyExists => ErrorKind::AlreadyExists,
            VaultError::NotFound => ErrorKind::NotFound,
            VaultError::BadCredentials => ErrorKind::BadCredentials,
            VaultError::WeakPassword(_) => ErrorKind::WeakPassword,
            _ => ErrorKind::Internal,
        };
        // Le message ne contient jamais de secret (voir VaultError).
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

impl From<crate::domain::DomainError> for CommandError {
    fn from(e: crate::domain::DomainError) -> Self {
        use crate::domain::DomainError as D;
        let kind = match &e {
            D::InvalidDomain(_) | D::InvalidInput(_) => ErrorKind::InvalidInput,
            D::NotFound => ErrorKind::NotFound,
            D::Db(_) => ErrorKind::Internal,
        };
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

impl From<crate::import::ImportError> for CommandError {
    fn from(e: crate::import::ImportError) -> Self {
        use crate::import::ImportError as I;
        match e {
            I::Domain(d) => d.into(),
            other => Self {
                kind: ErrorKind::InvalidInput,
                message: other.to_string(),
            },
        }
    }
}

impl From<crate::intel::IntelError> for CommandError {
    fn from(e: crate::intel::IntelError) -> Self {
        use crate::intel::IntelError as I;
        let kind = match &e {
            I::OfflineMode => ErrorKind::OfflineMode,
            I::NoApiKey | I::Unauthorized => ErrorKind::BadCredentials,
            I::Db(_) | I::Keychain(_) => ErrorKind::Internal,
            _ => ErrorKind::Network,
        };
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

impl CommandError {
    pub fn locked() -> Self {
        Self {
            kind: ErrorKind::Locked,
            message: "coffre verrouillé".into(),
        }
    }

    fn invalid_recovery() -> Self {
        Self {
            kind: ErrorKind::InvalidRecoveryKey,
            message: "clé de récupération invalide".into(),
        }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;

#[derive(Debug, Serialize, Type)]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub auto_lock_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct VaultLocked;

fn status(state: &AppState) -> VaultStatus {
    VaultStatus {
        exists: Vault::exists(&state.dir),
        unlocked: state.is_unlocked(),
        auto_lock_minutes: state.auto_lock_minutes(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    status(&state)
}

/// Score zxcvbn (0-4) pour la jauge de l'écran de création.
#[tauri::command]
#[specta::specta]
pub fn password_strength(password: String) -> u8 {
    let pw = SecretString::from(password);
    match check_master_password(&pw) {
        Ok(()) => 3, // ≥ 3 : accepté (le score exact n'est pas nécessaire côté UI)
        Err(VaultError::WeakPassword(s)) => s,
        Err(_) => 0,
    }
}

/// Crée le coffre et retourne la clé de récupération, à n'afficher qu'une fois.
#[tauri::command]
#[specta::specta]
pub async fn create_vault(state: State<'_, AppState>, password: String) -> CmdResult<String> {
    let pw = SecretString::from(password);
    let (vault, recovery) = Vault::create(&state.dir, &pw, state.kdf)?;
    state.set_vault(vault);
    Ok(recovery.to_display().to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn unlock_vault(state: State<'_, AppState>, password: String) -> CmdResult<()> {
    let pw = SecretString::from(password);
    state.set_vault(Vault::unlock(&state.dir, &pw)?);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn unlock_with_recovery_key(
    state: State<'_, AppState>,
    recovery_key: String,
) -> CmdResult<()> {
    let key = RecoveryKey::parse(&recovery_key).map_err(|_| CommandError::invalid_recovery())?;
    state.set_vault(Vault::unlock_with_recovery(&state.dir, &key)?);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn lock_vault(app: AppHandle, state: State<'_, AppState>) {
    if state.lock() {
        let _ = VaultLocked.emit(&app);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn change_master_password(
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> CmdResult<()> {
    let (old, new) = (
        SecretString::from(old_password),
        SecretString::from(new_password),
    );
    Vault::change_password(&state.dir, &old, &new, state.kdf)?;
    Ok(())
}

/// Mot de passe oublié : définit un nouveau mot de passe à l'aide de la clé de récupération.
#[tauri::command]
#[specta::specta]
pub async fn reset_master_password(
    state: State<'_, AppState>,
    recovery_key: String,
    new_password: String,
) -> CmdResult<()> {
    let key = RecoveryKey::parse(&recovery_key).map_err(|_| CommandError::invalid_recovery())?;
    let new = SecretString::from(new_password);
    Vault::reset_password(&state.dir, &key, &new, state.kdf)?;
    Ok(())
}

/// Signale une activité de l'utilisateur (repousse le verrouillage automatique).
#[tauri::command]
#[specta::specta]
pub fn touch_activity(state: State<'_, AppState>) {
    state.touch();
}

#[tauri::command]
#[specta::specta]
pub fn set_auto_lock_minutes(state: State<'_, AppState>, minutes: u32) -> CmdResult<()> {
    if !state.is_unlocked() {
        return Err(CommandError::locked());
    }
    state.set_auto_lock_minutes(minutes)?;
    Ok(())
}
