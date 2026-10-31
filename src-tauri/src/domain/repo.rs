use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{Connection, Row, params, params_from_iter};

use super::{
    Account, AccountFilter, AccountInput, AccountStatus, Criticality, DomainError, Identity,
    IdentityInput, IdentityKind, MfaKind, MfaMethod, SortKey, normalize_domain,
};

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')";
type Result<T> = std::result::Result<T, DomainError>;

/// Exécute `f` dans un point de sauvegarde : imbricable (ex. import de nombreux comptes
/// dans une transaction englobante) et annulé en cas d'erreur.
pub fn in_savepoint<T>(conn: &Connection, f: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("SAVEPOINT vigie_sp")?;
    match f() {
        Ok(v) => {
            conn.execute_batch("RELEASE vigie_sp")?;
            Ok(v)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK TO vigie_sp; RELEASE vigie_sp");
            Err(e)
        }
    }
}

fn invalid(msg: &str) -> DomainError {
    DomainError::InvalidInput(msg.into())
}

fn clean_opt(s: &Option<String>) -> Option<String> {
    s.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tags
        .iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty() && t.chars().count() <= 40)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn validate_date(d: &Option<String>) -> Result<Option<String>> {
    match clean_opt(d) {
        None => Ok(None),
        Some(s) => {
            let ok = s.len() == 10
                && s.bytes().enumerate().all(|(i, b)| match i {
                    4 | 7 => b == b'-',
                    _ => b.is_ascii_digit(),
                })
                && matches!(s[5..7].parse::<u8>(), Ok(1..=12))
                && matches!(s[8..10].parse::<u8>(), Ok(1..=31));
            if ok {
                Ok(Some(s))
            } else {
                Err(invalid("date attendue au format AAAA-MM-JJ"))
            }
        }
    }
}

// ---------- Comptes ----------

const ACCOUNT_COLS: &str = "a.id, a.domain, a.label, a.identity_id, a.username, a.criticality, \
     a.recovery_identity_id, a.pwd_changed_at, a.status, a.notes, a.created_at, a.updated_at";

fn row_to_account(r: &Row) -> rusqlite::Result<Account> {
    let bad = |c: usize| {
        rusqlite::Error::InvalidColumnType(c, "enum".into(), rusqlite::types::Type::Text)
    };
    Ok(Account {
        id: r.get(0)?,
        domain: r.get(1)?,
        label: r.get(2)?,
        identity_id: r.get(3)?,
        username: r.get(4)?,
        criticality: Criticality::parse(&r.get::<_, String>(5)?).ok_or_else(|| bad(5))?,
        recovery_identity_id: r.get(6)?,
        pwd_changed_at: r.get(7)?,
        status: AccountStatus::parse(&r.get::<_, String>(8)?).ok_or_else(|| bad(8))?,
        notes: r.get(9)?,
        tags: Vec::new(),
        mfa: Vec::new(),
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
    })
}

pub fn save_account(conn: &Connection, id: Option<u32>, input: &AccountInput) -> Result<Account> {
    in_savepoint(conn, || save_account_inner(conn, id, input))
}

