use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::ImportError;
use super::session::{ImportSession, find_duplicate, group_passwords};
use crate::domain::{
    self, AccountInput, AccountStatus, Criticality, DomainError, MfaKind, MfaMethod,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DuplicatePolicy {
    /// Ignorer les doublons.
    Skip,
    /// Compléter le compte existant (TOTP, date de changement, étiquettes, notes).
    Merge,
    /// Créer un nouveau compte quand même.
    KeepBoth,
}

#[derive(Debug, Default, Serialize, Type)]
pub struct ImportReport {
    pub created: u32,
    pub merged: u32,
    pub skipped_duplicates: u32,
    pub skipped_invalid: u32,
    pub excluded: u32,
    pub reuse_groups: u32,
    pub reused_accounts: u32,
}

/// Écrit l'import dans le coffre, de façon atomique. Seuls la force du mot de passe et
/// l'identifiant du groupe de réutilisation sont persistés — jamais le mot de passe ni un hash.
pub fn commit(
    conn: &Connection,
    session: &ImportSession,
    policy: DuplicatePolicy,
    excluded: &[u32],
) -> Result<ImportReport, ImportError> {
    let included = |i: usize| !excluded.contains(&(i as u32));
    // Les groupes sont recalculés sur les entrées retenues uniquement.
    let groups = group_passwords(session.entries.iter().enumerate().map(|(i, e)| {
        (included(i) && e.domain.is_some())
            .then_some(e.parsed.password.as_ref())
            .flatten()
    }));

    Ok(domain::in_savepoint(conn, || {
        let base: u32 = conn.query_row(
            "SELECT coalesce(max(reuse_group), 0) FROM password_check",
            [],
            |r| r.get(0),
        )?;
        let mut report = ImportReport {
            reuse_groups: groups.iter().flatten().max().copied().unwrap_or(0),
            reused_accounts: groups.iter().flatten().count() as u32,
            ..Default::default()
        };
        for (i, e) in session.entries.iter().enumerate() {
            if !included(i) {
                report.excluded += 1;
                continue;
            }
            let Some(domain) = &e.domain else {
                report.skipped_invalid += 1;
                continue;
            };
            let existing = find_duplicate(conn, domain, &e.parsed.username.to_lowercase())?;
            let account_id = match (existing, policy) {
                (Some(_), DuplicatePolicy::Skip) => {
                    report.skipped_duplicates += 1;
                    continue;
                }
                (Some(id), DuplicatePolicy::Merge) => {
                    let mut input = AccountInput::from(&domain::get_account(conn, id)?);
                    if e.parsed.has_totp && !input.mfa.iter().any(|m| m.kind == MfaKind::Totp) {
                        input.mfa.push(MfaMethod {
                            kind: MfaKind::Totp,
                            backup_codes: false,
                        });
                    }
                    if let Some(d) = &e.parsed.pwd_changed_at
                        && input.pwd_changed_at.as_ref().is_none_or(|old| d > old)
                    {
                        input.pwd_changed_at = Some(d.clone());
                    }
                    input.tags.extend(e.parsed.folder.clone());
                    if input.notes.is_none() {
                        input.notes = e.parsed.notes.clone();
                    }
                    report.merged += 1;
                    domain::save_account(conn, Some(id), &input)?.id
                }
                _ => {
                    let input = AccountInput {
                        domain: domain.clone(),
                        label: if e.parsed.name.is_empty() {
                            domain.clone()
                        } else {
                            e.parsed.name.clone()
                        },
                        identity_id: None,
                        username: (!e.parsed.username.is_empty())
                            .then(|| e.parsed.username.clone()),
                        criticality: Criticality::Medium,
                        recovery_identity_id: None,
                        pwd_changed_at: e.parsed.pwd_changed_at.clone(),
                        status: AccountStatus::Active,
                        notes: e.parsed.notes.clone(),
                        tags: e.parsed.folder.clone().into_iter().collect(),
                        mfa: if e.parsed.has_totp {
                            vec![MfaMethod {
                                kind: MfaKind::Totp,
                                backup_codes: false,
                            }]
                        } else {
                            vec![]
                        },
                    };
                    report.created += 1;
                    domain::save_account(conn, None, &input)?.id
                }
            };
            if e.parsed.password.is_some() {
                conn.execute(
                    "INSERT INTO password_check(account_id, checked_at, breach_count, strength, reuse_group) \
                     VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), NULL, ?2, ?3)",
                    params![account_id, e.strength, groups[i].map(|g| base + g)],
                )?;
            }
        }
        Ok::<_, DomainError>(report)
    })?)
}
