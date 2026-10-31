use proptest::prelude::*;
use secrecy::SecretString;
use tempfile::TempDir;

use super::*;
use crate::crypto::KdfParams;
use crate::domain::{self, AccountFilter, MfaKind, SortKey};
use crate::vault::Vault;

const BW_CSV: &[u8] = include_bytes!("../../fixtures/bitwarden.csv");
const BW_JSON: &[u8] = include_bytes!("../../fixtures/bitwarden.json");
const BW_ENC: &[u8] = include_bytes!("../../fixtures/bitwarden-encrypted.json");
const APPLE: &[u8] = include_bytes!("../../fixtures/apple.csv");
const GENERIC: &[u8] = include_bytes!("../../fixtures/generic.csv");
const CHROME: &[u8] = include_bytes!("../../fixtures/chrome.csv");
const FIREFOX: &[u8] = include_bytes!("../../fixtures/firefox.csv");

fn vault() -> (TempDir, Vault) {
    let d = TempDir::new().unwrap();
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

fn all_accounts(v: &Vault) -> Vec<domain::Account> {
    domain::list_accounts(
        v.conn(),
        &AccountFilter {
            search: None,
            status: None,
            criticality: None,
            no_mfa: false,
            tag: None,
            sort: SortKey::Label,
            descending: false,
        },
    )
    .unwrap()
}

fn session(bytes: &[u8], f: ImportFormat) -> ImportSession {
    ImportSession::build(parse(bytes, f, None).unwrap())
}

#[test]
fn detection_des_formats() {
    assert_eq!(detect(BW_CSV), Some(ImportFormat::BitwardenCsv));
    assert_eq!(detect(BW_JSON), Some(ImportFormat::BitwardenJson));
    assert_eq!(detect(APPLE), Some(ImportFormat::ApplePasswords));
    assert_eq!(detect(GENERIC), None);
    assert_eq!(
        csv_headers(GENERIC).unwrap(),
        ["Site", "Identifiant", "Secret", "Commentaire"]
    );
}

#[test]
fn bitwarden_csv() {
    let out = parse(BW_CSV, ImportFormat::BitwardenCsv, None).unwrap();
    // 5 identifiants + 1 application mobile ; la note sécurisée et la ligne vide sont ignorées.
    assert_eq!(out.entries.len(), 6);
    assert_eq!(out.skipped, 2);
    let gmail = &out.entries[0];
    assert!(gmail.has_totp);
    assert_eq!(gmail.folder.as_deref(), Some("Perso"));
    assert!(!out.entries[1].has_totp);
    let gh = out.entries.iter().find(|e| e.name == "GitHub").unwrap();
    assert_eq!(gh.url, "https://github.com/login");
    assert_eq!(gh.notes.as_deref(), Some("ligne 1\nligne 2"));
    assert_eq!(out.entries[3].folder, None);
}

#[test]
fn bitwarden_json() {
    let out = parse(BW_JSON, ImportFormat::BitwardenJson, None).unwrap();
    assert_eq!(out.entries.len(), 2);
    assert_eq!(out.skipped, 2);
    assert_eq!(out.entries[0].pwd_changed_at.as_deref(), Some("2023-01-10"));
    // Sans passwordRevisionDate : dernier historique, jamais revisionDate.
    assert_eq!(out.entries[1].pwd_changed_at.as_deref(), Some("2022-05-06"));
    assert!(out.entries[0].has_totp);
    assert_eq!(out.entries[0].folder.as_deref(), Some("Perso"));
    assert!(matches!(
        parse(BW_ENC, ImportFormat::BitwardenJson, None),
        Err(ImportError::Encrypted)
    ));
    assert!(parse(b"{ pas du json", ImportFormat::BitwardenJson, None).is_err());
}

#[test]
fn apple_et_generique() {
    let out = parse(APPLE, ImportFormat::ApplePasswords, None).unwrap();
    assert_eq!(out.entries.len(), 3);
    assert!(out.entries[0].has_totp);
    assert_eq!(out.entries[2].notes.as_deref(), Some("un, avec virgule"));

    let m = ColumnMapping {
        name: Some("Site".into()),
        url: Some("Site".into()),
        username: Some("identifiant".into()), // insensible à la casse
        password: Some("Secret".into()),
        notes: Some("Commentaire".into()),
        ..Default::default()
    };
    let out = parse(GENERIC, ImportFormat::GenericCsv, Some(&m)).unwrap();
    assert_eq!(out.entries.len(), 2);
    assert_eq!(out.entries[0].username, "bob");
    assert!(parse(GENERIC, ImportFormat::GenericCsv, None).is_err());
    let bad = ColumnMapping {
        name: Some("inexistante".into()),
        ..Default::default()
    };
    assert!(parse(GENERIC, ImportFormat::GenericCsv, Some(&bad)).is_err());
}

#[test]
fn limites() {
    let big = vec![b'a'; formats_max() + 1];
    assert!(matches!(
        parse(&big, ImportFormat::BitwardenCsv, None),
        Err(ImportError::TooLarge)
    ));
    assert!(matches!(csv_headers(&big), Err(ImportError::TooLarge)));
}
fn formats_max() -> usize {
    20 * 1024 * 1024
}

#[test]
fn reutilisation_us24() {
    let s = session(BW_CSV, ImportFormat::BitwardenCsv);
    let (_d, v) = vault();
    let p = s.preview(v.conn()).unwrap();
    let grouped: Vec<_> = p.items.iter().filter(|i| i.reuse_group.is_some()).collect();
    assert_eq!(grouped.len(), 3);
    assert!(
        grouped
            .iter()
            .all(|i| i.reuse_group == grouped[0].reuse_group)
    );
    assert_eq!((p.reuse_groups, p.reused_accounts), (1, 3));
    assert_eq!(p.items.iter().filter(|i| i.issue.is_some()).count(), 1); // androidapp://
    let forum = p.items.iter().find(|i| i.label == "Forum").unwrap();
    assert_eq!(forum.domain.as_deref(), Some("exemple.co.uk"));
}

/// Concatène tout le contenu textuel de la base déchiffrée.
fn dump_all(v: &Vault) -> String {
    let c = v.conn();
    let tables: Vec<String> = c
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut out = String::new();
    for t in tables {
        let n = c
            .prepare(&format!("SELECT * FROM \"{t}\""))
            .unwrap()
            .column_count();
        let mut stmt = c.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
        let mut rows = stmt.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            for i in 0..n {
                let val: rusqlite::types::Value = r.get(i).unwrap();
                out.push_str(&format!("{val:?}|"));
            }
        }
    }
    out
}