fn save_account_inner(conn: &Connection, id: Option<u32>, input: &AccountInput) -> Result<Account> {
    let domain = normalize_domain(&input.domain)?;
    let label = input.label.trim();
    if label.is_empty() || label.chars().count() > 120 {
        return Err(invalid("libellé requis (120 caractères max)"));
    }
    let username = clean_opt(&input.username);
    let notes = clean_opt(&input.notes);
    let pwd_changed_at = validate_date(&input.pwd_changed_at)?;

    let tx = conn;
    tx.execute(
        "INSERT INTO service(domain, name) VALUES (?1, ?2) ON CONFLICT(domain) DO NOTHING",
        params![domain, label],
    )?;
    let id = match id {
        None => {
            tx.execute(
                &format!(
                    "INSERT INTO account(domain, label, identity_id, username, criticality, \
                     recovery_identity_id, pwd_changed_at, status, notes, created_at, updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,{NOW},{NOW})"
                ),
                params![
                    domain,
                    label,
                    input.identity_id,
                    username,
                    input.criticality.as_str(),
                    input.recovery_identity_id,
                    pwd_changed_at,
                    input.status.as_str(),
                    notes
                ],
            )?;
            u32::try_from(tx.last_insert_rowid()).map_err(|_| invalid("identifiant"))?
        }
        Some(id) => {
            let n = tx.execute(
                &format!(
                    "UPDATE account SET domain=?1, label=?2, identity_id=?3, username=?4, \
                     criticality=?5, recovery_identity_id=?6, pwd_changed_at=?7, status=?8, \
                     notes=?9, updated_at={NOW} WHERE id=?10"
                ),
                params![
                    domain,
                    label,
                    input.identity_id,
                    username,
                    input.criticality.as_str(),
                    input.recovery_identity_id,
                    pwd_changed_at,
                    input.status.as_str(),
                    notes,
                    id
                ],
            )?;
            if n == 0 {
                return Err(DomainError::NotFound);
            }
            id
        }
    };

    tx.execute("DELETE FROM account_tag WHERE account_id=?1", [id])?;
    for t in clean_tags(&input.tags) {
        tx.execute(
            "INSERT INTO account_tag(account_id, tag) VALUES (?1, ?2)",
            params![id, t],
        )?;
    }
    tx.execute("DELETE FROM mfa_method WHERE account_id=?1", [id])?;
    let mut seen = Vec::new();
    for m in &input.mfa {
        if seen.contains(&m.kind) {
            continue;
        }
        seen.push(m.kind);
        tx.execute(
            "INSERT INTO mfa_method(account_id, kind, backup_codes) VALUES (?1, ?2, ?3)",
            params![id, m.kind.as_str(), m.backup_codes],
        )?;
    }
    get_account(conn, id)
}

pub fn get_account(conn: &Connection, id: u32) -> Result<Account> {
    let mut a = conn.query_row(
        &format!("SELECT {ACCOUNT_COLS} FROM account a WHERE a.id=?1"),
        [id],
        row_to_account,
    )?;
    a.tags = load_tags(conn)?.remove(&id).unwrap_or_default();
    a.mfa = load_mfa(conn)?.remove(&id).unwrap_or_default();
    Ok(a)
}

pub fn delete_account(conn: &Connection, id: u32) -> Result<()> {
    if conn.execute("DELETE FROM account WHERE id=?1", [id])? == 0 {
        return Err(DomainError::NotFound);
    }
    Ok(())
}

fn like_pattern(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

pub fn list_accounts(conn: &Connection, f: &AccountFilter) -> Result<Vec<Account>> {
    let mut conds: Vec<String> = Vec::new();
    let mut args: Vec<Value> = Vec::new();

    if let Some(q) = f.search.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        conds.push(
            "(a.label LIKE ? ESCAPE '\\' OR a.domain LIKE ? ESCAPE '\\' \
             OR a.username LIKE ? ESCAPE '\\' OR a.notes LIKE ? ESCAPE '\\')"
                .into(),
        );
        for _ in 0..4 {
            args.push(Value::Text(like_pattern(q)));
        }
    }
    if let Some(s) = f.status {
        conds.push("a.status = ?".into());
        args.push(Value::Text(s.as_str().into()));
    }
    if let Some(c) = f.criticality {
        conds.push("a.criticality = ?".into());
        args.push(Value::Text(c.as_str().into()));
    }
    if f.no_mfa {
        conds.push("NOT EXISTS (SELECT 1 FROM mfa_method m WHERE m.account_id = a.id)".into());
    }
    if let Some(t) = f.tag.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        conds.push(
            "EXISTS (SELECT 1 FROM account_tag t WHERE t.account_id = a.id AND t.tag = ?)".into(),
        );
        args.push(Value::Text(t.to_lowercase()));
    }

    let order = match f.sort {
        SortKey::Label => "a.label COLLATE NOCASE",
        SortKey::Domain => "a.domain",
        SortKey::UpdatedAt => "a.updated_at",
        SortKey::Criticality => {
            "CASE a.criticality WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 ELSE 3 END"
        }
    };
    let dir = if f.descending { "DESC" } else { "ASC" };
    let where_clause = if conds.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conds.join(" AND "))
    };
    let sql = format!(
        "SELECT {ACCOUNT_COLS} FROM account a {where_clause} ORDER BY {order} {dir}, a.id ASC"
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut accounts: Vec<Account> = stmt
        .query_map(params_from_iter(args), row_to_account)?
        .collect::<rusqlite::Result<_>>()?;
    let mut tags = load_tags(conn)?;
    let mut mfa = load_mfa(conn)?;
    for a in &mut accounts {
        a.tags = tags.remove(&a.id).unwrap_or_default();
        a.mfa = mfa.remove(&a.id).unwrap_or_default();
    }
    Ok(accounts)
}

