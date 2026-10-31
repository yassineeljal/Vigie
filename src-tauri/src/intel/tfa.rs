//! Catalogue 2fa.directory (US-34) : méthodes MFA et passkeys supportés par chaque site.
//! Téléchargement public ; seuls les services déjà présents dans l'inventaire sont mis à jour.

use rusqlite::{Connection, params};
use serde_json::Value;

use super::{IntelClient, IntelError};
use crate::domain::normalize_domain;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TfaEntry {
    pub domains: Vec<String>,
    pub methods: Vec<String>,
    pub passkeys: bool,
}

impl IntelClient {
    pub async fn fetch_tfa(&self) -> Result<Vec<TfaEntry>, IntelError> {
        let url = format!("{}/all.json", self.endpoints.tfa);
        let body = self.get(&url, &[]).await?.ok_or(IntelError::Http(404))?;
        let v: Value =
            serde_json::from_slice(&body).map_err(|e| IntelError::BadResponse(e.to_string()))?;
        parse_tfa(&v)
    }
}

/// Le catalogue est une liste de `[nom, {domain, tfa, …}]` ; on accepte aussi une liste d'objets.
pub fn parse_tfa(v: &Value) -> Result<Vec<TfaEntry>, IntelError> {
    let list = v
        .as_array()
        .ok_or_else(|| IntelError::BadResponse("liste attendue".into()))?;
    let mut out = Vec::new();
    for item in list {
        let obj = match item {
            Value::Array(pair) => pair.get(1),
            Value::Object(_) => Some(item),
            _ => None,
        };
        let Some(obj) = obj.and_then(Value::as_object) else {
            continue;
        };
        let mut domains: Vec<String> = obj
            .get("domain")
            .and_then(Value::as_str)
            .into_iter()
            .chain(
                obj.get("additional-domains")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            )
            .filter_map(|d| normalize_domain(d).ok())
            .collect();
        domains.sort();
        domains.dedup();
        if domains.is_empty() {
            continue;
        }
        let methods: Vec<String> = obj
            .get("tfa")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_lowercase)
                    .collect()
            })
            .unwrap_or_default();
        let passkeys = ["passwordless", "passkeys"]
            .iter()
            .filter_map(|k| obj.get(*k))
            .any(|v| match v {
                Value::String(s) => matches!(s.as_str(), "allowed" | "required" | "yes"),
                Value::Bool(b) => *b,
                _ => false,
            })
            || methods.iter().any(|m| m == "passkey");
        out.push(TfaEntry {
            domains,
            methods,
            passkeys,
        });
    }
    Ok(out)
}

/// Met à jour `service` pour les domaines de l'inventaire. Retourne le nombre de services touchés.
pub fn store_tfa(conn: &Connection, entries: &[TfaEntry]) -> Result<u32, IntelError> {
    let mut touched = 0;
    for e in entries {
        let json = serde_json::to_string(&e.methods).unwrap_or_else(|_| "[]".into());
        for d in &e.domains {
            touched += conn.execute(
                "UPDATE service SET supported_mfa=?2, passkeys=?3, synced_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')
                 WHERE domain=?1",
                params![d, json, e.passkeys],
            )? as u32;
        }
    }
    Ok(touched)
}
