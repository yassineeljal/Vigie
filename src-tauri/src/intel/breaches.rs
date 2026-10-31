//! Catalogue public des brèches HIBP et corrélation par domaine (ADR-004, US-31/32).

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{IntelClient, IntelError};
use crate::domain::normalize_domain;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawBreach {
    pub name: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub domain: String,
    pub breach_date: Option<String>,
    pub added_date: Option<String>,
    #[serde(default)]
    pub data_classes: Vec<String>,
    #[serde(default)]
    pub is_verified: bool,
    #[serde(default)]
    pub is_sensitive: bool,
    #[serde(default)]
    pub is_fabricated: bool,
    #[serde(default)]
    pub is_spam_list: bool,
}

impl IntelClient {
    /// Télécharge le catalogue public (aucune donnée de l'utilisateur n'est envoyée).
    pub async fn fetch_catalogue(&self) -> Result<Vec<RawBreach>, IntelError> {
        let url = format!("{}/breaches", self.endpoints.hibp);
        let body = self.get(&url, &[]).await?.ok_or(IntelError::Http(404))?;
        serde_json::from_slice(&body).map_err(|e| IntelError::BadResponse(e.to_string()))
    }
}

const SETTING_SYNCED_AT: &str = "breaches_synced_at";

/// Remplace le catalogue en cache. Les brèches fabriquées ou de type « spam list » sont ignorées.
pub fn store_catalogue(conn: &Connection, breaches: &[RawBreach]) -> Result<usize, IntelError> {
    conn.execute_batch("SAVEPOINT store_catalogue")?;
    let res = (|| -> Result<usize, IntelError> {
        let mut names = HashSet::new();
        let mut stmt = conn.prepare(
            "INSERT INTO breach(name, title, domain, breach_date, added_date, data_classes, verified, sensitive)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(name) DO UPDATE SET title=excluded.title, domain=excluded.domain,
               breach_date=excluded.breach_date, added_date=excluded.added_date,
               data_classes=excluded.data_classes, verified=excluded.verified, sensitive=excluded.sensitive",
        )?;
        for b in breaches
            .iter()
            .filter(|b| !b.is_fabricated && !b.is_spam_list && !b.name.is_empty())
        {
            let domain = normalize_domain(&b.domain).ok();
            let classes = serde_json::to_string(&b.data_classes).unwrap_or_else(|_| "[]".into());
            stmt.execute(params![
                b.name,
                b.title,
                domain,
                b.breach_date,
                b.added_date,
                classes,
                b.is_verified,
                b.is_sensitive
            ])?;
            names.insert(b.name.clone());
        }
        drop(stmt);
        // Retire les brèches qui ont disparu du catalogue.
        let existing: Vec<String> = conn
            .prepare("SELECT name FROM breach")?
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        for n in existing.iter().filter(|n| !names.contains(*n)) {
            conn.execute("DELETE FROM breach WHERE name = ?1", [n])?;
        }
        conn.execute(
            "INSERT INTO setting(key, value) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SETTING_SYNCED_AT],
        )?;
        Ok(names.len())
    })();
    match res {
        Ok(n) => {
            conn.execute_batch("RELEASE store_catalogue")?;
            Ok(n)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO store_catalogue; RELEASE store_catalogue");
            Err(e)
        }
    }
}

/// Une brèche est pertinente si elle expose des mots de passe et qu'elle est postérieure au
/// dernier changement du mot de passe (ou que celui-ci est inconnu) — voir US-32.
pub(super) fn is_relevant(
    breach_date: Option<&str>,
    data_classes: &[String],
    pwd_changed_at: Option<&str>,
) -> bool {
    let has_passwords = data_classes
        .iter()
        .any(|c| c.eq_ignore_ascii_case("passwords"));
    if !has_passwords {
        return false;
    }
    match (breach_date, pwd_changed_at) {
        (Some(b), Some(p)) => b > p,
        _ => true,
    }
}

#[derive(Debug, Default, Serialize, Type)]
pub struct MatchStats {
    pub total: u32,
    pub relevant: u32,
    /// Correspondances pertinentes absentes avant ce calcul (pour les notifications).
    pub newly_relevant: u32,
}

