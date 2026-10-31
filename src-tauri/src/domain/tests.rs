use secrecy::SecretString;
use tempfile::TempDir;

use super::*;
use crate::crypto::KdfParams;
use crate::vault::Vault;

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

fn input(domain: &str, label: &str) -> AccountInput {
    AccountInput {
        domain: domain.into(),
        label: label.into(),
        identity_id: None,
        username: None,
        criticality: Criticality::Medium,
        recovery_identity_id: None,
        pwd_changed_at: None,
        status: AccountStatus::Active,
        notes: None,
        tags: vec![],
        mfa: vec![],
    }
}

fn filter() -> AccountFilter {
    AccountFilter {
        search: None,
        status: None,
        criticality: None,
        no_mfa: false,
        tag: None,
        sort: SortKey::Label,
        descending: false,
    }
}

#[test]
fn crud_complet() {
    let (_d, v) = vault();
    let c = v.conn();
    let mut inp = input("https://www.github.com/login", " GitHub ");
    inp.tags = vec!["Dev".into(), "dev".into(), " travail ".into(), "".into()];
    inp.mfa = vec![
        MfaMethod {
            kind: MfaKind::Totp,
            backup_codes: true,
        },
        MfaMethod {
            kind: MfaKind::Totp,
            backup_codes: false,
        },
    ];
    inp.pwd_changed_at = Some("2023-01-10".into());
    let a = save_account(c, None, &inp).unwrap();
    assert_eq!(
        (a.domain.as_str(), a.label.as_str()),
        ("github.com", "GitHub")
    );
    assert_eq!(a.tags, vec!["dev", "travail"]);
    assert_eq!(a.mfa.len(), 1);
    assert!(a.mfa[0].backup_codes);

    inp.criticality = Criticality::Critical;
    inp.mfa.clear();
    let b = save_account(c, Some(a.id), &inp).unwrap();
    assert_eq!(b.criticality, Criticality::Critical);
    assert!(b.mfa.is_empty());
    assert_eq!(b.created_at, a.created_at);

    delete_account(c, a.id).unwrap();
    assert!(matches!(get_account(c, a.id), Err(DomainError::NotFound)));
    assert!(matches!(
        delete_account(c, a.id),
        Err(DomainError::NotFound)
    ));
    assert!(list_accounts(c, &filter()).unwrap().is_empty());
}

#[test]
fn validations() {
    let (_d, v) = vault();
    let c = v.conn();
    assert!(save_account(c, None, &input("localhost", "x")).is_err());
    assert!(save_account(c, None, &input("a.com", "   ")).is_err());
    let mut bad_date = input("a.com", "A");
    bad_date.pwd_changed_at = Some("10/01/2023".into());
    assert!(save_account(c, None, &bad_date).is_err());
    let mut bad_ref = input("a.com", "A");
    bad_ref.identity_id = Some(999);
    assert!(matches!(
        save_account(c, None, &bad_ref),
        Err(DomainError::InvalidInput(_))
    ));
    assert!(matches!(
        save_account(c, Some(42), &input("a.com", "A")),
        Err(DomainError::NotFound)
    ));
    // Aucune ligne orpheline après un échec : la transaction est annulée.
    assert!(list_accounts(c, &filter()).unwrap().is_empty());
}

