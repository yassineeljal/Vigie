mod actions;
mod commands;
mod crypto;
mod domain;
mod import;
mod intel;
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
        ])
        .events(collect_events![VaultLocked])
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
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let dir = app.path().app_data_dir()?;
            app.manage(AppState::new(dir, crypto::KdfParams::default()));
            spawn_idle_watcher(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erreur au démarrage de l'application");
}

#[cfg(test)]
mod tests {
    /// Régénère `src/lib/ipc.ts` (versionné) ; la CI échoue si le fichier diffère.
    #[test]
    fn export_types_ipc() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/ipc.ts");
        super::ipc_builder()
            .export(super::Typescript::default(), path)
            .expect("export TS");
    }
}
