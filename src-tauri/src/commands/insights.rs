//! Commandes IPC du tableau de bord, de l'historique et du graphe (US-50 à US-53).

use tauri::{AppHandle, State};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use super::state::AppState;
use super::vault::{CmdResult, CommandError, ErrorKind};
use crate::domain;
use crate::insights::{self, Dashboard, GraphData, ScorePoint};
use crate::scheduler;
use crate::scoring::Rules;

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

#[tauri::command]
#[specta::specta]
pub fn get_dashboard(state: State<'_, AppState>) -> CmdResult<Dashboard> {
    run(&state, insights::dashboard)
}

#[tauri::command]
#[specta::specta]
pub fn get_score_history(state: State<'_, AppState>, days: u32) -> CmdResult<Vec<ScorePoint>> {
    run(&state, |c, _| insights::history(c, days.min(3650)))
}

#[tauri::command]
#[specta::specta]
pub fn get_recovery_graph(state: State<'_, AppState>) -> CmdResult<GraphData> {
    run(&state, insights::recovery_graph)
}

#[tauri::command]
#[specta::specta]
pub fn get_notifications_enabled(state: State<'_, AppState>) -> CmdResult<bool> {
    state
        .with_conn(scheduler::notifications_enabled)
        .ok_or_else(CommandError::locked)
}

/// Active ou désactive les notifications ; l'activation demande l'autorisation à macOS.
/// Retourne l'état effectif (faux si l'autorisation est refusée).
#[tauri::command]
#[specta::specta]
pub fn set_notifications_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> CmdResult<bool> {
    let mut effective = enabled;
    if enabled {
        let n = app.notification();
        let granted = match n.permission_state() {
            Ok(PermissionState::Granted) => true,
            _ => matches!(n.request_permission(), Ok(PermissionState::Granted)),
        };
        effective = granted;
    }
    state
        .with_conn(|c| {
            domain::set_setting(
                c,
                scheduler::SETTING_NOTIFICATIONS,
                if effective { "1" } else { "0" },
            )
        })
        .ok_or_else(CommandError::locked)??;
    if enabled && !effective {
        return Err(CommandError {
            kind: ErrorKind::Internal,
            message:
                "autorisation refusée : active les notifications de Vigie dans Réglages Système"
                    .into(),
        });
    }
    Ok(effective)
}

/// Mémorise la langue de l'interface pour les textes produits côté Rust (notifications).
#[tauri::command]
#[specta::specta]
pub fn set_language(state: State<'_, AppState>, lang: String) -> CmdResult<()> {
    let value = if lang == "en" { "en" } else { "fr" };
    state
        .with_conn(|c| domain::set_setting(c, scheduler::SETTING_LANGUAGE, value))
        .ok_or_else(CommandError::locked)??;
    Ok(())
}
