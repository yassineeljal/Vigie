use std::collections::HashMap;

use rusqlite::Connection;
use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use specta::Type;

use super::ImportError;
use super::formats::{ParseOutcome, ParsedEntry};
use crate::domain::normalize_domain;

/// SHA-1 d'un mot de passe : (préfixe de 5 caractères, suffixe).
pub type PasswordHash = (String, zeroize::Zeroizing<String>);

pub struct SessionEntry {
    pub parsed: ParsedEntry,
    pub domain: Option<String>,
    pub issue: Option<String>,
    pub strength: Option<u8>,
    /// Nombre d'apparitions dans Pwned Passwords ; `None` tant que non vérifié.
    pub breach_count: Option<u32>,
}

/// Données d'import gardées en mémoire entre l'aperçu et la validation.
/// Les mots de passe sont des `SecretString` : effacés au drop.
pub struct ImportSession {
    pub entries: Vec<SessionEntry>,
    pub skipped_rows: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct PreviewItem {
    pub index: u32,
    pub label: String,
    pub domain: Option<String>,
    pub username: Option<String>,
    pub has_totp: bool,
    pub has_password: bool,
    pub strength: Option<u8>,
    pub breach_count: Option<u32>,
    /// Compte existant (même domaine et même identifiant).
    pub duplicate_of: Option<u32>,
    /// Entrée précédente du même fichier (même domaine et même identifiant).
    pub repeats_index: Option<u32>,
    /// Identifiant de groupe de réutilisation (local à cet aperçu).
    pub reuse_group: Option<u32>,
    pub issue: Option<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct ImportPreview {
    pub items: Vec<PreviewItem>,
    pub reuse_groups: u32,
    pub reused_accounts: u32,
    pub skipped_rows: u32,
}

/// Regroupe les entrées qui partagent le même mot de passe (groupes d'au moins 2).
/// Retourne, pour chaque position, `Some(groupe)` numéroté à partir de 1. Calcul purement
/// en mémoire : aucun hash de mot de passe n'est conservé (ADR-005).
pub fn group_passwords<'a>(
    passwords: impl Iterator<Item = Option<&'a SecretString>>,
) -> Vec<Option<u32>> {
    let pw: Vec<Option<&SecretString>> = passwords.collect();
    let mut by_pw: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, p) in pw.iter().enumerate() {
        if let Some(p) = p {
            by_pw.entry(p.expose_secret()).or_default().push(i);
        }
    }
    let mut groups: Vec<Vec<usize>> = by_pw.into_values().filter(|v| v.len() >= 2).collect();
    groups.sort_by_key(|g| g[0]);
    let mut out = vec![None; pw.len()];
    for (n, g) in groups.iter().enumerate() {
        for &i in g {
            out[i] = Some(n as u32 + 1);
        }
    }
    out
}

pub fn find_duplicate(
    conn: &Connection,
    domain: &str,
    username: &str,
) -> rusqlite::Result<Option<u32>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id FROM account WHERE domain = ?1 AND lower(coalesce(username, '')) = lower(?2) \
         ORDER BY id LIMIT 1",
    )?;
    match stmt.query_row((domain, username), |r| r.get(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

impl ImportSession {
    /// Hash SHA-1 (préfixe, suffixe) de chaque mot de passe, pour interroger Pwned Passwords
    /// sans sortir les mots de passe du cœur ni garder la session verrouillée pendant le réseau.
    pub fn password_hashes(&self) -> Vec<Option<PasswordHash>> {
        self.entries
            .iter()
            .map(|e| {
                (e.domain.is_some())
                    .then_some(e.parsed.password.as_ref())
                    .flatten()
                    .map(crate::intel::sha1_split)
            })
            .collect()
    }

    pub fn set_breach_counts(&mut self, counts: &[Option<u32>]) {
        for (e, c) in self.entries.iter_mut().zip(counts) {
            if c.is_some() {
                e.breach_count = *c;
            }
        }
    }

    pub fn build(outcome: ParseOutcome) -> Self {
        let entries = outcome
            .entries
            .into_iter()
            .map(|parsed| {
                let domain = normalize_domain(&parsed.url)
                    .or_else(|_| normalize_domain(&parsed.name))
                    .ok();
                let issue = domain
                    .is_none()
                    .then(|| "URL ou domaine invalide".to_owned());
                let strength = parsed.password.as_ref().map(crate::vault::password_score);
                SessionEntry {
                    parsed,
                    domain,
                    issue,
                    strength,
                    breach_count: None,
                }
            })
            .collect();
        Self {
            entries,
            skipped_rows: outcome.skipped,
        }
    }

    pub fn preview(&self, conn: &Connection) -> Result<ImportPreview, ImportError> {
        let groups = group_passwords(self.entries.iter().map(|e| e.parsed.password.as_ref()));
        let mut first_seen: HashMap<(String, String), u32> = HashMap::new();
        let mut items = Vec::with_capacity(self.entries.len());
        for (i, e) in self.entries.iter().enumerate() {
            let username = e.parsed.username.to_lowercase();
            let (mut duplicate_of, mut repeats_index) = (None, None);
            if let Some(d) = &e.domain {
                duplicate_of =
                    find_duplicate(conn, d, &username).map_err(crate::domain::DomainError::from)?;
                match first_seen.entry((d.clone(), username)) {
                    std::collections::hash_map::Entry::Occupied(o) => {
                        repeats_index = Some(*o.get())
                    }
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert(i as u32);
                    }
                }
            }
            items.push(PreviewItem {
                index: i as u32,
                label: if e.parsed.name.is_empty() {
                    e.domain.clone().unwrap_or_default()
                } else {
                    e.parsed.name.clone()
                },
                domain: e.domain.clone(),
                username: (!e.parsed.username.is_empty()).then(|| e.parsed.username.clone()),
                has_totp: e.parsed.has_totp,
                has_password: e.parsed.password.is_some(),
                strength: e.strength,
                breach_count: e.breach_count,
                duplicate_of,
                repeats_index,
                reuse_group: groups[i],
                issue: e.issue.clone(),
            });
        }
        Ok(ImportPreview {
            reuse_groups: groups.iter().flatten().max().copied().unwrap_or(0),
            reused_accounts: groups.iter().flatten().count() as u32,
            skipped_rows: self.skipped_rows,
            items,
        })
    }
}
