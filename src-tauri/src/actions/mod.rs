//! Génération, priorisation et suivi des actions correctives (US-42, US-43, US-44).
//! Gain d'une action = points des facteurs qu'elle corrige × poids de criticité du compte.

use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::{AccountStatus, DomainError, MfaKind};
use crate::scoring::{self, Factor, Rules, ScoreReport};

type Result<T> = std::result::Result<T, DomainError>;

macro_rules! kinds {
    ($($v:ident => $s:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
        #[serde(rename_all = "snake_case")]
        pub enum ActionKind { $($v),+ }
        impl ActionKind {
            pub const ALL: &'static [ActionKind] = &[$(ActionKind::$v),+];
            pub fn as_str(self) -> &'static str { match self { $(Self::$v => $s),+ } }
            pub fn parse(s: &str) -> Option<Self> { match s { $($s => Some(Self::$v),)+ _ => None } }
        }
    };
}
kinds!(
    ChangePassword => "change_password",
    ChangePasswordAfterBreach => "change_password_after_breach",
    EnableMfa => "enable_mfa",
    UpgradeMfa => "upgrade_mfa",
    SecurePivot => "secure_pivot",
    DeleteUnused => "delete_unused",
);

impl ActionKind {
    /// Facteurs que cette action corrige.
    pub fn fixes(self) -> &'static [Factor] {
        match self {
            Self::ChangePassword => &[
                Factor::PwnedPassword,
                Factor::WeakPassword,
                Factor::ReusedPassword,
            ],
            Self::ChangePasswordAfterBreach => &[Factor::BreachAfterChange],
            Self::EnableMfa => &[Factor::NoMfa],
            Self::UpgradeMfa => &[Factor::WeakMfaOnly],
            Self::SecurePivot => &[Factor::PivotWithoutStrongMfa],
            Self::DeleteUnused => &[Factor::UnusedActive],
        }
    }

    fn effort(self, rules: &Rules) -> u32 {
        let e = &rules.effort_minutes;
        match self {
            Self::ChangePassword => e.change_password,
            Self::ChangePasswordAfterBreach => e.change_password_after_breach,
            Self::EnableMfa => e.enable_mfa,
            Self::UpgradeMfa => e.upgrade_mfa,
            Self::SecurePivot => e.secure_pivot,
            Self::DeleteUnused => e.delete_unused,
        }
    }

    fn needs_mfa_kind(self) -> bool {
        matches!(self, Self::EnableMfa | Self::UpgradeMfa | Self::SecurePivot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    Todo,
    Done,
    Ignored,
}
impl ActionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Done => "done",
            Self::Ignored => "ignored",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        match s {
            "todo" => Some(Self::Todo),
            "done" => Some(Self::Done),
            "ignored" => Some(Self::Ignored),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PlanView {
    /// Tri par gain décroissant.
    Impact,
    /// Tri par gain ÷ effort décroissant.
    QuickWins,
}

#[derive(Debug, Serialize, Type)]
pub struct ActionView {
    pub id: u32,
    pub account_id: u32,
    pub account_label: String,
    pub domain: String,
    pub kind: ActionKind,
    pub gain: f64,
    pub effort_min: u32,
    /// gain ÷ effort (points par minute)
    pub ratio: f64,
    pub status: ActionStatus,
    /// Facteurs corrigés par l'action (vide pour les actions déjà traitées).
    pub factors: Vec<Factor>,
    /// Méthodes MFA que le site déclare supporter (2fa.directory), si connues.
    pub supported_mfa: Vec<String>,
    pub supports_passkeys: bool,
}

struct Candidate {
    account_id: u32,
    kind: ActionKind,
    gain: f64,
    factors: Vec<Factor>,
}

fn candidates(report: &ScoreReport) -> Vec<Candidate> {
    let mut out = Vec::new();
    for s in &report.accounts {
        for &kind in ActionKind::ALL {
            let hits: Vec<_> = s
                .factors
                .iter()
                .filter(|h| kind.fixes().contains(&h.factor))
                .collect();
            if hits.is_empty() {
                continue;
            }
            let points: u32 = hits.iter().map(|h| h.points).sum();
            out.push(Candidate {
                account_id: s.account_id,
                kind,
                gain: f64::from(points) * s.weight,
                factors: hits.iter().map(|h| h.factor).collect(),
            });
        }
    }
    out
}

/// Calcule le score courant et met la table `action` en cohérence avec lui.
pub fn refresh(conn: &Connection, rules: &Rules) -> Result<ScoreReport> {
    let report = scoring::compute(&scoring::load_facts(conn)?, rules);
    crate::domain::in_savepoint(conn, || {
        let mut existing: HashMap<(u32, ActionKind), (u32, ActionStatus)> = HashMap::new();
        for r in conn
            .prepare("SELECT id, account_id, kind, status FROM action")?
            .query_map([], |r| {
                Ok((
                    r.get::<_, u32>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
        {
            let (id, acc, kind, status) = r?;
            if let (Some(k), Some(s)) = (ActionKind::parse(&kind), ActionStatus::parse(&status)) {
                existing.insert((acc, k), (id, s));
            }
        }
        let mut live = std::collections::HashSet::new();
        for c in candidates(&report) {
            live.insert((c.account_id, c.kind));
            let effort = c.kind.effort(rules);
            match existing.get(&(c.account_id, c.kind)) {
                None => {
                    conn.execute(
                        "INSERT INTO action(account_id, kind, gain, effort_min, status, rules_version, created_at)
                         VALUES (?1, ?2, ?3, ?4, 'todo', ?5, strftime('%Y-%m-%dT%H:%M:%SZ','now'))",
                        params![c.account_id, c.kind.as_str(), c.gain, effort, report.rules_version],
                    )?;
                }
                Some((id, status)) => {
                    // Une action « faite » dont le problème réapparaît (nouvelle brèche…) est rouverte.
                    let status = if *status == ActionStatus::Done {
                        ActionStatus::Todo
                    } else {
                        *status
                    };
                    conn.execute(
                        "UPDATE action SET gain=?1, effort_min=?2, status=?3, rules_version=?4,
                           done_at = CASE WHEN ?3 = 'done' THEN done_at ELSE NULL END WHERE id=?5",
                        params![c.gain, effort, status.as_str(), report.rules_version, id],
                    )?;
                }
            }
        }
        // Les actions « à faire » devenues sans objet disparaissent ; le reste est conservé.
        for ((acc, kind), (id, status)) in &existing {
            if !live.contains(&(*acc, *kind)) && *status == ActionStatus::Todo {
                conn.execute("DELETE FROM action WHERE id=?1", [id])?;
            }
        }
        Ok(())
    })?;
    Ok(report)
}

pub fn list(
    conn: &Connection,
    rules: &Rules,
    view: PlanView,
    status: ActionStatus,
) -> Result<Vec<ActionView>> {
    let report = refresh(conn, rules)?;
    let factors: HashMap<(u32, ActionKind), Vec<Factor>> = candidates(&report)
        .into_iter()
        .map(|c| ((c.account_id, c.kind), c.factors))
        .collect();
    let mut stmt = conn.prepare(
        "SELECT x.id, x.account_id, a.label, a.domain, x.kind, x.gain, x.effort_min, x.status,
                s.supported_mfa, s.passkeys
         FROM action x JOIN account a ON a.id = x.account_id
         LEFT JOIN service s ON s.domain = a.domain
         WHERE x.status = ?1 AND a.status != 'deleted'",
    )?;
    let mut out: Vec<ActionView> = stmt
        .query_map([status.as_str()], |r| {
            let kind =
                ActionKind::parse(&r.get::<_, String>(4)?).ok_or(rusqlite::Error::InvalidQuery)?;
            let account_id: u32 = r.get(1)?;
            let gain: f64 = r.get(5)?;
            let effort: u32 = r.get(6)?;
            let supported: Option<String> = r.get(8)?;
            Ok(ActionView {
                id: r.get(0)?,
                account_id,
                account_label: r.get(2)?,
                domain: r.get(3)?,
                kind,
                gain,
                effort_min: effort,
                ratio: gain / f64::from(effort.max(1)),
                status,
                factors: factors
                    .get(&(account_id, kind))
                    .cloned()
                    .unwrap_or_default(),
                supported_mfa: supported
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default(),
                supports_passkeys: r.get::<_, Option<bool>>(9)?.unwrap_or(false),
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    out.sort_by(|a, b| {
        let key = |v: &ActionView| {
            if view == PlanView::Impact {
                v.gain
            } else {
                v.ratio
            }
        };
        key(b)
            .total_cmp(&key(a))
            .then(b.gain.total_cmp(&a.gain))
            .then(a.id.cmp(&b.id))
    });
    Ok(out)
}

fn is_strong(k: MfaKind) -> bool {
    matches!(k, MfaKind::Passkey | MfaKind::HardwareKey | MfaKind::Totp)
}

/// Marque une action comme faite en appliquant son effet à l'inventaire, puis recalcule.
/// - changement de mot de passe : date de changement = aujourd'hui, ancien contrôle remplacé
///   (le nouveau mot de passe n'est pas connu : « jamais vérifié ») ;
/// - MFA : la méthode choisie est ajoutée, avec codes de secours ;
/// - suppression : le compte passe à « supprimé ».
pub fn complete(
    conn: &Connection,
    rules: &Rules,
    id: u32,
    mfa_kind: Option<MfaKind>,
) -> Result<ScoreReport> {
    crate::domain::in_savepoint(conn, || {
        let (account_id, kind): (u32, String) = conn.query_row(
            "SELECT account_id, kind FROM action WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let kind = ActionKind::parse(&kind).ok_or(DomainError::NotFound)?;
        match kind {
            ActionKind::ChangePassword | ActionKind::ChangePasswordAfterBreach => {
                conn.execute(
                    "UPDATE account SET pwd_changed_at = date('now'), updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?1",
                    [account_id],
                )?;
                conn.execute(
                    "INSERT INTO password_check(account_id, checked_at, breach_count, strength, reuse_group)
                     VALUES (?1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), NULL, NULL, NULL)",
                    [account_id],
                )?;
                // Une brèche antérieure à ce changement ne compte plus : les correspondances sont réévaluées.
                crate::intel::update_matches(conn).map_err(|e| match e {
                    crate::intel::IntelError::Db(d) => DomainError::from(d),
                    _ => DomainError::NotFound,
                })?;
            }
            k if k.needs_mfa_kind() => {
                let m = mfa_kind
                    .ok_or_else(|| DomainError::InvalidInput("méthode MFA requise".into()))?;
                if k != ActionKind::EnableMfa && !is_strong(m) {
                    return Err(DomainError::InvalidInput(
                        "choisis une méthode forte : passkey, clé matérielle ou TOTP".into(),
                    ));
                }
                conn.execute(
                    "DELETE FROM mfa_method WHERE account_id=?1 AND kind=?2",
                    params![account_id, m.as_str()],
                )?;
                conn.execute(
                    "INSERT INTO mfa_method(account_id, kind, backup_codes) VALUES (?1, ?2, 1)",
                    params![account_id, m.as_str()],
                )?;
            }
            ActionKind::DeleteUnused => {
                conn.execute(
                    "UPDATE account SET status=?2, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?1",
                    params![account_id, AccountStatus::Deleted.as_str()],
                )?;
            }
            _ => {}
        }
        conn.execute(
            "UPDATE action SET status='done', done_at = strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE id=?1",
            [id],
        )?;
        refresh(conn, rules)
    })
}

/// Ignorer ou rouvrir. Ignorer masque l'action sans changer le score (le risque reste réel).
pub fn set_status(conn: &Connection, id: u32, status: ActionStatus) -> Result<()> {
    if conn.execute(
        "UPDATE action SET status=?2, done_at=NULL WHERE id=?1",
        params![id, status.as_str()],
    )? == 0
    {
        return Err(DomainError::NotFound);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
