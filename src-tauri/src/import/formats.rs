//! Analyseurs des exports (M5 : limites strictes, aucune panique sur entrée arbitraire).
//! Les mots de passe restent en `SecretString` et ne quittent jamais le cœur Rust.

use secrecy::SecretString;
use serde::Deserialize;
use specta::Type;

use super::ImportError;

pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ROWS: usize = 50_000;
const MAX_NAME: usize = 200;
const MAX_NOTES: usize = 10_000;
const MAX_FIELD: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ImportFormat {
    BitwardenCsv,
    BitwardenJson,
    ApplePasswords,
    /// Chrome, Edge, Brave (`name,url,username,password,note`) et Firefox (avec `timePasswordChanged`).
    BrowserCsv,
    GenericCsv,
}

/// Mappage de colonnes pour un CSV quelconque (noms d'en-têtes).
#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct ColumnMapping {
    pub name: Option<String>,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub totp: Option<String>,
    pub notes: Option<String>,
    pub folder: Option<String>,
}

pub struct ParsedEntry {
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: Option<SecretString>,
    pub has_totp: bool,
    pub notes: Option<String>,
    pub folder: Option<String>,
    /// AAAA-MM-JJ si l'export la fournit de façon fiable.
    pub pwd_changed_at: Option<String>,
}

#[derive(Default)]
pub struct ParseOutcome {
    pub entries: Vec<ParsedEntry>,
    /// Lignes ignorées (non-identifiants, lignes vides, lignes illisibles).
    pub skipped: u32,
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}
fn opt(s: &str, max: usize) -> Option<String> {
    let c = clip(s, max);
    (!c.is_empty()).then_some(c)
}
fn secret(s: &str) -> Option<SecretString> {
    // Les mots de passe ne sont ni tronqués ni « trimés » : on veut la valeur exacte.
    let s: String = s.chars().take(MAX_FIELD).collect();
    (!s.is_empty()).then(|| SecretString::from(s))
}

pub fn detect(bytes: &[u8]) -> Option<ImportFormat> {
    let text = strip_bom(bytes);
    if text.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        return Some(ImportFormat::BitwardenJson);
    }
    let headers = read_headers(text).ok()?.1;
    let has = |h: &str| headers.iter().any(|x| x.eq_ignore_ascii_case(h));
    if has("login_uri") && has("login_password") {
        Some(ImportFormat::BitwardenCsv)
    } else if has("title") && has("url") && has("username") && has("password") {
        Some(ImportFormat::ApplePasswords)
    } else if has("url")
        && has("username")
        && has("password")
        && (has("name") || has("timePasswordChanged"))
    {
        Some(ImportFormat::BrowserCsv)
    } else {
        None
    }
}

pub fn parse(
    bytes: &[u8],
    format: ImportFormat,
    mapping: Option<&ColumnMapping>,
) -> Result<ParseOutcome, ImportError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge);
    }
    let text = strip_bom(bytes);
    match format {
        ImportFormat::BitwardenJson => parse_bitwarden_json(text),
        ImportFormat::BitwardenCsv => parse_csv(
            text,
            &bitwarden_csv_mapping(),
            Some(("type", "login")),
            None,
        ),
        ImportFormat::ApplePasswords => parse_csv(text, &apple_mapping(), None, None),
        ImportFormat::BrowserCsv => {
            parse_csv(text, &browser_mapping(), None, Some("timePasswordChanged"))
        }
        ImportFormat::GenericCsv => {
            let m = mapping.ok_or(ImportError::Format("mappage de colonnes requis".into()))?;
            parse_csv(text, m, None, None)
        }
    }
}

fn strip_bom(b: &[u8]) -> &[u8] {
    b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b)
}

/// Détecte le séparateur (`,` `;` tabulation) d'après la première ligne.
fn sniff_delimiter(text: &[u8]) -> u8 {
    let first = text.split(|&b| b == b'\n').next().unwrap_or(&[]);
    b",;\t"
        .iter()
        .copied()
        .max_by_key(|d| first.iter().filter(|b| **b == *d).count())
        .unwrap_or(b',')
}

