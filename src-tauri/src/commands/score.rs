//! Commandes IPC du score et du plan d'action (US-34, US-40 à US-44).

use serde::Serialize;
use specta::Type;
use tauri::State;

use super::state::AppState;
use super::vault::{CmdResult, CommandError, ErrorKind};
use crate::actions::{self, ActionStatus, ActionView, PlanView};
use crate::domain::{self, MfaKind};
use crate::intel;
use crate::scoring::{Rules, ScoreReport};

fn run<T>(
    state: &AppState,
    f: impl FnOnce(&rusqlite::Connection, &Rules) -> Result<T, domain::DomainError>,
) -> CmdResult<T> {
    let rules = Rules::v1();
    state
        .with_conn(|c| f(c, &rules))
        .ok_or_else(CommandError::locked)?
        .map_err(Into::into)
}

/// Score global et score détaillé (facteurs et points) de chaque compte actif.
#[tauri::command]
#[specta::specta]
pub fn get_score_report(state: State<'_, AppState>) -> CmdResult<ScoreReport> {
    run(&state, actions::refresh)
}

#[tauri::command]
#[specta::specta]
pub fn list_actions(
    state: State<'_, AppState>,
    view: PlanView,
    status: ActionStatus,
) -> CmdResult<Vec<ActionView>> {
    run(&state, |c, r| actions::list(c, r, view, status))
}

/// Marque l'action faite en appliquant son effet ; `mfa_kind` est requis pour les actions MFA.
#[tauri::command]
#[specta::specta]
pub fn complete_action(
    state: State<'_, AppState>,
    id: u32,
    mfa_kind: Option<MfaKind>,
) -> CmdResult<ScoreReport> {
    run(&state, |c, r| actions::complete(c, r, id, mfa_kind))
}

#[tauri::command]
#[specta::specta]
pub fn set_action_status(
    state: State<'_, AppState>,
    id: u32,
    status: ActionStatus,
) -> CmdResult<()> {
    run(&state, |c, _| actions::set_status(c, id, status))
}

#[derive(Debug, Serialize, Type)]
pub struct MfaSyncReport {
    pub services_updated: u32,
}

/// Télécharge le catalogue 2fa.directory et met à jour les sites de l'inventaire.
pub async fn sync_mfa(state: &AppState) -> CmdResult<MfaSyncReport> {
    let offline = state
        .with_conn(|c| domain::get_setting(c, "offline_mode"))
        .ok_or_else(CommandError::locked)??;
    if offline.as_deref() == Some("1") {
        return Err(intel::IntelError::OfflineMode.into());
    }
    let entries = state.intel.fetch_tfa().await?;
    let services_updated = state
        .with_conn(|c| intel::store_tfa(c, &entries))
        .ok_or_else(CommandError::locked)??;
    Ok(MfaSyncReport { services_updated })
}

#[tauri::command]
#[specta::specta]
pub async fn sync_mfa_directory(state: State<'_, AppState>) -> CmdResult<MfaSyncReport> {
    sync_mfa(&state).await
}

/// Ouvre la page de changement de mot de passe dans le navigateur. L'URL est construite côté
/// Rust à partir du domaine stocké : `/.well-known/change-password` (standard W3C) ou, en repli
/// choisi par l'utilisateur, la page d'accueil. Aucun appel réseau de Vigie lui-même.
#[tauri::command]
#[specta::specta]
pub fn open_change_password(
    state: State<'_, AppState>,
    account_id: u32,
    homepage: bool,
) -> CmdResult<()> {
    let account = state
        .with_conn(|c| domain::get_account(c, account_id))
        .ok_or_else(CommandError::locked)??;
    let url = if homepage {
        format!("https://{}/", account.domain)
    } else {
        format!("https://{}/.well-known/change-password", account.domain)
    };
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| CommandError {
        kind: ErrorKind::Internal,
        message: format!("ouverture impossible : {e}"),
    })
}
