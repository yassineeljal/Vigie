//! Tâches périodiques et notifications (US-53). Les notifications ne contiennent jamais de
//! noms de comptes ni de courriels : elles peuvent s'afficher sur un écran verrouillé.

use rusqlite::Connection;

use crate::domain::{self, DomainError};

pub const SETTING_NOTIFICATIONS: &str = "notifications_enabled";
const SETTING_LAST_REMINDER: &str = "last_weekly_reminder";
const REMINDER_EVERY_DAYS: i64 = 7;

pub fn notifications_enabled(conn: &Connection) -> bool {
    domain::get_setting(conn, SETTING_NOTIFICATIONS)
        .ok()
        .flatten()
        .as_deref()
        == Some("1")
}

/// Le rappel hebdomadaire est dû s'il n'a jamais été envoyé ou s'il date d'au moins 7 jours.
pub fn weekly_reminder_due(conn: &Connection) -> Result<bool, DomainError> {
    let Some(last) = domain::get_setting(conn, SETTING_LAST_REMINDER)? else {
        return Ok(true);
    };
    let days: Option<i64> = conn.query_row(
        "SELECT CAST(julianday('now') - julianday(?1) AS INTEGER)",
        [last],
        |r| r.get(0),
    )?;
    Ok(days.is_none_or(|d| d >= REMINDER_EVERY_DAYS))
}

pub fn mark_reminder_sent(conn: &Connection) -> Result<(), DomainError> {
    domain::set_setting(conn, SETTING_LAST_REMINDER, &today(conn)?)
}

fn today(conn: &Connection) -> Result<String, DomainError> {
    Ok(
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| {
            r.get(0)
        })?,
    )
}

pub const SETTING_LANGUAGE: &str = "language";

/// Langue de l'interface (« fr » par défaut), pour les textes produits côté Rust.
pub fn language(conn: &Connection) -> &'static str {
    match domain::get_setting(conn, SETTING_LANGUAGE)
        .ok()
        .flatten()
        .as_deref()
    {
        Some("en") => "en",
        _ => "fr",
    }
}

pub fn breach_message(lang: &str, newly_relevant: u32) -> (String, String) {
    if lang == "en" {
        return (
            "New relevant breach".into(),
            if newly_relevant == 1 {
                "One of your accounts is affected by a recent breach. Open Vigie to see what to do."
                    .into()
            } else {
                format!(
                    "{newly_relevant} of your accounts are affected by recent breaches. Open Vigie to see what to do."
                )
            },
        );
    }
    (
        "Nouvelle brèche pertinente".into(),
        if newly_relevant == 1 {
            "Un de tes comptes est touché par une brèche récente. Ouvre Vigie pour voir quoi faire."
                .into()
        } else {
            format!(
                "{newly_relevant} de tes comptes sont touchés par des brèches récentes. Ouvre Vigie pour voir quoi faire."
            )
        },
    )
}

pub fn reminder_message(lang: &str, pending: u32, gain: f64) -> (String, String) {
    if lang == "en" {
        return (
            "Weekly reminder".into(),
            format!("{pending} security action(s) pending (potential gain: {gain:.0} points)."),
        );
    }
    (
        "Rappel hebdomadaire".into(),
        format!("{pending} action(s) de sécurité en attente (gain potentiel : {gain:.0} points)."),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KdfParams;
    use crate::vault::Vault;
    use secrecy::SecretString;

    fn vault() -> (tempfile::TempDir, Vault) {
        let d = tempfile::TempDir::new().unwrap();
        let (v, _) = Vault::create(
            d.path(),
            &SecretString::from("correct horse battery staple"),
            KdfParams {
                m_kib: 8,
                t: 1,
                p: 1,
            },
        )
        .unwrap();
        (d, v)
    }

    #[test]
    fn rappel_hebdomadaire() {
        let (_d, v) = vault();
        let c = v.conn();
        assert!(weekly_reminder_due(c).unwrap());
        mark_reminder_sent(c).unwrap();
        assert!(!weekly_reminder_due(c).unwrap());
        domain::set_setting(c, SETTING_LAST_REMINDER, "2020-01-01T00:00:00Z").unwrap();
        assert!(weekly_reminder_due(c).unwrap());
    }

    #[test]
    fn reglage_desactive_par_defaut() {
        let (_d, v) = vault();
        assert!(!notifications_enabled(v.conn()));
        domain::set_setting(v.conn(), SETTING_NOTIFICATIONS, "1").unwrap();
        assert!(notifications_enabled(v.conn()));
    }

    #[test]
    fn messages_sans_donnees_personnelles() {
        let (t, b) = breach_message("fr", 1);
        assert!(t.contains("brèche") && b.contains("Un de tes comptes"));
        assert!(breach_message("fr", 3).1.starts_with("3 de tes comptes"));
        assert!(reminder_message("fr", 4, 62.5).1.contains("4 action(s)"));
        assert!(breach_message("en", 2).1.starts_with("2 of your accounts"));
        assert!(
            reminder_message("en", 4, 62.5)
                .1
                .contains("4 security action(s)")
        );
    }

    #[test]
    fn langue_par_defaut_et_reglage() {
        let (_d, v) = vault();
        assert_eq!(language(v.conn()), "fr");
        domain::set_setting(v.conn(), SETTING_LANGUAGE, "en").unwrap();
        assert_eq!(language(v.conn()), "en");
        domain::set_setting(v.conn(), SETTING_LANGUAGE, "de").unwrap();
        assert_eq!(language(v.conn()), "fr");
    }
}