#[test]
fn import_sans_aucun_mot_de_passe_en_base() {
    let (_d, v) = vault();
    let s = session(BW_CSV, ImportFormat::BitwardenCsv);
    let r = commit(v.conn(), &s, DuplicatePolicy::Skip, &[]).unwrap();
    assert_eq!(
        (
            r.created,
            r.skipped_invalid,
            r.reuse_groups,
            r.reused_accounts
        ),
        (5, 1, 1, 3)
    );

    let rows: Vec<(Option<u32>, Option<u8>)> = v
        .conn()
        .prepare("SELECT reuse_group, strength FROM password_check ORDER BY account_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows.len(), 5);
    assert_eq!(rows.iter().filter(|(g, _)| g.is_some()).count(), 3);
    assert!(rows.iter().all(|(_, s)| s.is_some()));

    let dump = dump_all(&v);
    for secret in [
        "motdepasse-partage-1",
        "Tr0ub4dor",
        "gh-fake-password-9",
        "FAKEFAKEFAKE",
        "JBSWY3DPEHPK3PXP",
    ] {
        assert!(!dump.contains(secret), "{secret} trouvé dans la base");
    }
    let accounts = all_accounts(&v);
    let gmail = accounts.iter().find(|a| a.label == "Gmail").unwrap();
    assert_eq!(gmail.mfa[0].kind, MfaKind::Totp);
    assert_eq!(gmail.tags, ["perso"]);
}

#[test]
fn doublons_et_fusion() {
    let (_d, v) = vault();
    let csv = session(BW_CSV, ImportFormat::BitwardenCsv);
    commit(v.conn(), &csv, DuplicatePolicy::Skip, &[]).unwrap();

    // Même contenu : tout est un doublon.
    let p = csv.preview(v.conn()).unwrap();
    assert_eq!(
        p.items.iter().filter(|i| i.duplicate_of.is_some()).count(),
        5
    );
    let r = commit(v.conn(), &csv, DuplicatePolicy::Skip, &[]).unwrap();
    assert_eq!((r.created, r.skipped_duplicates), (0, 5));

    // Le JSON apporte les dates et le TOTP d'Instagram/Gmail : fusion.
    let json = session(BW_JSON, ImportFormat::BitwardenJson);
    let r = commit(v.conn(), &json, DuplicatePolicy::Merge, &[]).unwrap();
    // Gmail (google.com) : accounts.google.com et mail.google.com → même domaine, même identifiant.
    assert_eq!((r.created, r.merged), (0, 2));
    let accounts = all_accounts(&v);
    assert_eq!(accounts.len(), 5);
    let insta = accounts.iter().find(|a| a.label == "Instagram").unwrap();
    assert_eq!(insta.pwd_changed_at.as_deref(), Some("2022-05-06"));

    let r = commit(v.conn(), &json, DuplicatePolicy::KeepBoth, &[]).unwrap();
    assert_eq!(r.created, 2);
    assert_eq!(all_accounts(&v).len(), 7);
}

#[test]
fn exclusions_et_groupes_recalcules() {
    let (_d, v) = vault();
    let s = session(BW_CSV, ImportFormat::BitwardenCsv);
    // On exclut Facebook (2) et Forum (3) : plus de réutilisation.
    let r = commit(v.conn(), &s, DuplicatePolicy::Skip, &[2, 3]).unwrap();
    assert_eq!((r.created, r.excluded, r.reuse_groups), (3, 2, 0));
}

#[test]
fn groupes_distincts_entre_imports() {
    let (_d, v) = vault();
    commit(
        v.conn(),
        &session(APPLE, ImportFormat::ApplePasswords),
        DuplicatePolicy::Skip,
        &[],
    )
    .unwrap();
    let m = ColumnMapping {
        name: Some("Site".into()),
        url: Some("Site".into()),
        username: Some("Identifiant".into()),
        password: Some("Secret".into()),
        ..Default::default()
    };
    let g = ImportSession::build(parse(GENERIC, ImportFormat::GenericCsv, Some(&m)).unwrap());
    commit(v.conn(), &g, DuplicatePolicy::Skip, &[]).unwrap();
    let groups: Vec<u32> = v
        .conn()
        .prepare("SELECT DISTINCT reuse_group FROM password_check WHERE reuse_group IS NOT NULL")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        groups.len(),
        2,
        "deux imports ne partagent jamais un identifiant de groupe"
    );
}

