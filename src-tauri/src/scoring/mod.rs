//! Moteur de score pur et testable (US-40, US-41, NFR-10) : aucune E/S, tout est expliqué.
//! Probabilité L = somme des facteurs (plafonnée à 100) ; priorité = L × w ;
//! score global = 100 − Σ(Lᵢ × wᵢ) / Σwᵢ.

mod facts;
mod rules;

use std::collections::HashMap;

use serde::Serialize;
use specta::Type;

use crate::domain::{AccountStatus, Criticality, MfaKind};
pub use facts::load_facts;
pub use rules::Rules;

/// Tout ce que le moteur sait d'un compte.
#[derive(Debug, Clone)]
pub struct AccountFacts {
    pub id: u32,
    pub status: AccountStatus,
    pub criticality: Criticality,
    pub mfa: Vec<MfaKind>,
    /// Dernier décompte Pwned Passwords connu (`None` = jamais vérifié).
    pub pwned_count: Option<u32>,
    /// Force zxcvbn 0-4 du dernier contrôle.
    pub strength: Option<u8>,
    /// Le mot de passe partage son groupe avec au moins un autre compte actif.
    pub reused: bool,
    /// Brèche pertinente non acquittée (postérieure au dernier changement, ou date inconnue).
    pub breach_after_change: bool,
    /// Comptes qui dépendent de celui-ci pour leur récupération.
    pub dependents: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Factor {
    PwnedPassword,
    BreachAfterChange,
    ReusedPassword,
    NoMfa,
    PivotWithoutStrongMfa,
    WeakPassword,
    WeakMfaOnly,
    UnusedActive,
    NeverChecked,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct FactorHit {
    pub factor: Factor,
    pub points: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct AccountScore {
    pub account_id: u32,
    pub likelihood: u32,
    pub weight: f64,
    /// Criticité effective après héritage (peut dépasser celle saisie).
    pub effective_criticality: Criticality,
    /// L × w
    pub priority: f64,
    pub factors: Vec<FactorHit>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ScoreReport {
    pub rules_version: String,
    pub global: f64,
    pub accounts: Vec<AccountScore>,
}

fn is_strong(k: MfaKind) -> bool {
    matches!(k, MfaKind::Passkey | MfaKind::HardwareKey | MfaKind::Totp)
}

/// Facteurs applicables à un compte et leurs points.
pub fn factors(f: &AccountFacts, rules: &Rules) -> Vec<FactorHit> {
    let p = &rules.factors;
    let mut hits = Vec::new();
    let mut add = |factor, points| hits.push(FactorHit { factor, points });

    if f.pwned_count.is_some_and(|c| c > 0) {
        add(Factor::PwnedPassword, p.pwned_password);
    }
    if f.breach_after_change {
        add(Factor::BreachAfterChange, p.breach_after_change);
    }
    if f.reused {
        add(Factor::ReusedPassword, p.reused_password);
    }
    if f.mfa.is_empty() {
        add(Factor::NoMfa, p.no_mfa);
    }
    if f.dependents.len() as u32 >= rules.pivot.min_dependents
        && !f.mfa.iter().any(|k| is_strong(*k))
    {
        add(Factor::PivotWithoutStrongMfa, p.pivot_without_strong_mfa);
    }
    match f.strength {
        Some(0 | 1) => add(Factor::WeakPassword, p.weak_password_low),
        Some(2) => add(Factor::WeakPassword, p.weak_password_mid),
        _ => {}
    }
    if !f.mfa.is_empty()
        && f.mfa
            .iter()
            .all(|k| matches!(k, MfaKind::Sms | MfaKind::Email))
    {
        add(Factor::WeakMfaOnly, p.weak_mfa_only);
    }
    if f.status == AccountStatus::Unused {
        add(Factor::UnusedActive, p.unused_active);
    }
    if f.pwned_count.is_none() {
        add(Factor::NeverChecked, p.never_checked);
    }
    hits
}

fn rank(c: Criticality) -> u8 {
    match c {
        Criticality::Critical => 0,
        Criticality::High => 1,
        Criticality::Medium => 2,
        Criticality::Low => 3,
    }
}

/// Héritage : un compte pivot prend au moins la criticité la plus haute des comptes qu'il récupère.
pub fn effective_criticalities(accounts: &[AccountFacts]) -> HashMap<u32, Criticality> {
    let mut eff: HashMap<u32, Criticality> =
        accounts.iter().map(|a| (a.id, a.criticality)).collect();
    // Point fixe : la propagation est monotone (on ne fait que monter), donc elle converge.
    for _ in 0..accounts.len().max(1) {
        let mut changed = false;
        for a in accounts {
            for d in &a.dependents {
                let Some(&dep) = eff.get(d) else { continue };
                let cur = eff[&a.id];
                if rank(dep) < rank(cur) {
                    eff.insert(a.id, dep);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    eff
}

pub fn compute(accounts: &[AccountFacts], rules: &Rules) -> ScoreReport {
    let active: Vec<AccountFacts> = accounts
        .iter()
        .filter(|a| a.status != AccountStatus::Deleted)
        .cloned()
        .collect();
    let eff = effective_criticalities(&active);
    let mut scores: Vec<AccountScore> = active
        .iter()
        .map(|a| {
            let hits = factors(a, rules);
            let likelihood = hits.iter().map(|h| h.points).sum::<u32>().min(100);
            let crit = eff[&a.id];
            let weight = rules.weight(crit);
            AccountScore {
                account_id: a.id,
                likelihood,
                weight,
                effective_criticality: crit,
                priority: f64::from(likelihood) * weight,
                factors: hits,
            }
        })
        .collect();
    let total_w: f64 = scores.iter().map(|s| s.weight).sum();
    let global = if total_w == 0.0 {
        100.0
    } else {
        100.0 - scores.iter().map(|s| s.priority).sum::<f64>() / total_w
    };
    scores.sort_by(|a, b| {
        b.priority
            .total_cmp(&a.priority)
            .then(a.account_id.cmp(&b.account_id))
    });
    ScoreReport {
        rules_version: rules.version.clone(),
        global,
        accounts: scores,
    }
}

#[cfg(test)]
mod tests;