pub fn read_headers(text: &[u8]) -> Result<(u8, Vec<String>), ImportError> {
    let delim = sniff_delimiter(text);
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_reader(text);
    let headers = rdr
        .headers()
        .map_err(|_| ImportError::Format("en-têtes CSV illisibles".into()))?
        .iter()
        .map(|h| clip(h, 100))
        .collect::<Vec<_>>();
    if headers.is_empty() || headers.iter().all(String::is_empty) {
        return Err(ImportError::Format("fichier vide".into()));
    }
    Ok((delim, headers))
}

pub fn csv_headers(bytes: &[u8]) -> Result<Vec<String>, ImportError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge);
    }
    Ok(read_headers(strip_bom(bytes))?.1)
}

fn bitwarden_csv_mapping() -> ColumnMapping {
    ColumnMapping {
        name: Some("name".into()),
        url: Some("login_uri".into()),
        username: Some("login_username".into()),
        password: Some("login_password".into()),
        totp: Some("login_totp".into()),
        notes: Some("notes".into()),
        folder: Some("folder".into()),
    }
}

fn browser_mapping() -> ColumnMapping {
    ColumnMapping {
        name: Some("name".into()),
        url: Some("url".into()),
        username: Some("username".into()),
        password: Some("password".into()),
        totp: None,
        notes: Some("note".into()),
        folder: None,
    }
}

