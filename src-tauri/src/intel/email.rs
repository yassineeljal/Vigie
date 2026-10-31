//! Vérification par courriel (option). Le courriel part en clair vers HIBP : à utiliser
//! uniquement avec la clé API de l'utilisateur, et seulement sur demande explicite.

use rusqlite::{Connection, params};
use secrecy::{ExposeSecret, SecretString};

use super::breaches::is_relevant;
use super::{IntelClient, IntelError};

/// Noms des brèches contenant ce courriel (vide si aucune).
pub async fn check_email(
    client: &IntelClient,
    api_key: &SecretString,
    email: &str,
) -> Result<Vec<String>, IntelError> {
    // Encodage strict (« @ » → %40) ; les courriels sont validés à la saisie (pas d'espace).
    let encoded: String = url::form_urlencoded::byte_serialize(email.as_bytes()).collect();
    let url = format!(
        "{}/breachedaccount/{encoded}?truncateResponse=true",
        client.endpoints.hibp
    );
    let body = client
        .get(&url, &[("hibp-api-key", api_key.expose_secret())])
        .await?;
    let Some(body) = body else {
        return Ok(vec![]);
    };
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Item {
        name: String,
    }
    let items: Vec<Item> =
        serde_json::from_slice(&body).map_err(|e| IntelError::BadResponse(e.to_string()))?;
    Ok(items.into_iter().map(|i| i.name).collect())
}

/// Enregistre les correspondances « courriel » pour les comptes qui se connectent avec cette identité.
/// Retourne le nombre de correspondances écrites. Seules les brèches du catalogue en cache comptent.
pub fn store_email_matches(
    conn: &Connection,
    identity_id: u32,
    breach_names: &[String],
) -> Result<u32, IntelError> {
    conn.execute(
        "DELETE FROM breach_match WHERE kind='email'
         AND account_id IN (SELECT id FROM account WHERE identity_id = ?1)",
        [identity_id],
    )?;
    let mut written = 0;
    for name in breach_names {
        let Some((date, classes)) = conn
            .query_row(
                "SELECT breach_date, data_classes FROM breach WHERE name = ?1",
                [name],
                |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?)),
            )
            .ok()
        else {
            continue;
        };
        let classes: Vec<String> = serde_json::from_str(&classes).unwrap_or_default();
        let accounts: Vec<(u32, Option<String>)> = conn
            .prepare("SELECT id, pwd_changed_at FROM account WHERE identity_id = ?1 AND status != 'deleted'")?
            .query_map([identity_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (account, changed) in accounts {
            let relevant = is_relevant(date.as_deref(), &classes, changed.as_deref());
            written += conn.execute(
                "INSERT OR REPLACE INTO breach_match(breach_name, account_id, kind, relevant) VALUES (?1, ?2, 'email', ?3)",
                params![name, account, relevant],
            )? as u32;
        }
    }
    Ok(written)
}
