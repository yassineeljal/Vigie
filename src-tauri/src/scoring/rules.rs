use serde::Deserialize;

use crate::domain::Criticality;

const V1: &str = include_str!("../../scoring/v1.toml");

#[derive(Debug, Clone, Deserialize)]
pub struct Rules {
    pub version: String,
    pub factors: FactorPoints,
    pub criticality: CriticalityWeights,
    pub pivot: Pivot,
    pub effort_minutes: Efforts,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactorPoints {
    pub pwned_password: u32,
    pub breach_after_change: u32,
    pub reused_password: u32,
    pub no_mfa: u32,
    pub pivot_without_strong_mfa: u32,
    pub weak_password_low: u32,
    pub weak_password_mid: u32,
    pub weak_mfa_only: u32,
    pub unused_active: u32,
    pub never_checked: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CriticalityWeights {
    pub critical: f64,
    pub high: f64,
    pub medium: f64,
    pub low: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pivot {
    pub min_dependents: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Efforts {
    pub change_password: u32,
    pub change_password_after_breach: u32,
    pub enable_mfa: u32,
    pub upgrade_mfa: u32,
    pub secure_pivot: u32,
    pub delete_unused: u32,
}

impl Rules {
    pub fn v1() -> Self {
        toml::from_str(V1).expect("scoring/v1.toml invalide")
    }

    pub fn weight(&self, c: Criticality) -> f64 {
        match c {
            Criticality::Critical => self.criticality.critical,
            Criticality::High => self.criticality.high,
            Criticality::Medium => self.criticality.medium,
            Criticality::Low => self.criticality.low,
        }
    }
}
