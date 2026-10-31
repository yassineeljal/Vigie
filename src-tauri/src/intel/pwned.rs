//! Pwned Passwords avec k-anonymat : seuls les 5 premiers caractères du SHA-1 sont envoyés,
//! avec `Add-Padding: true`. Le mot de passe et son hash sont effacés après usage (US-30).

use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretString};
use sha1::{Digest, Sha1};
use zeroize::Zeroizing;

use super::{IntelClient, IntelError};

/// SHA-1 majuscule du mot de passe, scindé en (préfixe de 5 caractères, suffixe).
pub fn sha1_split(password: &SecretString) -> (String, Zeroizing<String>) {
    let digest = Sha1::digest(password.expose_secret().as_bytes());
    let hex = Zeroizing::new(data_encoding::HEXUPPER.encode(&digest));
    (hex[..5].to_owned(), Zeroizing::new(hex[5..].to_owned()))
}

/// Interroge une plage ; retourne `(suffixe, nombre)` en ignorant les lignes de padding (nombre 0).
pub(super) async fn range_lookup(
    client: &IntelClient,
    prefix: &str,
) -> Result<Vec<(String, u32)>, IntelError> {
    debug_assert!(prefix.len() == 5 && prefix.bytes().all(|b| b.is_ascii_hexdigit()));
    let url = format!("{}/range/{}", client.endpoints.pwned, prefix);
    let body = client
        .get(&url, &[("Add-Padding", "true")])
        .await?
        .ok_or(IntelError::Http(404))?;
    let text = String::from_utf8(body).map_err(|_| IntelError::BadResponse("UTF-8".into()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((suffix, count)) = line.trim().split_once(':') else {
            continue;
        };
        let count: u32 = count.trim().parse().unwrap_or(0);
        if count > 0 && suffix.len() == 35 {
            out.push((suffix.to_ascii_uppercase(), count));
        }
    }
    Ok(out)
}

pub fn count_in_range(range: &[(String, u32)], suffix: &str) -> u32 {
    range
        .iter()
        .find(|(s, _)| s == suffix)
        .map_or(0, |(_, c)| *c)
}

/// Nombre d'apparitions du mot de passe dans des fuites (0 si absent).
pub async fn pwned_count(client: &IntelClient, password: &SecretString) -> Result<u32, IntelError> {
    let (prefix, suffix) = sha1_split(password);
    let range = range_lookup(client, &prefix).await?;
    Ok(count_in_range(&range, &suffix))
}

/// Variante avec cache de plages (utilisée par les tests ; la production passe par `pwned_ranges`) (un même préfixe n'est demandé qu'une fois par lot).
#[cfg(test)]
pub async fn pwned_count_cached(
    client: &IntelClient,
    password: &SecretString,
    cache: &mut HashMap<String, Vec<(String, u32)>>,
) -> Result<u32, IntelError> {
    let (prefix, suffix) = sha1_split(password);
    if !cache.contains_key(&prefix) {
        let r = range_lookup(client, &prefix).await?;
        cache.insert(prefix.clone(), r);
    }
    Ok(count_in_range(&cache[&prefix], &suffix))
}

/// Récupère plusieurs plages avec une concurrence limitée (import de nombreux comptes, US-25).
pub async fn pwned_ranges(
    client: std::sync::Arc<IntelClient>,
    prefixes: Vec<String>,
) -> Result<HashMap<String, Vec<(String, u32)>>, IntelError> {
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;
    let sem = std::sync::Arc::new(Semaphore::new(8));
    let mut set = JoinSet::new();
    for prefix in prefixes {
        let (client, sem) = (client.clone(), sem.clone());
        set.spawn(async move {
            let _permit = sem.acquire().await.map_err(|_| IntelError::Offline)?;
            let r = range_lookup(&client, &prefix).await?;
            Ok::<_, IntelError>((prefix, r))
        });
    }
    let mut out = HashMap::new();
    while let Some(res) = set.join_next().await {
        let (prefix, r) = res.map_err(|_| IntelError::Offline)??;
        out.insert(prefix, r);
    }
    Ok(out)
}
