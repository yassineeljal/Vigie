//! Commandes IPC du renseignement (US-25, US-30 à US-33, US-35).
//! Les appels réseau sont faits ici, côté Rust, sans jamais tenir le verrou du coffre.

use std::collections::HashMap;
use std::time::Duration;

use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use super::state::AppState;
use super::vault::{CmdResult, CommandError, ErrorKind};
use crate::domain::{self, IdentityKind};
use crate::import::ImportPreview;
use crate::intel::{self, BreachMatchView, IntelError, MatchStats};
use crate::vault::password_score;

const SETTING_OFFLINE: &str = "offline_mode";
const SETTING_SYNCED: &str = "breaches_synced_at";
const SETTING_HAS_KEY: &str = "hibp_key_configured";
const KEY_NAME: &str = "hibp-api-key";
const SYNC_EVERY_HOURS: u64 = 24;
/// Pause entre deux vérifications de courriel (limite de débit de l'API HIBP).
const EMAIL_PACING: Duration = Duration::from_millis(1700);

#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct IntelUpdated {
    pub newly_relevant: u32,
}

fn db<T>(
    state: &AppState,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, IntelError>,
) -> CmdResult<T> {
    state
        .with_conn(f)
        .ok_or_else(CommandError::locked)?
        .map_err(Into::into)
}

fn setting(state: &AppState, key: &str) -> CmdResult<Option<String>> {
    state
        .with_conn(|c| domain::get_setting(c, key))
        .ok_or_else(CommandError::locked)?
        .map_err(Into::into)
}

fn require_online(state: &AppState) -> CmdResult<()> {
    if setting(state, SETTING_OFFLINE)?.as_deref() == Some("1") {
        return Err(IntelError::OfflineMode.into());
    }
    Ok(())
}

// ---------- État ----------

#[derive(Debug, Serialize, Type)]
pub struct IntelStatus {
    pub offline_mode: bool,
    pub breaches_synced_at: Option<String>,
    pub breach_count: u32,
    pub has_api_key: bool,
}

#[tauri::command]
#[specta::specta]
pub fn intel_status(state: State<'_, AppState>) -> CmdResult<IntelStatus> {
    let out = state
        .with_conn(|c| -> Result<IntelStatus, domain::DomainError> {
            Ok(IntelStatus {
                offline_mode: domain::get_setting(c, SETTING_OFFLINE)?.as_deref() == Some("1"),
                breaches_synced_at: domain::get_setting(c, SETTING_SYNCED)?,
                breach_count: c.query_row("SELECT count(*) FROM breach", [], |r| r.get(0))?,
                has_api_key: domain::get_setting(c, SETTING_HAS_KEY)?.as_deref() == Some("1"),
            })
        })
        .ok_or_else(CommandError::locked)??;
    Ok(out)
}

#[tauri::command]
#[specta::specta]
pub fn set_offline_mode(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    state
        .with_conn(|c| domain::set_setting(c, SETTING_OFFLINE, if enabled { "1" } else { "0" }))
        .ok_or_else(CommandError::locked)??;
    Ok(())
}

// ---------- Mot de passe (US-30) ----------

#[derive(Debug, Serialize, Type)]
pub struct PasswordCheckResult {
    pub breach_count: u32,
    pub strength: u8,
}

/// Vérifie un mot de passe : seuls les 5 premiers caractères de son SHA-1 sont envoyés.
/// Si `account_id` est fourni, le résultat (jamais le mot de passe) est enregistré.
#[tauri::command]
#[specta::specta]
pub async fn check_password(
    state: State<'_, AppState>,
    password: String,
    account_id: Option<u32>,
) -> CmdResult<PasswordCheckResult> {
    require_online(&state)?;
    let pw = SecretString::from(password);
    let strength = password_score(&pw);
    let breach_count = intel::pwned_count(&state.intel, &pw).await?;
    drop(pw); // effacé de la mémoire (SecretString)
    if let Some(id) = account_id {
        state
            .with_conn(|c| {
                c.execute(
                    "INSERT INTO password_check(account_id, checked_at, breach_count, strength)
                     VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?2, ?3)",
                    (id, breach_count, strength),
                )
            })
            .ok_or_else(CommandError::locked)?
            .map_err(domain::DomainError::from)?;
    }
    Ok(PasswordCheckResult {
        breach_count,
        strength,
    })
}

