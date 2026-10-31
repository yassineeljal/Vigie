//! Import des exports de gestionnaires de mots de passe (US-20 à US-24).
//!
//! Les fichiers sont lus **côté Rust** : les mots de passe ne traversent jamais la WebView.
//! Ils vivent dans une `ImportSession` en mémoire (effacée à la fin, à l'annulation et au
//! verrouillage) ; seuls la force et un identifiant de groupe de réutilisation sont persistés
//! (le coffre ne stocke jamais de mot de passe).

mod commit;
mod formats;
mod session;

pub use commit::{DuplicatePolicy, ImportReport, commit};
pub use formats::{ColumnMapping, ImportFormat, csv_headers, detect, parse};
pub use session::{ImportPreview, ImportSession, PasswordHash};

/// Lit un export en refusant les fichiers trop gros avant de les charger ; le contenu
/// (qui contient des mots de passe) est effacé de la mémoire au drop.
pub fn read_file(path: &std::path::Path) -> Result<zeroize::Zeroizing<Vec<u8>>, ImportError> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(ImportError::Format("ce n'est pas un fichier".into()));
    }
    if meta.len() > formats::MAX_FILE_BYTES as u64 {
        return Err(ImportError::TooLarge);
    }
    Ok(zeroize::Zeroizing::new(std::fs::read(path)?))
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("fichier trop volumineux (20 Mio maximum)")]
    TooLarge,
    #[error("trop de lignes (50 000 maximum)")]
    TooManyRows,
    #[error("export chiffré : réexporte en version non chiffrée")]
    Encrypted,
    #[error("format non reconnu : {0}")]
    Format(String),
    #[error("lecture impossible : {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Domain(#[from] crate::domain::DomainError),
}

#[cfg(test)]
mod tests;