fn load_tags(conn: &Connection) -> Result<HashMap<u32, Vec<String>>> {
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT account_id, tag FROM account_tag ORDER BY tag")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)))? {
        let (id, tag) = row?;
        map.entry(id).or_default().push(tag);
    }
    Ok(map)
}

fn load_mfa(conn: &Connection) -> Result<HashMap<u32, Vec<MfaMethod>>> {
    let mut map: HashMap<u32, Vec<MfaMethod>> = HashMap::new();
    let mut stmt =
        conn.prepare("SELECT account_id, kind, backup_codes FROM mfa_method ORDER BY id")?;
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, u32>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, bool>(2)?,
        ))
    })? {
        let (id, kind, backup_codes) = row?;
        if let Some(kind) = MfaKind::parse(&kind) {
            map.entry(id)
                .or_default()
                .push(MfaMethod { kind, backup_codes });
        }
    }
    Ok(map)
}

pub fn list_tags(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT DISTINCT tag FROM account_tag ORDER BY tag")?;
    let tags = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(tags)
}

// ---------- Identités ----------

fn normalize_identity(kind: IdentityKind, value: &str) -> Result<String> {
    let v = value.trim();
    match kind {
        IdentityKind::Email => {
            let v = v.to_lowercase();
            let ok = v.len() <= 254
                && v.split_once('@').is_some_and(|(l, d)| {
                    !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')
                })
                && !v.contains(char::is_whitespace);
            if ok {
                Ok(v)
            } else {
                Err(invalid("courriel invalide"))
            }
        }
        IdentityKind::Phone => {
            let plus = v.starts_with('+');
            let digits: String = v.chars().filter(char::is_ascii_digit).collect();
            let other_ok = v
                .chars()
                .all(|c| c.is_ascii_digit() || " +-.()".contains(c));
            if !other_ok || !(6..=15).contains(&digits.len()) {
                return Err(invalid("numéro de téléphone invalide"));
            }
            Ok(if plus { format!("+{digits}") } else { digits })
        }
    }
}

fn row_to_identity(r: &Row) -> rusqlite::Result<Identity> {
    let kind = IdentityKind::parse(&r.get::<_, String>(1)?).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(1, "kind".into(), rusqlite::types::Type::Text)
    })?;
    Ok(Identity {
        id: r.get(0)?,
        kind,
        value: r.get(2)?,
        account_id: r.get(3)?,
    })
}

pub fn list_identities(conn: &Connection) -> Result<Vec<Identity>> {
    let mut stmt =
        conn.prepare("SELECT id, kind, value, account_id FROM identity ORDER BY kind, value")?;
    let v = stmt
        .query_map([], row_to_identity)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(v)
}

pub fn save_identity(
    conn: &Connection,
    id: Option<u32>,
    input: &IdentityInput,
) -> Result<Identity> {
    let value = normalize_identity(input.kind, &input.value)?;
    let id = match id {
        None => {
            conn.execute(
                "INSERT INTO identity(kind, value, account_id) VALUES (?1, ?2, ?3)",
                params![input.kind.as_str(), value, input.account_id],
            )?;
            u32::try_from(conn.last_insert_rowid()).map_err(|_| invalid("identifiant"))?
        }
        Some(id) => {
            let n = conn.execute(
                "UPDATE identity SET kind=?1, value=?2, account_id=?3 WHERE id=?4",
                params![input.kind.as_str(), value, input.account_id, id],
            )?;
            if n == 0 {
                return Err(DomainError::NotFound);
            }
            id
        }
    };
    Ok(conn.query_row(
        "SELECT id, kind, value, account_id FROM identity WHERE id=?1",
        [id],
        row_to_identity,
    )?)
}

pub fn delete_identity(conn: &Connection, id: u32) -> Result<()> {
    if conn.execute("DELETE FROM identity WHERE id=?1", [id])? == 0 {
        return Err(DomainError::NotFound);
    }
    Ok(())
}

// ---------- Réglages (jamais de secret) ----------

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    match conn.query_row("SELECT value FROM setting WHERE key=?1", [key], |r| {
        r.get(0)
    }) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}
