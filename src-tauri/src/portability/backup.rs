//! Sauvegarde chiffrée (US-61). La base est déjà chiffrée (SQLCipher) et sa clé est enveloppée
//! par le mot de passe maître et la clé de récupération : la sauvegarde est donc le couple
//! (en-tête, base) empaqueté dans un seul fichier, sans aucun déchiffrement ni clé en clair.
//!
//! Format : `VIGIEBAK1` | longueur de l'en-tête (u32 BE) | en-tête JSON | base SQLCipher.

use std::fs;
use std::io::Write;
use std::path::Path;

use secrecy::SecretString;

use crate::vault::{DB_FILE, HEADER_FILE, Vault, VaultError};

const MAGIC: &[u8] = b"VIGIEBAK1";
const MAX_HEADER: usize = 16 * 1024;
const MAX_BACKUP: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("fichier de sauvegarde invalide ou corrompu")]
    Invalid,
    #[error("sauvegarde trop volumineuse")]
    TooLarge,
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("erreur d'E/S : {0}")]
    Io(#[from] std::io::Error),
}

/// Écrit la sauvegarde dans `out` (permissions 0600). Le coffre doit être déverrouillé et
/// inactif pendant la copie (l'appelant tient le verrou de l'état).
pub fn create_backup(vault_dir: &Path, out: &Path) -> Result<u64, BackupError> {
    let header = fs::read(vault_dir.join(HEADER_FILE))?;
    let db = fs::read(vault_dir.join(DB_FILE))?;
    if header.len() > MAX_HEADER {
        return Err(BackupError::Invalid);
    }
    let tmp = out.with_extension("vigiebak.tmp");
    let mut f = fs::File::create(&tmp)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    f.write_all(MAGIC)?;
    f.write_all(&(header.len() as u32).to_be_bytes())?;
    f.write_all(&header)?;
    f.write_all(&db)?;
    f.sync_all()?;
    fs::rename(&tmp, out)?;
    Ok(MAGIC.len() as u64 + 4 + header.len() as u64 + db.len() as u64)
}

fn split(bytes: &[u8]) -> Result<(&[u8], &[u8]), BackupError> {
    let rest = bytes.strip_prefix(MAGIC).ok_or(BackupError::Invalid)?;
    let (len, rest) = rest.split_first_chunk::<4>().ok_or(BackupError::Invalid)?;
    let len = u32::from_be_bytes(*len) as usize;
    if len == 0 || len > MAX_HEADER || rest.len() <= len {
        return Err(BackupError::Invalid);
    }
    Ok(rest.split_at(len))
}

