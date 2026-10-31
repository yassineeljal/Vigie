//! Export JSON/CSV. Aucun mot de passe n'existe dans le coffre ; les notes (texte libre qui
//! pourrait contenir n'importe quoi) sont exclues par défaut. Les cellules CSV qui pourraient
//! être interprétées comme des formules par un tableur sont neutralisées (M7).

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use specta::Type;

use crate::domain::{self, AccountFilter, DomainError, SortKey};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Json,
    Csv,
}

#[derive(Debug, Clone, Copy, Deserialize, Type)]
pub struct ExportOptions {
    pub include_notes: bool,
}

type Result<T> = std::result::Result<T, DomainError>;

fn all_accounts(conn: &Connection) -> Result<Vec<domain::Account>> {
    domain::list_accounts(
        conn,
        &AccountFilter {
            search: None,
            status: None,
            criticality: None,
            no_mfa: false,
            tag: None,
            sort: SortKey::Domain,
            descending: false,
        },
    )
}

fn identity_values(conn: &Connection) -> Result<std::collections::HashMap<u32, String>> {
    Ok(domain::list_identities(conn)?
        .into_iter()
        .map(|i| (i.id, i.value))
        .collect())
}

/// Neutralise l'injection de formules : une cellule qui commence par `= + - @` ou un caractère
/// de contrôle de tabulation/retour est préfixée d'une apostrophe.
pub fn csv_safe(cell: &str) -> String {
    match cell.chars().next() {
        Some('=' | '+' | '-' | '@' | '\t' | '\r') => format!("'{cell}"),
        _ => cell.to_owned(),
    }
}

