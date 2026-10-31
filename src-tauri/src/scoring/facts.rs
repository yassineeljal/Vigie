//! Lecture des faits depuis le coffre pour alimenter le moteur.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::AccountFacts;
use crate::domain::{AccountStatus, Criticality, DomainError, MfaKind};

type Result<T> = std::result::Result<T, DomainError>;

pub fn load_facts(conn: &Connection) -> Result<Vec<AccountFacts>> {
    let mut mfa: HashMap<u32, Vec<MfaKind>> = HashMap::new();
    for r in conn
        .prepare("SELECT account_id, kind FROM mfa_method ORDER BY id")?
        .query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))?
    {
        let (id, kind) = r?;
        if let Some(k) = MfaKind::parse(&kind) {
            mfa.entry(id).or_default().push(k);
        }
    }

    // Dernier contrôle de force / groupe, et dernier décompte de fuite connu.
    let mut strength: HashMap<u32, (Option<u8>, Option<u32>)> = HashMap::new();
    for r in conn
        .prepare(
            "SELECT account_id, strength, reuse_group FROM password_check
             WHERE id IN (SELECT max(id) FROM password_check GROUP BY account_id)",
        )?
        .query_map([], |r| {
            Ok((
                r.get::<_, u32>(0)?,
                r.get::<_, Option<u8>>(1)?,
                r.get::<_, Option<u32>>(2)?,
            ))
        })?
    {
        let (id, s, g) = r?;
        strength.insert(id, (s, g));
    }
    // Dernier décompte de fuite : un contrôle plus récent sans décompte (mot de passe changé,
    // pas encore revérifié) remplace l'ancien résultat, qui ne concerne plus le mot de passe actuel.
    let mut pwned: HashMap<u32, u32> = HashMap::new();
    for r in conn
        .prepare(
            "SELECT account_id, breach_count FROM password_check
             WHERE breach_count IS NOT NULL
               AND id IN (SELECT max(id) FROM password_check GROUP BY account_id)",
        )?
        .query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, u32>(1)?)))?
    {
        let (id, c) = r?;
        pwned.insert(id, c);
    }

    let breached: HashSet<u32> = conn
        .prepare("SELECT DISTINCT account_id FROM breach_match WHERE relevant = 1 AND acknowledged_at IS NULL")?
        .query_map([], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;

    struct Row {
        id: u32,
        status: AccountStatus,
        criticality: Criticality,
        login: Option<u32>,
        recovery: Option<u32>,
    }
    let rows: Vec<Row> = conn
        .prepare("SELECT id, status, criticality, identity_id, recovery_identity_id FROM account")?
        .query_map([], |r| {
            let bad = |c| {
                rusqlite::Error::InvalidColumnType(c, "enum".into(), rusqlite::types::Type::Text)
            };
            Ok(Row {
                id: r.get(0)?,
                status: AccountStatus::parse(&r.get::<_, String>(1)?).ok_or_else(|| bad(1))?,
                criticality: Criticality::parse(&r.get::<_, String>(2)?).ok_or_else(|| bad(2))?,
                login: r.get(3)?,
                recovery: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;

    // identité -> compte qui la porte (ex. le courriel de ton Gmail)
    let owner: HashMap<u32, u32> = conn
        .prepare("SELECT id, account_id FROM identity WHERE account_id IS NOT NULL")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;

    // Un compte dépend de celui qui porte son identité de récupération (à défaut, celle de connexion).
    let live: HashSet<u32> = rows
        .iter()
        .filter(|r| r.status != AccountStatus::Deleted)
        .map(|r| r.id)
        .collect();
    let mut dependents: HashMap<u32, Vec<u32>> = HashMap::new();
    for r in rows.iter().filter(|r| live.contains(&r.id)) {
        if let Some(provider) = r.recovery.or(r.login).and_then(|i| owner.get(&i)).copied()
            && provider != r.id
            && live.contains(&provider)
        {
            dependents.entry(provider).or_default().push(r.id);
        }
    }

    // Réutilisation : le groupe doit encore compter au moins 2 comptes actifs.
    let mut group_size: HashMap<u32, u32> = HashMap::new();
    for r in rows.iter().filter(|r| live.contains(&r.id)) {
        if let Some((_, Some(g))) = strength.get(&r.id) {
            *group_size.entry(*g).or_default() += 1;
        }
    }

    Ok(rows
        .into_iter()
        .map(|r| {
            let (s, g) = strength.get(&r.id).copied().unwrap_or((None, None));
            AccountFacts {
                id: r.id,
                status: r.status,
                criticality: r.criticality,
                mfa: mfa.remove(&r.id).unwrap_or_default(),
                pwned_count: pwned.get(&r.id).copied(),
                strength: s,
                reused: g.is_some_and(|g| group_size.get(&g).copied().unwrap_or(0) >= 2),
                breach_after_change: breached.contains(&r.id),
                dependents: dependents.remove(&r.id).unwrap_or_default(),
            }
        })
        .collect())
}
