//! Couche IPC : commandes exposées au front, types TypeScript générés par tauri-specta.

use serde::Serialize;
use specta::Type;

#[derive(Debug, Serialize, Type)]
pub struct Pong {
    pub message: String,
    pub version: String,
}

/// Vérifie que le pont IPC fonctionne (T-06).
#[tauri::command]
#[specta::specta]
pub fn ping() -> Pong {
    Pong {
        message: "pong".into(),
        version: env!("CARGO_PKG_VERSION").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_repond_pong() {
        let p = ping();
        assert_eq!(p.message, "pong");
        assert_eq!(p.version, env!("CARGO_PKG_VERSION"));
    }
}