/// Recalcule les correspondances par domaine, en conservant les acquittements.
pub fn update_matches(conn: &Connection) -> Result<MatchStats, IntelError> {
    conn.execute_batch("SAVEPOINT update_matches")?;
    let res = (|| -> Result<MatchStats, IntelError> {
        let mut previous: HashMap<(String, u32), (Option<String>, bool)> = HashMap::new();
        {
            let mut s = conn.prepare(
                "SELECT breach_name, account_id, acknowledged_at, relevant FROM breach_match WHERE kind='domain'",
            )?;
            for r in s.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, bool>(3)?,
                ))
            })? {
                let (n, a, ack, rel) = r?;
                previous.insert((n, a), (ack, rel));
            }
        }
        conn.execute("DELETE FROM breach_match WHERE kind='domain'", [])?;

        let mut stats = MatchStats::default();
        let mut stmt = conn.prepare(
            "SELECT b.name, a.id, b.breach_date, b.data_classes, a.pwd_changed_at
             FROM account a JOIN breach b ON b.domain = a.domain
             WHERE a.status != 'deleted'",
        )?;
        let rows: Vec<_> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        for (name, account, date, classes, changed) in rows {
            let classes: Vec<String> = serde_json::from_str(&classes).unwrap_or_default();
            let relevant = is_relevant(date.as_deref(), &classes, changed.as_deref());
            let prev = previous.get(&(name.clone(), account));
            let ack = prev.and_then(|(a, _)| a.clone());
            conn.execute(
                "INSERT INTO breach_match(breach_name, account_id, kind, relevant, acknowledged_at)
                 VALUES (?1, ?2, 'domain', ?3, ?4)",
                params![name, account, relevant, ack],
            )?;
            stats.total += 1;
            if relevant {
                stats.relevant += 1;
                if !prev.is_some_and(|(_, was)| *was) {
                    stats.newly_relevant += 1;
                }
            }
        }
        Ok(stats)
    })();
    match res {
        Ok(s) => {
            conn.execute_batch("RELEASE update_matches")?;
            Ok(s)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO update_matches; RELEASE update_matches");
            Err(e)
        }
    }
}

#[derive(Debug, Serialize, Type)]
pub struct BreachMatchView {
    pub breach_name: String,
    pub title: String,
    pub breach_date: Option<String>,
    pub data_classes: Vec<String>,
    pub account_id: u32,
    pub account_label: String,
    pub domain: String,
    /// `domain` ou `email`.
    pub kind: String,
    pub relevant: bool,
    pub acknowledged_at: Option<String>,
}

pub fn breach_matches(conn: &Connection) -> Result<Vec<BreachMatchView>, IntelError> {
    let mut stmt = conn.prepare(
        "SELECT m.breach_name, b.title, b.breach_date, b.data_classes, a.id, a.label, a.domain,
                m.kind, m.relevant, m.acknowledged_at
         FROM breach_match m
         JOIN breach b ON b.name = m.breach_name
         JOIN account a ON a.id = m.account_id
         ORDER BY m.relevant DESC, m.acknowledged_at IS NOT NULL, b.breach_date DESC, a.label COLLATE NOCASE",
    )?;
    let out = stmt
        .query_map([], |r| {
            let classes: String = r.get(3)?;
            Ok(BreachMatchView {
                breach_name: r.get(0)?,
                title: r.get(1)?,
                breach_date: r.get(2)?,
                data_classes: serde_json::from_str(&classes).unwrap_or_default(),
                account_id: r.get(4)?,
                account_label: r.get(5)?,
                domain: r.get(6)?,
                kind: r.get(7)?,
                relevant: r.get(8)?,
                acknowledged_at: r.get(9)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(out)
}

pub fn acknowledge(
    conn: &Connection,
    breach: &str,
    account_id: u32,
    kind: &str,
) -> Result<(), IntelError> {
    conn.execute(
        "UPDATE breach_match SET acknowledged_at = strftime('%Y-%m-%dT%H:%M:%SZ','now')
         WHERE breach_name=?1 AND account_id=?2 AND kind=?3",
        params![breach, account_id, kind],
    )?;
    Ok(())
}
