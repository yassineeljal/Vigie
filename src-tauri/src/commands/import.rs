//! Commandes IPC d'import (US-20 à US-24). Le chemin du fichier choisi reste côté Rust :
//! l'interface ne peut pas demander la lecture d'un chemin arbitraire.

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use super::state::AppState;
use super::vault::{CmdResult, CommandError, ErrorKind};
use crate::import::{
    self, ColumnMapping, DuplicatePolicy, ImportFormat, ImportPreview, ImportReport, ImportSession,
};

#[derive(Debug, Serialize, Type)]
pub struct PickedFile {
    pub name: String,
    pub size_bytes: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct FileInspection {
    /// Format reconnu automatiquement ; `None` → mappage de colonnes à fournir.
    pub format: Option<ImportFormat>,
    pub headers: Vec<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct ImportOutcome {
    pub report: ImportReport,
    /// Fichier source, à supprimer par l'utilisateur (M11).
    pub file_path: String,
}

fn no_file() -> CommandError {
    CommandError {
        kind: ErrorKind::InvalidInput,
        message: "aucun fichier sélectionné".into(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn import_pick_file(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<PickedFile>> {
    if !state.is_unlocked() {
        return Err(CommandError::locked());
    }
    let picked = app
        .dialog()
        .file()
        .add_filter("Exports (CSV, JSON)", &["csv", "json"])
        .blocking_pick_file();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let size = std::fs::metadata(&path)
        .map_err(import::ImportError::from)?
        .len();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    state.set_picked_path(Some(path));
    Ok(Some(PickedFile {
        name,
        size_bytes: u32::try_from(size).unwrap_or(u32::MAX),
    }))
}

#[tauri::command]
#[specta::specta]
pub async fn import_inspect(state: State<'_, AppState>) -> CmdResult<FileInspection> {
    let path = state.picked_path().ok_or_else(no_file)?;
    let bytes = import::read_file(&path)?;
    let format = import::detect(&bytes);
    let headers = match format {
        Some(ImportFormat::BitwardenJson) => vec![],
        _ => import::csv_headers(&bytes).unwrap_or_default(),
    };
    Ok(FileInspection { format, headers })
}

#[tauri::command]
#[specta::specta]
pub async fn import_start(
    state: State<'_, AppState>,
    format: ImportFormat,
    mapping: Option<ColumnMapping>,
) -> CmdResult<ImportPreview> {
    let path = state.picked_path().ok_or_else(no_file)?;
    let bytes = import::read_file(&path)?;
    let outcome = import::parse(&bytes, format, mapping.as_ref())?;
    drop(bytes);
    let session = ImportSession::build(outcome);
    state
        .start_import(session, |conn, s| s.preview(conn))
        .ok_or_else(CommandError::locked)?
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn import_commit(
    state: State<'_, AppState>,
    policy: DuplicatePolicy,
    excluded: Vec<u32>,
) -> CmdResult<ImportOutcome> {
    let path = state.picked_path().ok_or_else(no_file)?;
    let report = state
        .finish_import(|conn, s| import::commit(conn, s, policy, &excluded))
        .ok_or_else(CommandError::locked)??;
    Ok(ImportOutcome {
        report,
        file_path: path.display().to_string(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn import_cancel(state: State<'_, AppState>) {
    state.cancel_import();
}
