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
use tauri_specta::{Builder, collect_commands};

fn ipc_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![commands::ping])
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
