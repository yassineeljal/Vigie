//! État partagé de l'application : coffre déverrouillé (ou non) et minuteur d'inactivité.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::crypto::KdfParams;
use std::sync::Arc;

use crate::import::ImportSession;
use crate::intel::{IntelClient, KeychainStore, SecretStore};
use crate::vault::Vault;

pub const DEFAULT_AUTO_LOCK_MIN: u32 = 5;
const SETTING_AUTO_LOCK: &str = "auto_lock_minutes";

pub struct AppState {
    pub dir: PathBuf,
    pub kdf: KdfParams,
    pub intel: Arc<IntelClient>,
    pub secrets: Arc<dyn SecretStore>,
    inner: Mutex<Inner>,
}

struct Inner {
    vault: Option<Vault>,
    /// Import en cours : mots de passe en mémoire, effacés au verrouillage.
    import: Option<ImportSession>,
    picked_path: Option<PathBuf>,
    last_activity: Instant,
    auto_lock: Duration,
}

impl AppState {
    pub fn new(dir: PathBuf, kdf: KdfParams) -> Self {
        Self {
            dir,
            kdf,
            intel: Arc::new(IntelClient::production()),
            secrets: Arc::new(KeychainStore),
            inner: Mutex::new(Inner {
                vault: None,
                import: None,
                picked_path: None,
                last_activity: Instant::now(),
                auto_lock: minutes(DEFAULT_AUTO_LOCK_MIN),
            }),
        }
    }

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        // Un panic ailleurs ne doit pas empêcher de verrouiller le coffre.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_unlocked(&self) -> bool {
        self.lock_inner().vault.is_some()
    }

    /// Installe le coffre déverrouillé et charge les réglages persistés.
    pub fn set_vault(&self, vault: Vault) {
        let mut g = self.lock_inner();
        if let Some(min) = read_auto_lock(&vault) {
            g.auto_lock = minutes(min);
        }
        g.vault = Some(vault);
        g.last_activity = Instant::now();
    }

    /// Verrouille ; retourne `true` si un coffre était ouvert.
    pub fn lock(&self) -> bool {
        let mut g = self.lock_inner();
        g.import = None;
        g.picked_path = None;
        g.vault.take().is_some()
    }

    /// Exécute `f` avec la connexion du coffre ; `None` si le coffre est verrouillé.
    /// Compte comme une activité de l'utilisateur.
    pub fn with_conn<T>(&self, f: impl FnOnce(&rusqlite::Connection) -> T) -> Option<T> {
        let mut g = self.lock_inner();
        g.last_activity = Instant::now();
        g.vault.as_ref().map(|v| f(v.conn()))
    }

    pub fn set_picked_path(&self, p: Option<PathBuf>) {
        let mut g = self.lock_inner();
        g.picked_path = p;
        g.import = None;
    }

    pub fn picked_path(&self) -> Option<PathBuf> {
        self.lock_inner().picked_path.clone()
    }

    /// Installe la session d'import et retourne son aperçu.
    pub fn start_import<T>(
        &self,
        session: ImportSession,
        preview: impl FnOnce(&rusqlite::Connection, &ImportSession) -> T,
    ) -> Option<T> {
        let mut g = self.lock_inner();
        g.last_activity = Instant::now();
        let out = preview(g.vault.as_ref()?.conn(), &session);
        g.import = Some(session);
        Some(out)
    }

    /// Exécute `f` avec la connexion et la session d'import ; `None` si verrouillé ou sans session.
    /// La session est consommée (effacée) que `f` réussisse ou non.
    pub fn finish_import<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection, &ImportSession) -> T,
    ) -> Option<T> {
        let mut g = self.lock_inner();
        g.last_activity = Instant::now();
        let session = g.import.take()?;
        let out = f(g.vault.as_ref()?.conn(), &session);
        g.picked_path = None;
        Some(out)
    }

    /// Hash des mots de passe de la session d'import (pour interroger Pwned Passwords).
    pub fn import_hashes(&self) -> Option<Vec<Option<crate::import::PasswordHash>>> {
        self.lock_inner()
            .import
            .as_ref()
            .map(|s| s.password_hashes())
    }

    pub fn import_set_counts(&self, counts: &[Option<u32>]) -> bool {
        match self.lock_inner().import.as_mut() {
            Some(s) => {
                s.set_breach_counts(counts);
                true
            }
            None => false,
        }
    }

    /// Reconstruit l'aperçu de la session d'import en cours.
    pub fn import_preview(
        &self,
    ) -> Option<Result<crate::import::ImportPreview, crate::import::ImportError>> {
        let g = self.lock_inner();
        Some(g.import.as_ref()?.preview(g.vault.as_ref()?.conn()))
    }

    pub fn cancel_import(&self) {
        let mut g = self.lock_inner();
        g.import = None;
        g.picked_path = None;
    }

    pub fn touch(&self) {
        self.lock_inner().last_activity = Instant::now();
    }

    /// Verrouille si l'inactivité dépasse le délai ; `true` si le coffre vient d'être verrouillé.
    pub fn lock_if_idle(&self) -> bool {
        let mut g = self.lock_inner();
        if g.vault.is_some() && !g.auto_lock.is_zero() && g.last_activity.elapsed() >= g.auto_lock {
            g.vault = None;
            g.import = None;
            g.picked_path = None;
            return true;
        }
        false
    }

    pub fn auto_lock_minutes(&self) -> u32 {
        (self.lock_inner().auto_lock.as_secs() / 60) as u32
    }

    /// Règle le délai (0 = jamais) et le persiste dans le coffre.
    pub fn set_auto_lock_minutes(&self, min: u32) -> Result<(), crate::vault::VaultError> {
        let mut g = self.lock_inner();
        let Some(vault) = g.vault.as_ref() else {
            return Err(crate::vault::VaultError::NotFound);
        };
        vault.conn().execute(
            "INSERT INTO setting(key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (SETTING_AUTO_LOCK, min.to_string()),
        )?;
        g.auto_lock = minutes(min);
        g.last_activity = Instant::now();
        Ok(())
    }
}

