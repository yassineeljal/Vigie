use super::*;
use tempfile::TempDir;

const PW: &str = "correct horse battery staple";
const PW2: &str = "tr0ub4dor&3-but-much-longer-now";

fn fast() -> KdfParams {
    KdfParams {
        m_kib: 8,
        t: 1,
        p: 1,
    }
}
fn pw(s: &str) -> SecretString {
    SecretString::from(s)
}
fn new_vault(dir: &Path) -> RecoveryKey {
    Vault::create(dir, &pw(PW), fast()).unwrap().1
}
fn count_tables(v: &Vault) -> i64 {
    v.conn()
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table'",
            [],
            |r| r.get(0),
        )
        .unwrap()
}

#[test]
fn creation_puis_deverrouillage() {
    let d = TempDir::new().unwrap();
    let (v, _) = Vault::create(d.path(), &pw(PW), fast()).unwrap();
    assert!(count_tables(&v) >= 10);
    drop(v);
    assert!(Vault::exists(d.path()));
    let v = Vault::unlock(d.path(), &pw(PW)).unwrap();
    assert!(count_tables(&v) >= 10);
}

#[test]
fn mauvais_mot_de_passe() {
    let d = TempDir::new().unwrap();
    new_vault(d.path());
    assert!(matches!(
        Vault::unlock(d.path(), &pw("pas le bon mot de passe du tout")),
        Err(VaultError::BadCredentials)
    ));
}

#[test]
fn mot_de_passe_faible_refuse() {
    let d = TempDir::new().unwrap();
    assert!(matches!(
        Vault::create(d.path(), &pw("password"), fast()),
        Err(VaultError::WeakPassword(_))
    ));
    assert!(!Vault::exists(d.path()));
    assert!(!d.path().join(DB_FILE).exists());
}

#[test]
fn double_creation_refusee() {
    let d = TempDir::new().unwrap();
    new_vault(d.path());
    assert!(matches!(
        Vault::create(d.path(), &pw(PW), fast()),
        Err(VaultError::AlreadyExists)
    ));
}

#[test]
fn rien_de_lisible_sur_disque() {
    let d = TempDir::new().unwrap();
    let (v, _) = Vault::create(d.path(), &pw(PW), fast()).unwrap();
    v.conn()
        .execute(
            "INSERT INTO service(domain, name) VALUES ('secret-marker.example', 'MarqueurUnique')",
            [],
        )
        .unwrap();
    drop(v);
    let raw = fs::read(d.path().join(DB_FILE)).unwrap();
    assert!(!raw.starts_with(b"SQLite format 3"));
    for needle in [&b"secret-marker"[..], b"MarqueurUnique", b"service"] {
        assert!(!raw.windows(needle.len()).any(|w| w == needle));
    }
    // L'en-tête ne contient ni le mot de passe ni de clé en clair.
    let header = fs::read_to_string(d.path().join(HEADER_FILE)).unwrap();
    assert!(!header.contains(PW));
}