/// Restaure une sauvegarde dans `vault_dir`. Elle est d'abord vérifiée dans un dossier
/// temporaire (le mot de passe doit l'ouvrir) ; le coffre actuel n'est remplacé qu'ensuite et
/// conservé sous `*.replaced-<horodatage>`.
pub fn restore_backup(
    vault_dir: &Path,
    backup: &Path,
    password: &SecretString,
) -> Result<(), BackupError> {
    if fs::metadata(backup)?.len() > MAX_BACKUP {
        return Err(BackupError::TooLarge);
    }
    let bytes = fs::read(backup)?;
    let (header, db) = split(&bytes)?;

    let staging = tempfile::Builder::new()
        .prefix("vigie-restore")
        .tempdir_in(parent_or_dot(vault_dir))?;
    fs::write(staging.path().join(HEADER_FILE), header)?;
    fs::write(staging.path().join(DB_FILE), db)?;
    // Vérification : mot de passe correct, base lisible et migrable.
    drop(
        Vault::unlock(staging.path(), password).map_err(|e| match e {
            VaultError::BadHeader => BackupError::Invalid,
            other => BackupError::Vault(other),
        })?,
    );

    fs::create_dir_all(vault_dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    for name in [DB_FILE, HEADER_FILE] {
        let current = vault_dir.join(name);
        if current.exists() {
            fs::rename(&current, vault_dir.join(format!("{name}.replaced-{stamp}")))?;
        }
    }
    fs::copy(staging.path().join(DB_FILE), vault_dir.join(DB_FILE))?;
    // En-tête en dernier : sans lui, le coffre n'existe pas.
    fs::copy(
        staging.path().join(HEADER_FILE),
        vault_dir.join(HEADER_FILE),
    )?;
    Ok(())
}

fn parent_or_dot(p: &Path) -> &Path {
    p.parent()
        .filter(|x| !x.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KdfParams;
    use tempfile::TempDir;

    const PW: &str = "correct horse battery staple";
    fn pw(s: &str) -> SecretString {
        SecretString::from(s)
    }
    fn fast() -> KdfParams {
        KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
        }
    }
    fn vault_with(marker: &str) -> (TempDir, Vault) {
        let d = TempDir::new().unwrap();
        let (v, _) = Vault::create(d.path(), &pw(PW), fast()).unwrap();
        v.conn()
            .execute(
                "INSERT INTO setting(key, value) VALUES ('marker', ?1)",
                [marker],
            )
            .unwrap();
        (d, v)
    }
    fn marker(v: &Vault) -> String {
        v.conn()
            .query_row("SELECT value FROM setting WHERE key='marker'", [], |r| {
                r.get(0)
            })
            .unwrap()
    }

    #[test]
    fn sauvegarde_puis_restauration_sur_une_machine_vierge() {
        let (d, v) = vault_with("donnée-importante");
        drop(v);
        let out = TempDir::new().unwrap();
        let file = out.path().join("copie.vigiebak");
        let size = create_backup(d.path(), &file).unwrap();
        assert_eq!(size, fs::metadata(&file).unwrap().len());

        // Rien de lisible dans la sauvegarde.
        let raw = fs::read(&file).unwrap();
        assert!(raw.starts_with(MAGIC));
        assert!(!raw.windows(17).any(|w| w == "donnée-importante".as_bytes()));
        assert!(!raw.windows(15).any(|w| w == b"SQLite format 3"));

        let fresh = TempDir::new().unwrap();
        restore_backup(fresh.path(), &file, &pw(PW)).unwrap();
        let restored = Vault::unlock(fresh.path(), &pw(PW)).unwrap();
        assert_eq!(marker(&restored), "donnée-importante");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn mauvais_mot_de_passe_ne_touche_pas_au_coffre_actuel() {
        let (src, v) = vault_with("ancien");
        drop(v);
        let out = TempDir::new().unwrap();
        let file = out.path().join("b.vigiebak");
        create_backup(src.path(), &file).unwrap();

        let (cur, v) = vault_with("actuel");
        drop(v);
        assert!(matches!(
            restore_backup(cur.path(), &file, &pw("pas le bon mot de passe du tout")),
            Err(BackupError::Vault(VaultError::BadCredentials))
        ));
        assert_eq!(
            marker(&Vault::unlock(cur.path(), &pw(PW)).unwrap()),
            "actuel"
        );
        assert_eq!(
            fs::read_dir(cur.path()).unwrap().count(),
            2,
            "aucun fichier parasite"
        );
    }

    #[test]
    fn la_restauration_conserve_l_ancien_coffre() {
        let (src, v) = vault_with("sauvegardé");
        drop(v);
        let out = TempDir::new().unwrap();
        let file = out.path().join("b.vigiebak");
        create_backup(src.path(), &file).unwrap();

        let (cur, v) = vault_with("remplacé");
        drop(v);
        restore_backup(cur.path(), &file, &pw(PW)).unwrap();
        assert_eq!(
            marker(&Vault::unlock(cur.path(), &pw(PW)).unwrap()),
            "sauvegardé"
        );
        let kept: Vec<_> = fs::read_dir(cur.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".replaced-"))
            .collect();
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn fichiers_invalides_refuses() {
        let out = TempDir::new().unwrap();
        let target = TempDir::new().unwrap();
        for (name, content) in [
            ("vide", &b""[..]),
            ("magic", b"PASUNEBACKUP"),
            ("tronque", b"VIGIEBAK1\x00\x00\x00\x10abc"),
            ("entete-geant", b"VIGIEBAK1\xff\xff\xff\xffabc"),
            ("entete-nul", b"VIGIEBAK1\x00\x00\x00\x00abcdef"),
        ] {
            let f = out.path().join(name);
            fs::write(&f, content).unwrap();
            assert!(
                matches!(
                    restore_backup(target.path(), &f, &pw(PW)),
                    Err(BackupError::Invalid)
                ),
                "{name}"
            );
        }
        assert!(!Vault::exists(target.path()));
    }

    #[test]
    fn entete_altere_dans_la_sauvegarde() {
        let (src, v) = vault_with("x");
        drop(v);
        let out = TempDir::new().unwrap();
        let file = out.path().join("b.vigiebak");
        create_backup(src.path(), &file).unwrap();
        let mut bytes = fs::read(&file).unwrap();
        bytes[MAGIC.len() + 4 + 5] ^= 0x01; // un octet de l'en-tête JSON
        fs::write(&file, bytes).unwrap();
        let target = TempDir::new().unwrap();
        assert!(restore_backup(target.path(), &file, &pw(PW)).is_err());
        assert!(!Vault::exists(target.path()));
    }
}