/// Convertit des millisecondes depuis 1970 en `AAAA-MM-JJ` (algorithme de Howard Hinnant).
pub(super) fn epoch_ms_to_date(ms: i64) -> Option<String> {
    if !(946_684_800_000..=32_503_680_000_000).contains(&ms) {
        return None; // 0 / inconnu (avant 2000) ou après l'an 3000 : valeur aberrante
    }
    let z = ms / 86_400_000 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

fn apple_mapping() -> ColumnMapping {
    ColumnMapping {
        name: Some("Title".into()),
        url: Some("URL".into()),
        username: Some("Username".into()),
        password: Some("Password".into()),
        totp: Some("OTPAuth".into()),
        notes: Some("Notes".into()),
        folder: None,
    }
}

fn parse_csv(
    text: &[u8],
    m: &ColumnMapping,
    only_rows: Option<(&str, &str)>,
    date_col: Option<&str>,
) -> Result<ParseOutcome, ImportError> {
    let (delim, headers) = read_headers(text)?;
    let idx = |name: &Option<String>| -> Option<usize> {
        let n = name.as_deref()?;
        headers.iter().position(|h| h.eq_ignore_ascii_case(n))
    };
    let (i_name, i_url, i_user, i_pw, i_totp, i_notes, i_folder) = (
        idx(&m.name),
        idx(&m.url),
        idx(&m.username),
        idx(&m.password),
        idx(&m.totp),
        idx(&m.notes),
        idx(&m.folder),
    );
    if i_name.is_none() && i_url.is_none() {
        return Err(ImportError::Format("colonne nom ou URL introuvable".into()));
    }
    let i_date = date_col.and_then(|c| headers.iter().position(|h| h.eq_ignore_ascii_case(c)));
    let i_filter =
        only_rows.and_then(|(col, _)| headers.iter().position(|h| h.eq_ignore_ascii_case(col)));

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_reader(text);
    let mut out = ParseOutcome::default();
    for (n, rec) in rdr.records().enumerate() {
        if n >= MAX_ROWS {
            return Err(ImportError::TooManyRows);
        }
        let Ok(rec) = rec else {
            out.skipped += 1;
            continue;
        };
        let get = |i: Option<usize>| i.and_then(|i| rec.get(i)).unwrap_or("");
        if let (Some(i), Some((_, want))) = (i_filter, only_rows)
            && !rec.get(i).unwrap_or("").eq_ignore_ascii_case(want)
        {
            out.skipped += 1;
            continue;
        }
        // Bitwarden sépare plusieurs URI par des sauts de ligne : on garde la première.
        let url = get(i_url).lines().next().unwrap_or("").trim().to_owned();
        let entry = ParsedEntry {
            name: clip(get(i_name), MAX_NAME),
            url: clip(&url, MAX_FIELD),
            username: clip(get(i_user), MAX_FIELD),
            password: secret(get(i_pw)),
            has_totp: !get(i_totp).trim().is_empty(),
            notes: opt(get(i_notes), MAX_NOTES),
            folder: opt(get(i_folder), MAX_NAME).filter(|f| !f.eq_ignore_ascii_case("no folder")),
            pwd_changed_at: i_date
                .and_then(|i| rec.get(i))
                .and_then(|v| v.trim().parse::<i64>().ok())
                .and_then(epoch_ms_to_date),
        };
        if entry.name.is_empty() && entry.url.is_empty() {
            out.skipped += 1;
            continue;
        }
        out.entries.push(entry);
    }
    Ok(out)
}

// ---------- Bitwarden JSON ----------

#[derive(Deserialize)]
struct BwExport {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    folders: Vec<BwFolder>,
    #[serde(default)]
    items: Vec<BwItem>,
}
#[derive(Deserialize)]
struct BwFolder {
    id: Option<String>,
    name: Option<String>,
}
#[derive(Deserialize)]
struct BwItem {
    #[serde(rename = "folderId")]
    folder_id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<u8>,
    name: Option<String>,
    notes: Option<String>,
    login: Option<BwLogin>,
    #[serde(default, rename = "passwordHistory")]
    password_history: Vec<BwHistory>,
}
#[derive(Deserialize)]
struct BwLogin {
    #[serde(default)]
    uris: Vec<BwUri>,
    username: Option<String>,
    password: Option<String>,
    totp: Option<String>,
    #[serde(rename = "passwordRevisionDate")]
    password_revision_date: Option<String>,
}
#[derive(Deserialize)]
struct BwUri {
    uri: Option<String>,
}
#[derive(Deserialize)]
struct BwHistory {
    #[serde(rename = "lastUsedDate")]
    last_used_date: Option<String>,
}

/// `2023-01-10T08:00:00.000Z` → `2023-01-10` (rejette tout ce qui ne ressemble pas à une date).
fn date_prefix(s: &str) -> Option<String> {
    let d = s.get(..10)?;
    let ok = d.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    ok.then(|| d.to_owned())
}

fn parse_bitwarden_json(text: &[u8]) -> Result<ParseOutcome, ImportError> {
    let export: BwExport = serde_json::from_slice(text)
        .map_err(|_| ImportError::Format("JSON Bitwarden invalide".into()))?;
    if export.encrypted {
        return Err(ImportError::Encrypted);
    }
    if export.items.len() > MAX_ROWS {
        return Err(ImportError::TooManyRows);
    }
    let folders: std::collections::HashMap<&str, &str> = export
        .folders
        .iter()
        .filter_map(|f| Some((f.id.as_deref()?, f.name.as_deref()?)))
        .collect();
    let mut out = ParseOutcome::default();
    for item in &export.items {
        let (Some(1), Some(login)) = (item.kind, item.login.as_ref()) else {
            out.skipped += 1;
            continue;
        };
        // Date de changement du mot de passe : on n'utilise que des dates fiables
        // (révision du mot de passe, sinon dernier historique) — jamais `revisionDate`,
        // qui change à chaque modification de l'entrée et surestimerait la fraîcheur.
        let pwd_changed_at = login
            .password_revision_date
            .as_deref()
            .and_then(date_prefix)
            .or_else(|| {
                item.password_history
                    .iter()
                    .filter_map(|h| h.last_used_date.as_deref().and_then(date_prefix))
                    .max()
            });
        let entry = ParsedEntry {
            name: clip(item.name.as_deref().unwrap_or(""), MAX_NAME),
            url: clip(
                login
                    .uris
                    .iter()
                    .find_map(|u| u.uri.as_deref())
                    .unwrap_or(""),
                MAX_FIELD,
            ),
            username: clip(login.username.as_deref().unwrap_or(""), MAX_FIELD),
            password: login.password.as_deref().and_then(secret),
            has_totp: login.totp.as_deref().is_some_and(|t| !t.trim().is_empty()),
            notes: item.notes.as_deref().and_then(|n| opt(n, MAX_NOTES)),
            folder: item
                .folder_id
                .as_deref()
                .and_then(|id| folders.get(id))
                .and_then(|n| opt(n, MAX_NAME)),
            pwd_changed_at,
        };
        if entry.name.is_empty() && entry.url.is_empty() {
            out.skipped += 1;
            continue;
        }
        out.entries.push(entry);
    }
    Ok(out)
}