fn minutes(m: u32) -> Duration {
    Duration::from_secs(u64::from(m) * 60)
}

fn read_auto_lock(vault: &Vault) -> Option<u32> {
    vault
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE key = ?1",
            [SETTING_AUTO_LOCK],
            |r| r.get::<_, String>(0),
        )
        .ok()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::Vault;
    use secrecy::SecretString;

    fn fast() -> KdfParams {
        KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
        }
    }

    fn unlocked() -> (tempfile::TempDir, AppState) {
        let d = tempfile::TempDir::new().unwrap();
        let (v, _) = Vault::create(
            d.path(),
            &SecretString::from("correct horse battery staple"),
            fast(),
        )
        .unwrap();
        let s = AppState::new(d.path().to_path_buf(), fast());
        s.set_vault(v);
        (d, s)
    }

    #[test]
    fn verrouillage_apres_inactivite() {
        let (_d, s) = unlocked();
        assert!(!s.lock_if_idle());
        {
            let mut g = s.lock_inner();
            g.last_activity = Instant::now() - Duration::from_secs(6 * 60);
        }
        assert!(s.lock_if_idle());
        assert!(!s.is_unlocked());
        assert!(!s.lock_if_idle());
    }

    #[test]
    fn activite_repousse_le_verrouillage() {
        let (_d, s) = unlocked();
        {
            let mut g = s.lock_inner();
            g.last_activity = Instant::now() - Duration::from_secs(4 * 60);
        }
        s.touch();
        assert!(!s.lock_if_idle());
    }

    #[test]
    fn delai_persiste_et_zero_desactive() {
        let (d, s) = unlocked();
        s.set_auto_lock_minutes(15).unwrap();
        s.lock();
        let v = Vault::unlock(
            d.path(),
            &SecretString::from("correct horse battery staple"),
        )
        .unwrap();
        s.set_vault(v);
        assert_eq!(s.auto_lock_minutes(), 15);

        s.set_auto_lock_minutes(0).unwrap();
        {
            let mut g = s.lock_inner();
            g.last_activity = Instant::now() - Duration::from_secs(10_000);
        }
        assert!(!s.lock_if_idle());
    }
}
