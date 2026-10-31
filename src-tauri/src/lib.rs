mod actions;
mod commands;
mod crypto;
mod domain;
mod import;
mod insights;
mod intel;
mod portability;
mod scheduler;
mod scoring;
mod vault;

use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{Builder, collect_commands, collect_events};

use commands::state::AppState;
use commands::vault::VaultLocked;

fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::ping,
            commands::vault::vault_status,
            commands::vault::password_strength,
            commands::vault::create_vault,
            commands::vault::unlock_vault,
            commands::vault::unlock_with_recovery_key,
            commands::vault::lock_vault,
            commands::vault::change_master_password,
            commands::vault::reset_master_password,
            commands::vault::touch_activity,
            commands::vault::set_auto_lock_minutes,
            commands::inventory::list_accounts,
            commands::inventory::get_account,
            commands::inventory::save_account,
            commands::inventory::delete_account,
            commands::inventory::list_tags,
            commands::inventory::list_identities,
            commands::inventory::save_identity,
            commands::inventory::delete_identity,
            commands::inventory::preview_domain,
            commands::import::import_pick_file,
            commands::import::import_inspect,
            commands::import::import_start,
            commands::import::import_commit,
            commands::import::import_cancel,
            commands::intel::intel_status,
            commands::intel::set_offline_mode,
            commands::intel::check_password,
            commands::intel::sync_breaches,
            commands::intel::list_breach_matches,
            commands::intel::acknowledge_breach_match,
            commands::intel::set_hibp_api_key,
            commands::intel::clear_hibp_api_key,
            commands::intel::check_identity_emails,
            commands::intel::import_check_leaks,
            commands::intel::import_preview,
            commands::insights::get_dashboard,
            commands::insights::get_score_history,
            commands::insights::get_recovery_graph,
            commands::insights::get_notifications_enabled,
            commands::insights::set_notifications_enabled,
            commands::insights::set_language,
            commands::portability::export_inventory,
            commands::portability::backup_vault,
            commands::portability::restore_vault_backup,
            commands::score::get_score_report,
            commands::score::list_actions,
            commands::score::complete_action,
            commands::score::set_action_status,
            commands::score::sync_mfa_directory,
            commands::score::open_change_password,
        ])
        .events(collect_events![VaultLocked, commands::intel::IntelUpdated])
}

/// Verrouille le coffre après inactivité (US-02) et prévient l'interface.
fn spawn_idle_watcher(app: tauri::AppHandle) {
    use tauri_specta::Event;
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if app.state::<AppState>().lock_if_idle() {
                let _ = VaultLocked.emit(&app);
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = ipc_builder();

    #[cfg(debug_assertions)]
    builder
        .export(Typescript::default(), "../src/lib/ipc.ts")
        .expect("échec de l'export des types IPC");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let dir = app.path().app_data_dir()?;
            app.manage(AppState::new(dir, crypto::KdfParams::default()));
            spawn_idle_watcher(app.handle().clone());
            commands::intel::spawn_scheduler(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erreur au démarrage de l'application");
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Régénère `src/lib/ipc.ts` (versionné) ; la CI échoue si le fichier diffère.
    #[test]
    fn export_types_ipc() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc.ts");
        super::ipc_builder()
            .export(super::Typescript::default(), path)
            .expect("export TS");
    }

    fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rust_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// M2 : aucun journal ni affichage de débogage dans le code de production, donc aucun
    /// moyen d'écrire un secret dans des logs. Les tests et le calibrage sont exemptés.
    #[test]
    fn aucun_log_ni_debug_dans_le_code_de_production() {
        let mut files = vec![];
        rust_files(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let banned = [
            "println!",
            "eprintln!",
            "dbg!",
            "tracing::",
            "log::",
            "print!(",
        ];
        for f in files {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            if name == "tests.rs" || name == "kdf.rs" {
                continue; // kdf.rs : test de calibrage `#[ignore]`
            }
            let text = std::fs::read_to_string(&f).unwrap();
            // Le code de production est avant le module de tests intégré.
            let prod = text.split("#[cfg(test)]").next().unwrap();
            for b in banned {
                // Occurrence isolée seulement : « dialog:: » ne doit pas déclencher « log:: ».
                let found = prod.match_indices(b).any(|(i, _)| {
                    !prod[..i]
                        .chars()
                        .next_back()
                        .is_some_and(|c| c.is_alphanumeric() || c == '_')
                });
                assert!(!found, "{} contient {b}", f.display());
            }
        }
    }

    /// M2 : les structures qui portent des mots de passe n'implémentent pas `Debug`.
    #[test]
    fn les_types_a_mots_de_passe_n_exposent_pas_debug() {
        let formats = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/import/formats.rs"),
        )
        .unwrap();
        let idx = formats.find("pub struct ParsedEntry").unwrap();
        let before = &formats[idx.saturating_sub(120)..idx];
        assert!(
            !before.contains("Debug"),
            "ParsedEntry ne doit pas dériver Debug"
        );
        let session = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/import/session.rs"),
        )
        .unwrap();
        for ty in ["pub struct ImportSession", "pub struct SessionEntry"] {
            let idx = session.find(ty).unwrap();
            assert!(
                !session[idx.saturating_sub(120)..idx].contains("Debug"),
                "{ty} ne doit pas dériver Debug"
            );
        }
    }
}