// ---------- Catalogue (US-31, US-32, US-35) ----------

#[derive(Debug, Serialize, Type)]
pub struct SyncReport {
    pub breaches: u32,
    pub stats: MatchStats,
}

pub async fn sync_catalogue(state: &AppState) -> CmdResult<SyncReport> {
    require_online(state)?;
    let list = state.intel.fetch_catalogue().await?;
    let (breaches, stats) = db(state, |c| {
        let n = intel::store_catalogue(c, &list)?;
        Ok((n as u32, intel::update_matches(c)?))
    })?;
    Ok(SyncReport { breaches, stats })
}

#[tauri::command]
#[specta::specta]
pub async fn sync_breaches(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SyncReport> {
    let report = sync_catalogue(&state).await?;
    let _ = IntelUpdated {
        newly_relevant: report.stats.newly_relevant,
    }
    .emit(&app);
    Ok(report)
}

#[tauri::command]
#[specta::specta]
pub fn list_breach_matches(state: State<'_, AppState>) -> CmdResult<Vec<BreachMatchView>> {
    db(&state, intel::breach_matches)
}

#[tauri::command]
#[specta::specta]
pub fn acknowledge_breach_match(
    state: State<'_, AppState>,
    breach_name: String,
    account_id: u32,
    kind: String,
) -> CmdResult<()> {
    db(&state, |c| {
        intel::acknowledge(c, &breach_name, account_id, &kind)
    })
}

/// Tâches périodiques (toutes les 15 min, coffre ouvert) : synchronisation quotidienne du
/// catalogue et de 2fa.directory, point du jour de l'historique, notifications.
pub fn spawn_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(15 * 60)).await;
            let state = app.state::<AppState>();
            if !state.is_unlocked() {
                continue;
            }
            if due_for_sync(&state).unwrap_or(false) {
                // Échec silencieux (hors ligne, limite de débit) : le cache reste utilisable.
                let _ = super::score::sync_mfa(&state).await;
                if let Ok(report) = sync_catalogue(&state).await {
                    let _ = IntelUpdated {
                        newly_relevant: report.stats.newly_relevant,
                    }
                    .emit(&app);
                    if report.stats.newly_relevant > 0 {
                        let lang = state.with_conn(crate::scheduler::language).unwrap_or("fr");
                        notify(
                            &app,
                            &state,
                            crate::scheduler::breach_message(lang, report.stats.newly_relevant),
                        );
                    }
                }
            }
            let rules = crate::scoring::Rules::v1();
            let reminder = state.with_conn(|c| -> Option<(u32, f64)> {
                let report = crate::actions::refresh(c, &rules).ok()?;
                crate::insights::record_today(c, &report).ok()?;
                if !crate::scheduler::notifications_enabled(c)
                    || !crate::scheduler::weekly_reminder_due(c).ok()?
                {
                    return None;
                }
                let (pending, gain) = crate::insights::pending_summary(c, &rules).ok()?;
                // Rien à signaler : on ne marque pas le rappel comme envoyé.
                (pending > 0 && crate::scheduler::mark_reminder_sent(c).is_ok())
                    .then_some((pending, gain))
            });
            if let Some(Some((pending, gain))) = reminder {
                let lang = state.with_conn(crate::scheduler::language).unwrap_or("fr");
                notify(
                    &app,
                    &state,
                    crate::scheduler::reminder_message(lang, pending, gain),
                );
            }
        }
    });
}

