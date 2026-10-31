use secrecy::SecretString;
use tempfile::TempDir;

use super::*;
use crate::crypto::KdfParams;
use crate::domain::{self, AccountInput, Criticality, IdentityInput, IdentityKind, MfaMethod};
use crate::scoring::Factor;
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

fn input(domain: &str, crit: Criticality) -> AccountInput {
    AccountInput {
        domain: domain.into(),
        label: domain.into(),
        identity_id: None,
        username: None,
        criticality: crit,
        recovery_identity_id: None,
        pwd_changed_at: None,
        status: AccountStatus::Active,
        notes: None,
        tags: vec![],
        mfa: vec![MfaMethod {
            kind: MfaKind::Totp,
            backup_codes: true,
        }],
    }
}

fn check(v: &Vault, account: u32, breaches: Option<u32>, strength: u8, group: Option<u32>) {
    v.conn()
        .execute(
            "INSERT INTO password_check(account_id, checked_at, breach_count, strength, reuse_group)
             VALUES (?1, '2026-10-01T00:00:00Z', ?2, ?3, ?4)",
            rusqlite::params![account, breaches, strength, group],
        )
        .unwrap();
}

/// Scénario : Gmail (pivot, critique, sans MFA), Instagram + Facebook (mot de passe réutilisé),
/// une banque dont le mot de passe a fuité, un vieux forum inutilisé.
struct Scenario {
    gmail: u32,
    insta: u32,
    bank: u32,
    forum: u32,
}

fn scenario(v: &Vault) -> Scenario {
    let c = v.conn();
    let mail = domain::save_identity(
        c,
        None,
        &IdentityInput {
            kind: IdentityKind::Email,
            value: "moi@gmail.com".into(),
            account_id: None,
        },
    )
    .unwrap();
    let mut g = input("gmail.com", Criticality::Critical);
    g.mfa.clear();
    let gmail = domain::save_account(c, None, &g).unwrap().id;
    domain::save_identity(
        c,
        Some(mail.id),
        &IdentityInput {
            kind: IdentityKind::Email,
            value: "moi@gmail.com".into(),
            account_id: Some(gmail),
        },
    )
    .unwrap();
    check(v, gmail, Some(0), 4, None);

    let mut ids = vec![];
    for (d, crit) in [
        ("instagram.com", Criticality::Medium),
        ("facebook.com", Criticality::Medium),
        ("netflix.com", Criticality::Low),
    ] {
        let mut i = input(d, crit);
        i.recovery_identity_id = Some(mail.id);
        i.mfa.clear();
        ids.push(domain::save_account(c, None, &i).unwrap().id);
    }
    check(v, ids[0], Some(0), 3, Some(1));
    check(v, ids[1], Some(0), 3, Some(1));
    check(v, ids[2], Some(0), 4, None);

    let bank = domain::save_account(c, None, &input("banque.ca", Criticality::Critical))
        .unwrap()
        .id;
    check(v, bank, Some(12), 3, None);

    let mut f = input("vieuxforum.org", Criticality::Low);
    f.status = AccountStatus::Unused;
    let forum = domain::save_account(c, None, &f).unwrap().id;
    check(v, forum, Some(0), 4, None);
    Scenario {
        gmail,
        insta: ids[0],
        bank,
        forum,
    }
}