#[test]
fn donnees_persistees_apres_verrouillage() {
    let d = TempDir::new().unwrap();
    let (v, _) = Vault::create(d.path(), &pw(PW), fast()).unwrap();
    v.conn()
        .execute("INSERT INTO setting(key, value) VALUES ('k', 'v')", [])
        .unwrap();
    drop(v);
    let v = Vault::unlock(d.path(), &pw(PW)).unwrap();
    let val: String = v
        .conn()
        .query_row("SELECT value FROM setting WHERE key='k'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(val, "v");
}

#[test]
fn recuperation_avec_cle() {
    let d = TempDir::new().unwrap();
    let recovery = new_vault(d.path());
    let shown = recovery.to_display();
    let parsed = RecoveryKey::parse(&shown).unwrap();
    assert!(Vault::unlock_with_recovery(d.path(), &parsed).is_ok());
    assert!(matches!(
        Vault::unlock_with_recovery(d.path(), &RecoveryKey::generate()),
        Err(VaultError::BadCredentials)
    ));
}

#[test]
fn changement_de_mot_de_passe() {
    let d = TempDir::new().unwrap();
    let (v, recovery) = Vault::create(d.path(), &pw(PW), fast()).unwrap();
    v.conn()
        .execute("INSERT INTO setting(key, value) VALUES ('k', 'v')", [])
        .unwrap();
    drop(v);
    let db_before = fs::read(d.path().join(DB_FILE)).unwrap();

    Vault::change_password(d.path(), &pw(PW), &pw(PW2), fast()).unwrap();

    // La base n'est pas rechiffrée, seule l'enveloppe change.
    assert_eq!(db_before, fs::read(d.path().join(DB_FILE)).unwrap());
    assert!(Vault::unlock(d.path(), &pw(PW)).is_err());
    assert!(Vault::unlock(d.path(), &pw(PW2)).is_ok());
    // La clé de récupération reste valide.
    assert!(Vault::unlock_with_recovery(d.path(), &recovery).is_ok());
}

#[test]
fn changement_refuse_si_ancien_incorrect_ou_nouveau_faible() {
    let d = TempDir::new().unwrap();
    new_vault(d.path());
    assert!(matches!(
        Vault::change_password(d.path(), &pw("mauvais mot de passe long"), &pw(PW2), fast()),
        Err(VaultError::BadCredentials)
    ));
    assert!(matches!(
        Vault::change_password(d.path(), &pw(PW), &pw("1234"), fast()),
        Err(VaultError::WeakPassword(_))
    ));
    assert!(Vault::unlock(d.path(), &pw(PW)).is_ok());
}

#[test]
fn reinitialisation_par_cle_de_recuperation() {
    let d = TempDir::new().unwrap();
    let recovery = new_vault(d.path());
    Vault::reset_password(d.path(), &recovery, &pw(PW2), fast()).unwrap();
    assert!(Vault::unlock(d.path(), &pw(PW2)).is_ok());
    assert!(Vault::unlock(d.path(), &pw(PW)).is_err());
}

#[test]
fn en_tete_corrompu() {
    let d = TempDir::new().unwrap();
    new_vault(d.path());
    let h = d.path().join(HEADER_FILE);

    fs::write(&h, b"pas du json").unwrap();
    assert!(matches!(
        Vault::unlock(d.path(), &pw(PW)),
        Err(VaultError::BadHeader)
    ));

    // Paramètres hostiles : refus sans tenter d'allouer des gigaoctets.
    fs::write(
        &h,
        r#"{"version":1,"kdf":{"m_kib":4000000000,"t":1,"p":1},"salt":"","wrapped_password":"","wrapped_recovery":""}"#,
    )
    .unwrap();
    assert!(matches!(
        Vault::unlock(d.path(), &pw(PW)),
        Err(VaultError::BadHeader)
    ));

    fs::remove_file(&h).unwrap();
    assert!(matches!(
        Vault::unlock(d.path(), &pw(PW)),
        Err(VaultError::NotFound)
    ));
}

#[test]
fn enveloppe_alteree_refusee() {
    let d = TempDir::new().unwrap();
    new_vault(d.path());
    let h = d.path().join(HEADER_FILE);
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&h).unwrap()).unwrap();
    let s = json["wrapped_password"].as_str().unwrap().to_owned();
    // Altère un caractère du milieu (reste du base64 valide).
    let mut chars: Vec<char> = s.chars().collect();
    chars[10] = if chars[10] == 'A' { 'B' } else { 'A' };
    json["wrapped_password"] = chars.into_iter().collect::<String>().into();
    fs::write(&h, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(Vault::unlock(d.path(), &pw(PW)).is_err());
}

#[test]
fn base_alteree_refusee() {
    let d = TempDir::new().unwrap();
    new_vault(d.path());
    let p = d.path().join(DB_FILE);
    let mut raw = fs::read(&p).unwrap();
    for b in &mut raw[..4096] {
        *b ^= 0xff;
    }
    fs::write(&p, raw).unwrap();
    assert!(Vault::unlock(d.path(), &pw(PW)).is_err());
}