fn notify(app: &AppHandle, state: &AppState, (title, body): (String, String)) {
    use tauri_plugin_notification::NotificationExt;
    if state.with_conn(crate::scheduler::notifications_enabled) != Some(true) {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

fn due_for_sync(state: &AppState) -> Option<bool> {
    state
        .with_conn(|c| -> Result<bool, domain::DomainError> {
            let last: Option<i64> = c
                .query_row(
                    "SELECT strftime('%s', value) FROM setting WHERE key = ?1",
                    [SETTING_SYNCED],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok());
            let now: i64 = c.query_row("SELECT strftime('%s','now')", [], |r| {
                r.get::<_, String>(0).map(|s| s.parse().unwrap_or(0))
            })?;
            Ok(last.is_none_or(|l| now - l >= (SYNC_EVERY_HOURS * 3600) as i64))
        })
        .and_then(Result::ok)
}

// ---------- Courriel (US-33) ----------

#[tauri::command]
#[specta::specta]
pub fn set_hibp_api_key(state: State<'_, AppState>, api_key: String) -> CmdResult<()> {
    let key = SecretString::from(api_key.trim().to_owned());
    state.secrets.set(KEY_NAME, &key)?;
    state
        .with_conn(|c| domain::set_setting(c, SETTING_HAS_KEY, "1"))
        .ok_or_else(CommandError::locked)??;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn clear_hibp_api_key(state: State<'_, AppState>) -> CmdResult<()> {
    state.secrets.delete(KEY_NAME)?;
    state
        .with_conn(|c| domain::set_setting(c, SETTING_HAS_KEY, "0"))
        .ok_or_else(CommandError::locked)??;
    Ok(())
}

#[derive(Debug, Serialize, Type)]
pub struct EmailCheckReport {
    pub checked: u32,
    pub breached: u32,
    pub matches: u32,
}

/// Envoie chaque courriel de l'inventaire à HIBP (en clair, avec TA clé API). À lancer
/// seulement après confirmation explicite de l'utilisateur.
#[tauri::command]
#[specta::specta]
pub async fn check_identity_emails(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<EmailCheckReport> {
    require_online(&state)?;
    let key = state.secrets.get(KEY_NAME)?.ok_or(IntelError::NoApiKey)?;
    let emails: Vec<(u32, String)> = state
        .with_conn(domain::list_identities)
        .ok_or_else(CommandError::locked)??
        .into_iter()
        .filter(|i| i.kind == IdentityKind::Email)
        .map(|i| (i.id, i.value))
        .collect();
    // Les correspondances « courriel » s'appuient sur le catalogue en cache.
    if setting(&state, SETTING_SYNCED)?.is_none() {
        sync_catalogue(&state).await?;
    }
    let mut report = EmailCheckReport {
        checked: 0,
        breached: 0,
        matches: 0,
    };
    for (n, (id, email)) in emails.iter().enumerate() {
        if n > 0 {
            tokio::time::sleep(EMAIL_PACING).await;
        }
        let names = intel::check_email(&state.intel, &key, email).await?;
        report.checked += 1;
        if !names.is_empty() {
            report.breached += 1;
        }
        report.matches += db(&state, |c| intel::store_email_matches(c, *id, &names))?;
    }
    let _ = IntelUpdated { newly_relevant: 0 }.emit(&app);
    Ok(report)
}

// ---------- Vérification à l'import (US-25) ----------

#[derive(Debug, Serialize, Type)]
pub struct LeakCheckSummary {
    pub checked: u32,
    pub leaked: u32,
}

/// Vérifie les mots de passe de la session d'import auprès de Pwned Passwords (k-anonymat).
/// Rien n'est stocké : le décompte reste en mémoire jusqu'à la validation de l'import.
#[tauri::command]
#[specta::specta]
pub async fn import_check_leaks(state: State<'_, AppState>) -> CmdResult<LeakCheckSummary> {
    require_online(&state)?;
    let hashes = state.import_hashes().ok_or_else(|| CommandError {
        kind: ErrorKind::InvalidInput,
        message: "aucun import en cours".into(),
    })?;
    let mut prefixes: Vec<String> = hashes.iter().flatten().map(|(p, _)| p.clone()).collect();
    prefixes.sort();
    prefixes.dedup();
    let ranges: HashMap<_, _> = intel::pwned_ranges(state.intel.clone(), prefixes).await?;
    let counts: Vec<Option<u32>> = hashes
        .iter()
        .map(|h| {
            h.as_ref()
                .map(|(p, s)| intel::count_in_range(&ranges[p], s))
        })
        .collect();
    let summary = LeakCheckSummary {
        checked: counts.iter().flatten().count() as u32,
        leaked: counts.iter().flatten().filter(|c| **c > 0).count() as u32,
    };
    if !state.import_set_counts(&counts) {
        return Err(CommandError {
            kind: ErrorKind::InvalidInput,
            message: "import annulé".into(),
        });
    }
    Ok(summary)
}

/// Aperçu à jour de la session d'import (après la vérification des fuites).
#[tauri::command]
#[specta::specta]
pub fn import_preview(state: State<'_, AppState>) -> CmdResult<ImportPreview> {
    state
        .import_preview()
        .ok_or_else(CommandError::locked)?
        .map_err(Into::into)
}
