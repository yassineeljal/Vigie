//! Commandes IPC de l'inventaire (US-10 à US-16).

use tauri::State;

use super::state::AppState;
use super::vault::{CmdResult, CommandError};
use crate::domain::{
    self, Account, AccountFilter, AccountInput, Identity, IdentityInput, normalize_domain,
};

fn run<T>(
    state: &AppState,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, domain::DomainError>,
) -> CmdResult<T> {
    state
        .with_conn(f)
        .ok_or_else(CommandError::locked)?
        .map_err(Into::into)
}

#[tauri::command]
#[specta::specta]
pub fn list_accounts(state: State<'_, AppState>, filter: AccountFilter) -> CmdResult<Vec<Account>> {
    run(&state, |c| domain::list_accounts(c, &filter))
}

#[tauri::command]
#[specta::specta]
pub fn get_account(state: State<'_, AppState>, id: u32) -> CmdResult<Account> {
    run(&state, |c| domain::get_account(c, id))
}

/// Crée (`id` absent) ou modifie un compte.
#[tauri::command]
#[specta::specta]
pub fn save_account(
    state: State<'_, AppState>,
    id: Option<u32>,
    input: AccountInput,
) -> CmdResult<Account> {
    run(&state, |c| domain::save_account(c, id, &input))
}

#[tauri::command]
#[specta::specta]
pub fn delete_account(state: State<'_, AppState>, id: u32) -> CmdResult<()> {
    run(&state, |c| domain::delete_account(c, id))
}

#[tauri::command]
#[specta::specta]
pub fn list_tags(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    run(&state, domain::list_tags)
}

#[tauri::command]
#[specta::specta]
pub fn list_identities(state: State<'_, AppState>) -> CmdResult<Vec<Identity>> {
    run(&state, domain::list_identities)
}

#[tauri::command]
#[specta::specta]
pub fn save_identity(
    state: State<'_, AppState>,
    id: Option<u32>,
    input: IdentityInput,
) -> CmdResult<Identity> {
    run(&state, |c| domain::save_identity(c, id, &input))
}

#[tauri::command]
#[specta::specta]
pub fn delete_identity(state: State<'_, AppState>, id: u32) -> CmdResult<()> {
    run(&state, |c| domain::delete_identity(c, id))
}

/// Aperçu du domaine normalisé pendant la saisie (US-16).
#[tauri::command]
#[specta::specta]
pub fn preview_domain(input: String) -> Option<String> {
    normalize_domain(&input).ok()
}
