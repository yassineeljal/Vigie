//! Tableau de bord, historique du score et graphe de récupération (US-50 à US-52).

mod graph;

use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde::Serialize;
use specta::Type;

use crate::actions::{self, ActionStatus, ActionView, PlanView};
use crate::domain::{Criticality, DomainError};
use crate::intel::{self, BreachMatchView};
use crate::scoring::{self, Factor, Rules, ScoreReport};

pub use graph::{GraphData, recovery_graph};

type Result<T> = std::result::Result<T, DomainError>;

#[derive(Debug, Serialize, Type)]
pub struct CriticalityCount {
    pub criticality: Criticality,
    pub count: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct FactorCount {
    pub factor: Factor,
    pub accounts: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct RiskBucket {
    /// `low` (L < 25), `moderate` (< 50), `high` (< 75), `severe` (≥ 75)
    pub level: String,
    pub count: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct ScorePoint {
    pub taken_at: String,
    pub score: f64,
}

#[derive(Debug, Serialize, Type)]
pub struct Dashboard {
    pub score: f64,
    /// Variation depuis le plus ancien point des 30 derniers jours (`None` sans historique).
    pub delta_30d: Option<f64>,
    pub accounts_total: u32,
    pub by_criticality: Vec<CriticalityCount>,
    pub risk_buckets: Vec<RiskBucket>,
    pub without_mfa: u32,
    pub critical_without_strong_mfa: u32,
    pub top_factors: Vec<FactorCount>,
    pub recent_breaches: Vec<BreachMatchView>,
    pub top_actions: Vec<ActionView>,
    pub pending_actions: u32,
    pub potential_gain: f64,
    pub history: Vec<ScorePoint>,
}

fn bucket(l: u32) -> &'static str {
    match l {
        0..=24 => "low",
        25..=49 => "moderate",
        50..=74 => "high",
        _ => "severe",
    }
}

/// Enregistre (ou met à jour) le point du jour : l'historique garde la dernière valeur de chaque jour.
pub fn record_today(conn: &Connection, report: &ScoreReport) -> Result<()> {
    let details = serde_json::json!({
        "accounts": report.accounts.len(),
        "at_risk": report.accounts.iter().filter(|a| a.likelihood >= 50).count(),
    })
    .to_string();
    let updated = conn.execute(
        "UPDATE score_snapshot SET taken_at = strftime('%Y-%m-%dT%H:%M:%SZ','now'), global_score = ?1,
           rules_version = ?2, details = ?3
         WHERE date(taken_at) = date('now')",
        params![report.global, report.rules_version, details],
    )?;
    if updated == 0 {
        conn.execute(
            "INSERT INTO score_snapshot(taken_at, global_score, rules_version, details)
             VALUES (strftime('%Y-%m-%dT%H:%M:%SZ','now'), ?1, ?2, ?3)",
            params![report.global, report.rules_version, details],
        )?;
    }
    Ok(())
}

pub fn history(conn: &Connection, days: u32) -> Result<Vec<ScorePoint>> {
    let mut stmt = conn.prepare(
        "SELECT taken_at, global_score FROM score_snapshot
         WHERE date(taken_at) >= date('now', ?1) ORDER BY taken_at",
    )?;
    let pts = stmt
        .query_map([format!("-{days} days")], |r| {
            Ok(ScorePoint {
                taken_at: r.get(0)?,
                score: r.get(1)?,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    Ok(pts)
}

pub fn dashboard(conn: &Connection, rules: &Rules) -> Result<Dashboard> {
    let report = actions::refresh(conn, rules)?;
    record_today(conn, &report)?;
    let facts: HashMap<u32, scoring::AccountFacts> = scoring::load_facts(conn)?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();

    let mut by_crit: HashMap<Criticality, u32> = HashMap::new();
    let mut buckets: HashMap<&str, u32> = HashMap::new();
    let mut factor_counts: HashMap<Factor, u32> = HashMap::new();
    let (mut without_mfa, mut critical_weak) = (0, 0);
    for s in &report.accounts {
        *by_crit.entry(s.effective_criticality).or_default() += 1;
        *buckets.entry(bucket(s.likelihood)).or_default() += 1;
        for h in &s.factors {
            *factor_counts.entry(h.factor).or_default() += 1;
        }
        let f = &facts[&s.account_id];
        if f.mfa.is_empty() {
            without_mfa += 1;
        }
        let strong = f.mfa.iter().any(|k| {
            matches!(
                k,
                crate::domain::MfaKind::Passkey
                    | crate::domain::MfaKind::HardwareKey
                    | crate::domain::MfaKind::Totp
            )
        });
        if s.effective_criticality == Criticality::Critical && !strong {
            critical_weak += 1;
        }
    }
    let mut top_factors: Vec<FactorCount> = factor_counts
        .into_iter()
        .map(|(factor, accounts)| FactorCount { factor, accounts })
        .collect();
    top_factors.sort_by(|a, b| {
        b.accounts
            .cmp(&a.accounts)
            .then((a.factor as u8).cmp(&(b.factor as u8)))
    });

    let todo = actions::list(conn, rules, PlanView::Impact, ActionStatus::Todo)?;
    let pending_actions = todo.len() as u32;
    let potential_gain = todo.iter().map(|a| a.gain).sum();
    let hist = history(conn, 90)?;
    let delta_30d = history(conn, 30)?
        .first()
        .map(|p| report.global - p.score)
        .filter(|d| d.abs() > f64::EPSILON || hist.len() > 1);

    let recent_breaches = intel::breach_matches(conn)
        .map_err(|e| match e {
            intel::IntelError::Db(d) => DomainError::from(d),
            _ => DomainError::NotFound,
        })?
        .into_iter()
        .filter(|m| m.relevant && m.acknowledged_at.is_none())
        .take(5)
        .collect();

    Ok(Dashboard {
        score: report.global,
        delta_30d,
        accounts_total: report.accounts.len() as u32,
        by_criticality: [
            Criticality::Critical,
            Criticality::High,
            Criticality::Medium,
            Criticality::Low,
        ]
        .into_iter()
        .map(|c| CriticalityCount {
            criticality: c,
            count: by_crit.get(&c).copied().unwrap_or(0),
        })
        .collect(),
        risk_buckets: ["low", "moderate", "high", "severe"]
            .into_iter()
            .map(|l| RiskBucket {
                level: l.into(),
                count: buckets.get(l).copied().unwrap_or(0),
            })
            .collect(),
        without_mfa,
        critical_without_strong_mfa: critical_weak,
        top_factors,
        recent_breaches,
        top_actions: todo.into_iter().take(5).collect(),
        pending_actions,
        potential_gain,
        history: hist,
    })
}

/// Nombre d'actions en attente (rappel hebdomadaire) — pas de noms de comptes dans les notifications.
pub fn pending_summary(conn: &Connection, rules: &Rules) -> Result<(u32, f64)> {
    let todo = actions::list(conn, rules, PlanView::Impact, ActionStatus::Todo)?;
    Ok((todo.len() as u32, todo.iter().map(|a| a.gain).sum()))
}

#[cfg(test)]
mod tests;