#[test]
fn filtres_et_tris() {
    let (_d, v) = vault();
    let c = v.conn();
    let mut gmail = input("gmail.com", "Gmail");
    gmail.criticality = Criticality::Critical;
    gmail.tags = vec!["perso".into()];
    let mut insta = input("instagram.com", "Instagram");
    insta.criticality = Criticality::Low;
    insta.mfa = vec![MfaMethod {
        kind: MfaKind::Sms,
        backup_codes: false,
    }];
    let mut bank = input("banque.ca", "Banque 100%");
    bank.criticality = Criticality::Critical;
    bank.mfa = vec![MfaMethod {
        kind: MfaKind::Passkey,
        backup_codes: true,
    }];
    bank.status = AccountStatus::Unused;
    for i in [&gmail, &insta, &bank] {
        save_account(c, None, i).unwrap();
    }

    let labels = |f: &AccountFilter| -> Vec<String> {
        list_accounts(c, f)
            .unwrap()
            .into_iter()
            .map(|a| a.label)
            .collect()
    };
    assert_eq!(labels(&filter()), ["Banque 100%", "Gmail", "Instagram"]);

    // Démo du sprint : « critiques sans MFA ».
    let mut f = filter();
    f.criticality = Some(Criticality::Critical);
    f.no_mfa = true;
    assert_eq!(labels(&f), ["Gmail"]);

    let mut f = filter();
    f.search = Some("100%".into());
    assert_eq!(labels(&f), ["Banque 100%"]);
    f.search = Some("%".into()); // le joker LIKE est échappé
    assert_eq!(labels(&f), ["Banque 100%"]);
    f.search = Some("_".into());
    assert!(labels(&f).is_empty());

    let mut f = filter();
    f.tag = Some("PERSO".into());
    assert_eq!(labels(&f), ["Gmail"]);
    f = filter();
    f.status = Some(AccountStatus::Unused);
    assert_eq!(labels(&f), ["Banque 100%"]);

    f = filter();
    f.sort = SortKey::Criticality;
    assert_eq!(labels(&f), ["Gmail", "Banque 100%", "Instagram"]);
    f.descending = true;
    assert_eq!(labels(&f)[0], "Instagram");
    f.search = Some("x'; DROP TABLE account; --".into());
    assert!(labels(&f).is_empty());
    assert_eq!(list_tags(c).unwrap(), ["perso"]);
}

#[test]
fn identites() {
    let (_d, v) = vault();
    let c = v.conn();
    let mail = save_identity(
        c,
        None,
        &IdentityInput {
            kind: IdentityKind::Email,
            value: "  Moi@Gmail.COM ".into(),
            account_id: None,
        },
    )
    .unwrap();
    assert_eq!(mail.value, "moi@gmail.com");
    let tel = save_identity(
        c,
        None,
        &IdentityInput {
            kind: IdentityKind::Phone,
            value: "+1 (514) 555-0100".into(),
            account_id: None,
        },
    )
    .unwrap();
    assert_eq!(tel.value, "+15145550100");
    for (kind, value) in [
        (IdentityKind::Email, "pas-un-courriel"),
        (IdentityKind::Email, "a@b"),
        (IdentityKind::Phone, "abc"),
        (IdentityKind::Phone, "123"),
    ] {
        assert!(
            save_identity(
                c,
                None,
                &IdentityInput {
                    kind,
                    value: value.into(),
                    account_id: None
                }
            )
            .is_err()
        );
    }
    // Doublon refusé.
    assert!(
        save_identity(
            c,
            None,
            &IdentityInput {
                kind: IdentityKind::Email,
                value: "moi@gmail.com".into(),
                account_id: None
            }
        )
        .is_err()
    );

    // Lier l'identité au compte Gmail, l'utiliser comme connexion et récupération.
    let gmail = save_account(c, None, &input("gmail.com", "Gmail")).unwrap();
    save_identity(
        c,
        Some(mail.id),
        &IdentityInput {
            kind: IdentityKind::Email,
            value: mail.value.clone(),
            account_id: Some(gmail.id),
        },
    )
    .unwrap();
    let mut insta = input("instagram.com", "Instagram");
    insta.identity_id = Some(mail.id);
    insta.recovery_identity_id = Some(tel.id);
    let insta = save_account(c, None, &insta).unwrap();
    assert_eq!(insta.identity_id, Some(mail.id));

    // Supprimer une identité détache les comptes sans les supprimer.
    delete_identity(c, mail.id).unwrap();
    let insta = get_account(c, insta.id).unwrap();
    assert_eq!(insta.identity_id, None);
    assert_eq!(insta.recovery_identity_id, Some(tel.id));
    assert_eq!(list_identities(c).unwrap().len(), 1);
}

#[test]
fn mille_comptes_restent_rapides() {
    let (_d, v) = vault();
    let c = v.conn();
    c.execute_batch("BEGIN").unwrap();
    for i in 0..1000 {
        let mut a = input(&format!("site{i}.com"), &format!("Site {i}"));
        a.tags = vec![format!("t{}", i % 7)];
        a.mfa = vec![MfaMethod {
            kind: MfaKind::Totp,
            backup_codes: false,
        }];
        save_account(c, None, &a).unwrap();
    }
    c.execute_batch("COMMIT").unwrap();
    let start = std::time::Instant::now();
    assert_eq!(list_accounts(c, &filter()).unwrap().len(), 1000);
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}
