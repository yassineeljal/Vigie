//! Modèles du domaine et accès aux données de l'inventaire (US-10 à US-16).

mod models;
mod normalize;
mod repo;

pub use models::*;
pub use normalize::normalize_domain;
pub use repo::*;

#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("domaine invalide : {0}")]
    InvalidDomain(String),
    #[error("données invalides : {0}")]
    InvalidInput(String),
    #[error("élément introuvable")]
    NotFound,
    #[error("erreur de base de données : {0}")]
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for DomainError {
    fn from(e: rusqlite::Error) -> Self {
        match &e {
            rusqlite::Error::QueryReturnedNoRows => DomainError::NotFound,
            rusqlite::Error::SqliteFailure(f, _)
                if f.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                DomainError::InvalidInput("contrainte violée (référence ou doublon)".into())
            }
            _ => DomainError::Db(e),
        }
    }
}

#[cfg(test)]
mod tests;