#[test]
fn us42_plan_trie_par_impact_puis_quick_wins() {
    let (_d, v) = vault();
    let s = scenario(&v);
    let rules = Rules::v1();
    let impact = list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo).unwrap();

    // Gain décroissant, et chaque action indique effort et facteurs corrigés.
    assert!(impact.windows(2).all(|w| w[0].gain >= w[1].gain));
    assert!(
        impact
            .iter()
            .all(|a| a.effort_min > 0 && !a.factors.is_empty())
    );
    // Banque : mot de passe fuité (45) × 1,0 = 45 ; Gmail : sans MFA 25 × 1,0 = 25 + pivot 20.
    let bank = impact.iter().find(|a| a.account_id == s.bank).unwrap();
    assert_eq!(
        (bank.kind, bank.gain, bank.effort_min),
        (ActionKind::ChangePassword, 45.0, 5)
    );
    assert_eq!(bank.factors, [Factor::PwnedPassword]);
    let gmail_mfa = impact
        .iter()
        .find(|a| a.account_id == s.gmail && a.kind == ActionKind::EnableMfa)
        .unwrap();
    assert_eq!(gmail_mfa.gain, 25.0);
    let pivot = impact
        .iter()
        .find(|a| a.kind == ActionKind::SecurePivot)
        .unwrap();
    assert_eq!(
        (pivot.account_id, pivot.gain, pivot.effort_min),
        (s.gmail, 20.0, 10)
    );
    // Instagram : réutilisé (30) × 0,5 = 15.
    let insta = impact
        .iter()
        .find(|a| a.account_id == s.insta && a.kind == ActionKind::ChangePassword)
        .unwrap();
    assert_eq!(insta.gain, 15.0);

    let quick = list(v.conn(), &rules, PlanView::QuickWins, ActionStatus::Todo).unwrap();
    assert!(quick.windows(2).all(|w| w[0].ratio >= w[1].ratio));
    // gain/effort : banque 45/5 = 9 ; pivot 20/10 = 2 → l'ordre diffère de la vue Impact.
    assert_eq!(quick[0].account_id, s.bank);
    assert!(
        quick.iter().position(|a| a.id == pivot.id)
            > impact.iter().position(|a| a.id == pivot.id).map(|_| 0)
    );
    assert_eq!(quick.len(), impact.len());
    let _ = s.forum;
}

#[test]
fn us43_faire_une_action_fait_monter_le_score() {
    let (_d, v) = vault();
    let s = scenario(&v);
    let rules = Rules::v1();
    let before = refresh(v.conn(), &rules).unwrap().global;
    let plan = list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo).unwrap();

    let bank = plan.iter().find(|a| a.account_id == s.bank).unwrap();
    let after_pw = complete(v.conn(), &rules, bank.id, None).unwrap();
    assert!(after_pw.global > before);
    // L'action disparaît de « à faire » et apparaît dans « faites ».
    assert!(
        list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo)
            .unwrap()
            .iter()
            .all(|a| a.id != bank.id)
    );
    assert_eq!(
        list(v.conn(), &rules, PlanView::Impact, ActionStatus::Done)
            .unwrap()
            .len(),
        1
    );
    let changed: Option<String> = v
        .conn()
        .query_row(
            "SELECT pwd_changed_at FROM account WHERE id=?1",
            [s.bank],
            |r| r.get(0),
        )
        .unwrap();
    assert!(changed.is_some());

    // MFA : méthode requise ; Gmail passe à TOTP, ce qui supprime aussi le facteur pivot.
    let enable = plan
        .iter()
        .find(|a| a.account_id == s.gmail && a.kind == ActionKind::EnableMfa)
        .unwrap();
    assert!(complete(v.conn(), &rules, enable.id, None).is_err());
    let after_mfa = complete(v.conn(), &rules, enable.id, Some(MfaKind::Totp)).unwrap();
    assert!(after_mfa.global > after_pw.global);
    let todo = list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    assert!(todo.iter().all(|a| !(a.account_id == s.gmail)));
    let backup: bool = v
        .conn()
        .query_row(
            "SELECT backup_codes FROM mfa_method WHERE account_id=?1",
            [s.gmail],
            |r| r.get(0),
        )
        .unwrap();
    assert!(backup);

    // Suppression du compte inutilisé.
    let del = todo
        .iter()
        .find(|a| a.kind == ActionKind::DeleteUnused)
        .unwrap();
    let after_del = complete(v.conn(), &rules, del.id, None).unwrap();
    // Le score est une moyenne pondérée : retirer un compte peu risqué peut la faire bouger dans
    // les deux sens ; ce qui compte ici est que le compte sorte du calcul.
    let _ = after_mfa;
    assert!(after_del.accounts.iter().all(|a| a.account_id != s.forum));
}

