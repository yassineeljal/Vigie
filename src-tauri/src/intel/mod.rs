//! Renseignement externe (US-25, US-30 à US-35) : Pwned Passwords, catalogue HIBP, courriel.
//! Tous les appels réseau passent par `IntelClient` : HTTPS uniquement, hôtes en
//! liste blanche, aucune redirection, backoff exponentiel et gestion des 429 (NFR-06).

mod breaches;
mod client;
mod email;
mod pwned;
mod secrets;
mod tfa;

#[cfg(test)]
pub use breaches::RawBreach;
pub use breaches::{
    BreachMatchView, MatchStats, acknowledge, breach_matches, store_catalogue, update_matches,
};
pub use client::IntelClient;
pub use email::{check_email, store_email_matches};
#[cfg(test)]
pub use pwned::pwned_count_cached;
pub use pwned::{count_in_range, pwned_count, pwned_ranges, sha1_split};
#[cfg(test)]
pub use secrets::MemoryStore;
pub use secrets::{KeychainStore, SecretStore};
#[cfg(test)]
pub use tfa::parse_tfa;
pub use tfa::store_tfa;

#[derive(Debug, thiserror::Error)]
pub enum IntelError {
    #[error("hors ligne ou service injoignable")]
    Offline,
    #[error("limite de débit atteinte, réessaie plus tard")]
    RateLimited,
    #[error("clé API refusée par le service")]
    Unauthorized,
    #[error("réponse inattendue du service (HTTP {0})")]
    Http(u16),
    #[error("réponse illisible : {0}")]
    BadResponse(String),
    #[error("destination non autorisée")]
    Forbidden,
    #[error("mode hors ligne activé")]
    OfflineMode,
    #[error("aucune clé API HIBP configurée")]
    NoApiKey,
    #[error("trousseau : {0}")]
    Keychain(String),
    #[error(transparent)]
    Db(#[from] rusqlite::Error),
}

#[cfg(test)]
mod tests;