pub fn export_csv(conn: &Connection, opts: ExportOptions) -> Result<Vec<u8>> {
    let idents = identity_values(conn)?;
    let mut w = csv::Writer::from_writer(vec![]);
    let mut header = vec![
        "domain",
        "label",
        "username",
        "login_identity",
        "recovery_identity",
        "criticality",
        "status",
        "mfa",
        "tags",
        "password_changed_on",
    ];
    if opts.include_notes {
        header.push("notes");
    }
    let csv_err = |_| DomainError::InvalidInput("écriture CSV impossible".into());
    w.write_record(&header).map_err(csv_err)?;
    for a in all_accounts(conn)? {
        let mfa = a
            .mfa
            .iter()
            .map(|m| {
                format!(
                    "{}{}",
                    serde_json::to_value(m.kind)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    if m.backup_codes { "+codes" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(";");
        let get = |id: Option<u32>| id.and_then(|i| idents.get(&i)).cloned().unwrap_or_default();
        let mut row = vec![
            a.domain.clone(),
            a.label.clone(),
            a.username.clone().unwrap_or_default(),
            get(a.identity_id),
            get(a.recovery_identity_id),
            a.criticality.as_str().to_owned(),
            a.status.as_str().to_owned(),
            mfa,
            a.tags.join(";"),
            a.pwd_changed_at.clone().unwrap_or_default(),
        ];
        if opts.include_notes {
            row.push(a.notes.clone().unwrap_or_default());
        }
        w.write_record(row.iter().map(|c| csv_safe(c)))
            .map_err(csv_err)?;
    }
    w.into_inner()
        .map_err(|_| DomainError::InvalidInput("écriture CSV impossible".into()))
}

pub fn export_json(conn: &Connection, opts: ExportOptions) -> Result<Vec<u8>> {
    let idents = identity_values(conn)?;
    let accounts = all_accounts(conn)?;
    let domain_of: std::collections::HashMap<u32, String> =
        accounts.iter().map(|a| (a.id, a.domain.clone())).collect();
    let exported_at: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |r| {
            r.get(0)
        })?;
    let get = |id: Option<u32>| id.and_then(|i| idents.get(&i)).cloned();
    let accounts_json: Vec<_> = accounts
        .iter()
        .map(|a| {
            let mut v = json!({
                "domain": a.domain,
                "label": a.label,
                "username": a.username,
                "login_identity": get(a.identity_id),
                "recovery_identity": get(a.recovery_identity_id),
                "criticality": a.criticality,
                "status": a.status,
                "mfa": a.mfa,
                "tags": a.tags,
                "password_changed_on": a.pwd_changed_at,
            });
            if opts.include_notes {
                v["notes"] = json!(a.notes);
            }
            v
        })
        .collect();
    let identities: Vec<_> = domain::list_identities(conn)?
        .into_iter()
        .map(|i| json!({ "kind": i.kind, "value": i.value, "account_domain": i.account_id.and_then(|id| domain_of.get(&id)) }))
        .collect();
    let doc = json!({
        "format": "vigie-inventory",
        "version": 1,
        "exported_at": exported_at,
        "contains_passwords": false,
        "accounts": accounts_json,
        "identities": identities,
    });
    serde_json::to_vec_pretty(&doc)
        .map_err(|_| DomainError::InvalidInput("écriture JSON impossible".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::KdfParams;
    use crate::domain::{
        AccountInput, AccountStatus, Criticality, IdentityInput, IdentityKind, MfaKind, MfaMethod,
    };
    use crate::vault::Vault;
    use secrecy::SecretString;

    fn setup() -> (tempfile::TempDir, Vault) {
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
        let c = v.conn();
        let mail = domain::save_identity(
            c,
            None,
            &IdentityInput {
                kind: IdentityKind::Email,
                value: "moi@example.test".into(),
                account_id: None,
            },
        )
        .unwrap();
        for (domain_, label, notes) in [
            (
                "exemple.com",
                "=HYPERLINK(\"http://evil.test\",\"clic\")",
                Some("pense-bête secret"),
            ),
            ("autre.org", "+cmd|' /C calc'!A0", None),
            ("normal.net", "Normal, avec virgule", Some("-1+1")),
        ] {
            domain::save_account(
                c,
                None,
                &AccountInput {
                    domain: domain_.into(),
                    label: label.into(),
                    identity_id: Some(mail.id),
                    username: Some("@alice".into()),
                    criticality: Criticality::High,
                    recovery_identity_id: None,
                    pwd_changed_at: Some("2024-01-02".into()),
                    status: AccountStatus::Active,
                    notes: notes.map(Into::into),
                    tags: vec!["perso".into(), "dev".into()],
                    mfa: vec![MfaMethod {
                        kind: MfaKind::Totp,
                        backup_codes: true,
                    }],
                },
            )
            .unwrap();
        }
        (d, v)
    }

    #[test]
    fn csv_neutralise_les_formules_et_exclut_les_notes_par_defaut() {
        let (_d, v) = setup();
        let out = String::from_utf8(
            export_csv(
                v.conn(),
                ExportOptions {
                    include_notes: false,
                },
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!out.contains("pense-bête"));
        assert!(!out.contains("notes"));
        let mut rdr = csv::Reader::from_reader(out.as_bytes());
        let rows: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        assert_eq!(rows.len(), 3);
        for r in &rows {
            for cell in r.iter() {
                assert!(
                    !cell.starts_with(['=', '+', '-', '@']),
                    "cellule dangereuse : {cell}"
                );
            }
        }
        let by_domain = |d: &str| rows.iter().find(|r| &r[0] == d).unwrap().clone();
        assert_eq!(
            &by_domain("exemple.com")[1],
            "'=HYPERLINK(\"http://evil.test\",\"clic\")"
        );
        assert_eq!(&by_domain("autre.org")[1], "'+cmd|' /C calc'!A0");
        assert_eq!(&by_domain("normal.net")[1], "Normal, avec virgule");
        assert_eq!(&by_domain("normal.net")[2], "'@alice"); // identifiant commençant par @
        assert_eq!(&by_domain("normal.net")[7], "totp+codes");
        assert_eq!(&by_domain("normal.net")[8], "dev;perso");
    }

    #[test]
    fn csv_avec_notes_neutralise_aussi_les_notes() {
        let (_d, v) = setup();
        let out = String::from_utf8(
            export_csv(
                v.conn(),
                ExportOptions {
                    include_notes: true,
                },
            )
            .unwrap(),
        )
        .unwrap();
        let mut rdr = csv::Reader::from_reader(out.as_bytes());
        let rows: Vec<csv::StringRecord> = rdr.records().map(|r| r.unwrap()).collect();
        let n = rows.iter().find(|r| &r[0] == "normal.net").unwrap();
        assert_eq!(&n[10], "'-1+1");
    }

    #[test]
    fn json_sans_secrets() {
        let (_d, v) = setup();
        let raw = export_json(
            v.conn(),
            ExportOptions {
                include_notes: false,
            },
        )
        .unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(doc["format"], "vigie-inventory");
        assert_eq!(doc["contains_passwords"], false);
        assert_eq!(doc["accounts"].as_array().unwrap().len(), 3);
        assert_eq!(doc["identities"][0]["value"], "moi@example.test");
        assert_eq!(doc["accounts"][0]["login_identity"], "moi@example.test");
        assert!(doc["accounts"][0].get("notes").is_none());
        let text = String::from_utf8(raw).unwrap();
        assert!(!text.contains("pense-bête"));
        let with = export_json(
            v.conn(),
            ExportOptions {
                include_notes: true,
            },
        )
        .unwrap();
        assert!(String::from_utf8(with).unwrap().contains("pense-bête"));
    }

    #[test]
    fn csv_safe_cas_limites() {
        assert_eq!(csv_safe(""), "");
        assert_eq!(csv_safe("ok"), "ok");
        assert_eq!(csv_safe("\tx"), "'\tx");
        assert_eq!(csv_safe("a=b"), "a=b");
    }
}
