//! Commandes IPC d'export et de sauvegarde (US-60, US-61). Les chemins sont choisis par
//! l'utilisateur dans les boîtes de dialogue natives, côté Rust.

use std::path::Path;

use secrecy::SecretString;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::state::AppState;
use super::vault::{CmdResult, CommandError, ErrorKind};
use crate::portability::{self, BackupError, ExportFormat, ExportOptions};
use crate::vault::VaultError;

#[derive(Debug, Serialize, Type)]
pub struct SavedFile {
    pub path: String,
    pub size_bytes: u32,
}

fn io_err(e: std::io::Error) -> CommandError {
    CommandError {
        kind: ErrorKind::Internal,
        message: format!("écriture impossible : {e}"),
    }
}

impl From<BackupError> for CommandError {
    fn from(e: BackupError) -> Self {
        match e {
            BackupError::Vault(v) => v.into(),
            other => Self {
                kind: ErrorKind::InvalidInput,
                message: other.to_string(),
            },
        }
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Exporte l'inventaire (sans mot de passe). `None` si l'utilisateur annule.
#[tauri::command]
#[specta::specta]
pub async fn export_inventory(
    app: AppHandle,
    state: State<'_, AppState>,
    format: ExportFormat,
    options: ExportOptions,
) -> CmdResult<Option<SavedFile>> {
    let (name, ext) = match format {
        ExportFormat::Json => ("vigie-inventaire.json", "json"),
        ExportFormat::Csv => ("vigie-inventaire.csv", "csv"),
    };
    let bytes = state
        .with_conn(|c| match format {
            ExportFormat::Json => portability::export_json(c, options),
            ExportFormat::Csv => portability::export_csv(c, options),
        })
        .ok_or_else(CommandError::locked)??;
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(name)
        .add_filter(ext.to_uppercase(), &[ext])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    write_private(&path, &bytes).map_err(io_err)?;
    Ok(Some(SavedFile {
        path: path.display().to_string(),
        size_bytes: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
    }))
}

/// Crée une sauvegarde chiffrée (même mot de passe maître / clé de récupération que le coffre).
#[tauri::command]
#[specta::specta]
pub async fn backup_vault(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<SavedFile>> {
    if !state.is_unlocked() {
        return Err(CommandError::locked());
    }
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name("vigie.vigiebak")
        .add_filter("Sauvegarde Vigie", &["vigiebak"])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    // Le verrou de l'état est tenu pendant la copie : aucune écriture concurrente.
    let size = state
        .with_conn(|_| portability::create_backup(&state.dir, &path))
        .ok_or_else(CommandError::locked)??;
    Ok(Some(SavedFile {
        path: path.display().to_string(),
        size_bytes: u32::try_from(size).unwrap_or(u32::MAX),
    }))
}

/// Restaure une sauvegarde choisie par l'utilisateur. `password` est celui de la sauvegarde.
/// Le coffre en cours est verrouillé puis remplacé (l'ancien est conservé à côté).
#[tauri::command]
#[specta::specta]
pub async fn restore_vault_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    password: String,
) -> CmdResult<bool> {
    let pw = SecretString::from(password);
    let Some(path) = app
        .dialog()
        .file()
        .add_filter("Sauvegarde Vigie", &["vigiebak"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(false);
    };
    state.lock();
    portability::restore_backup(&state.dir, &path, &pw).map_err(|e| match e {
        BackupError::Vault(VaultError::BadCredentials) => CommandError {
            kind: ErrorKind::BadCredentials,
            message: "mot de passe incorrect pour cette sauvegarde".into(),
        },
        other => other.into(),
    })?;
    Ok(true)
}