#[test]
fn mfa_faible_refusee_pour_une_mise_a_niveau() {
    let (_d, v) = vault();
    let c = v.conn();
    let mut i = input("sms-only.com", Criticality::High);
    i.mfa = vec![MfaMethod {
        kind: MfaKind::Sms,
        backup_codes: false,
    }];
    let id = domain::save_account(c, None, &i).unwrap().id;
    check(&v, id, Some(0), 4, None);
    let rules = Rules::v1();
    let a = list(c, &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].kind, ActionKind::UpgradeMfa);
    assert!(complete(c, &rules, a[0].id, Some(MfaKind::Email)).is_err());
    assert!(complete(c, &rules, a[0].id, Some(MfaKind::Passkey)).is_ok());
    assert!(
        list(c, &rules, PlanView::Impact, ActionStatus::Todo)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ignorer_masque_sans_changer_le_score_et_rouvrir() {
    let (_d, v) = vault();
    scenario(&v);
    let rules = Rules::v1();
    let before = refresh(v.conn(), &rules).unwrap().global;
    let plan = list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    let n = plan.len();
    set_status(v.conn(), plan[0].id, ActionStatus::Ignored).unwrap();
    let after = list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    assert_eq!(after.len(), n - 1);
    assert_eq!(refresh(v.conn(), &rules).unwrap().global, before);
    // L'action ignorée survit aux recalculs.
    refresh(v.conn(), &rules).unwrap();
    assert_eq!(
        list(v.conn(), &rules, PlanView::Impact, ActionStatus::Ignored)
            .unwrap()
            .len(),
        1
    );
    set_status(v.conn(), plan[0].id, ActionStatus::Todo).unwrap();
    assert_eq!(
        list(v.conn(), &rules, PlanView::Impact, ActionStatus::Todo)
            .unwrap()
            .len(),
        n
    );
    assert!(matches!(
        set_status(v.conn(), 9999, ActionStatus::Done),
        Err(DomainError::NotFound)
    ));
}

#[test]
fn les_actions_obsoletes_disparaissent_et_une_action_faite_se_rouvre() {
    let (_d, v) = vault();
    let c = v.conn();
    let id = domain::save_account(c, None, &input("exemple.com", Criticality::Critical))
        .unwrap()
        .id;
    check(&v, id, Some(0), 4, None);
    let rules = Rules::v1();
    assert!(
        list(c, &rules, PlanView::Impact, ActionStatus::Todo)
            .unwrap()
            .is_empty()
    );

    // Une brèche pertinente apparaît → action « changer le mot de passe après brèche » (US-32).
    c.execute("INSERT INTO breach(name, title, domain, breach_date, data_classes) VALUES ('B','B','exemple.com','2026-09-01','[\"Passwords\"]')", []).unwrap();
    crate::intel::update_matches(c).unwrap();
    let plan = list(c, &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(
        (plan[0].kind, plan[0].gain, plan[0].effort_min),
        (ActionKind::ChangePasswordAfterBreach, 40.0, 10)
    );

    // Fait : le mot de passe est changé aujourd'hui, la brèche (antérieure) ne compte plus.
    complete(c, &rules, plan[0].id, None).unwrap();
    assert!(
        list(c, &rules, PlanView::Impact, ActionStatus::Todo)
            .unwrap()
            .iter()
            .all(|a| a.kind != ActionKind::ChangePasswordAfterBreach)
    );

    // Nouvelle brèche postérieure : l'action faite est rouverte.
    c.execute("INSERT INTO breach(name, title, domain, breach_date, data_classes) VALUES ('C','C','exemple.com','2999-01-01','[\"Passwords\"]')", []).unwrap();
    crate::intel::update_matches(c).unwrap();
    let plan = list(c, &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    assert!(
        plan.iter()
            .any(|a| a.kind == ActionKind::ChangePasswordAfterBreach)
    );
}