#[test]
fn mille_entrees_sous_dix_secondes() {
    let mut csv = String::from("Title,URL,Username,Password,Notes,OTPAuth\n");
    for i in 0..1000 {
        csv.push_str(&format!(
            "Site {i},https://site{i}.com,user{i},fake-password-{}-xyz,,\n",
            i % 50
        ));
    }
    let (_d, v) = vault();
    let start = std::time::Instant::now();
    let s = session(csv.as_bytes(), ImportFormat::ApplePasswords);
    let r = commit(v.conn(), &s, DuplicatePolicy::Skip, &[]).unwrap();
    assert_eq!(r.created, 1000);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        start.elapsed()
    );
}

proptest! {
    #[test]
    fn les_analyseurs_ne_paniquent_jamais(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let m = ColumnMapping { name: Some("a".into()), url: Some("b".into()), ..Default::default() };
        for f in [ImportFormat::BitwardenCsv, ImportFormat::BitwardenJson, ImportFormat::ApplePasswords, ImportFormat::GenericCsv, ImportFormat::BrowserCsv] {
            let _ = parse(&bytes, f, Some(&m));
        }
        let _ = detect(&bytes);
        let _ = csv_headers(&bytes);
    }

    #[test]
    fn csv_textuel_arbitraire(s in "[ -~\\n,;\"]{0,400}") {
        let _ = parse(s.as_bytes(), ImportFormat::BitwardenCsv, None);
        let _ = parse(s.as_bytes(), ImportFormat::ApplePasswords, None);
        if let Ok(out) = parse(s.as_bytes(), ImportFormat::ApplePasswords, None) {
            let _ = ImportSession::build(out);
        }
    }
}

#[test]
fn us25_comptes_de_fuite_conserves_sans_mot_de_passe() {
    let (_d, v) = vault();
    let mut s = session(APPLE, ImportFormat::ApplePasswords);
    let hashes = s.password_hashes();
    assert_eq!(hashes.len(), 3);
    // Le hash du premier mot de passe est un SHA-1 (préfixe de 5 + suffixe de 35 caractères).
    let (p, suffix) = hashes[0].as_ref().unwrap();
    assert_eq!((p.len(), suffix.len()), (5, 35));
    s.set_breach_counts(&[Some(0), Some(42), None]);
    commit(v.conn(), &s, DuplicatePolicy::Skip, &[]).unwrap();
    let counts: Vec<Option<u32>> = v
        .conn()
        .prepare("SELECT breach_count FROM password_check ORDER BY account_id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(counts, [Some(0), Some(42), None]);
    let preview = s.preview(v.conn()).unwrap();
    assert_eq!(preview.items[1].breach_count, Some(42));
}

#[test]
fn navigateurs_chrome_et_firefox() {
    assert_eq!(detect(CHROME), Some(ImportFormat::BrowserCsv));
    assert_eq!(detect(FIREFOX), Some(ImportFormat::BrowserCsv));
    let c = parse(CHROME, ImportFormat::BrowserCsv, None).unwrap();
    assert_eq!(c.entries.len(), 2);
    assert_eq!(
        c.entries[1].notes.as_deref(),
        Some("une note, avec virgule")
    );
    assert!(c.entries.iter().all(|e| e.pwd_changed_at.is_none()));

    let f = parse(FIREFOX, ImportFormat::BrowserCsv, None).unwrap();
    assert_eq!(f.entries.len(), 2);
    // 1673308800000 ms = 2023-01-10 ; 0 (inconnu) ne doit pas devenir une fausse date de 1970.
    assert_eq!(f.entries[0].pwd_changed_at.as_deref(), Some("2023-01-10"));
    assert_eq!(f.entries[1].pwd_changed_at, None);
}

#[test]
fn conversion_des_dates_firefox() {
    assert_eq!(
        formats::epoch_ms_to_date(1_673_308_800_000).as_deref(),
        Some("2023-01-10")
    );
    assert_eq!(
        formats::epoch_ms_to_date(951_782_400_000).as_deref(),
        Some("2000-02-29")
    );
    assert_eq!(formats::epoch_ms_to_date(-1), None);
    assert_eq!(formats::epoch_ms_to_date(i64::MAX), None);
}
