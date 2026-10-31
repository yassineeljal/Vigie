use proptest::prelude::*;

use super::*;

fn facts(id: u32, crit: Criticality) -> AccountFacts {
    AccountFacts {
        id,
        status: AccountStatus::Active,
        criticality: crit,
        mfa: vec![MfaKind::Totp],
        pwned_count: Some(0),
        strength: Some(4),
        reused: false,
        breach_after_change: false,
        dependents: vec![],
    }
}

fn hit(s: &AccountScore, f: Factor) -> Option<u32> {
    s.factors.iter().find(|h| h.factor == f).map(|h| h.points)
}

#[test]
fn les_poids_correspondent_au_plan() {
    let r = Rules::v1();
    assert_eq!(r.version, "v1");
    let p = &r.factors;
    assert_eq!(
        [
            p.pwned_password,
            p.breach_after_change,
            p.reused_password,
            p.no_mfa,
            p.pivot_without_strong_mfa,
            p.weak_password_low,
            p.weak_password_mid,
            p.weak_mfa_only,
            p.unused_active,
            p.never_checked
        ],
        [45, 40, 30, 25, 20, 20, 10, 10, 10, 5]
    );
    assert_eq!(
        [
            r.weight(Criticality::Critical),
            r.weight(Criticality::High),
            r.weight(Criticality::Medium),
            r.weight(Criticality::Low)
        ],
        [1.0, 0.75, 0.5, 0.25]
    );
}

/// Exemples du plan (§6.3) : Instagram 55 × 0,5 = 27,5 ; Gmail 45 × 1,0 = 45.
#[test]
fn exemple_du_plan() {
    let mut insta = facts(1, Criticality::Medium);
    insta.mfa.clear();
    insta.reused = true;
    let mut gmail = facts(2, Criticality::Critical);
    gmail.mfa.clear();
    gmail.dependents = vec![10, 11, 12];
    let report = compute(&[insta, gmail], &Rules::v1());
    let by = |id| report.accounts.iter().find(|s| s.account_id == id).unwrap();
    assert_eq!((by(1).likelihood, by(1).priority), (55, 27.5));
    assert_eq!((by(2).likelihood, by(2).priority), (45, 45.0));
    // Gmail passe en premier.
    assert_eq!(report.accounts[0].account_id, 2);
    // Σ(L×w)/Σw = (27,5 + 45) / 1,5
    assert!((report.global - (100.0 - 72.5 / 1.5)).abs() < 1e-9);
}

#[test]
fn chaque_facteur_individuellement() {
    let r = Rules::v1();
    let score = |f: AccountFacts| compute(&[f], &r).accounts.remove(0);

    let mut f = facts(1, Criticality::Low);
    f.pwned_count = Some(3);
    assert_eq!(hit(&score(f), Factor::PwnedPassword), Some(45));

    let mut f = facts(1, Criticality::Low);
    f.breach_after_change = true;
    assert_eq!(hit(&score(f), Factor::BreachAfterChange), Some(40));

    let mut f = facts(1, Criticality::Low);
    f.strength = Some(1);
    assert_eq!(hit(&score(f), Factor::WeakPassword), Some(20));
    let mut f = facts(1, Criticality::Low);
    f.strength = Some(2);
    assert_eq!(hit(&score(f), Factor::WeakPassword), Some(10));
    let mut f = facts(1, Criticality::Low);
    f.strength = Some(3);
    assert_eq!(hit(&score(f), Factor::WeakPassword), None);

    let mut f = facts(1, Criticality::Low);
    f.mfa = vec![MfaKind::Sms, MfaKind::Email];
    assert_eq!(hit(&score(f), Factor::WeakMfaOnly), Some(10));
    let mut f = facts(1, Criticality::Low);
    f.mfa = vec![MfaKind::Sms, MfaKind::Totp];
    assert_eq!(hit(&score(f), Factor::WeakMfaOnly), None);

    let mut f = facts(1, Criticality::Low);
    f.status = AccountStatus::Unused;
    assert_eq!(hit(&score(f), Factor::UnusedActive), Some(10));

    let mut f = facts(1, Criticality::Low);
    f.pwned_count = None;
    assert_eq!(hit(&score(f), Factor::NeverChecked), Some(5));

    // Pivot : au moins 3 dépendants ET pas de MFA forte (push seul ne compte pas comme forte).
    let mut f = facts(1, Criticality::Low);
    f.dependents = vec![2, 3, 4];
    assert_eq!(hit(&score(f.clone()), Factor::PivotWithoutStrongMfa), None);
    f.mfa = vec![MfaKind::Push];
    assert_eq!(
        hit(&score(f.clone()), Factor::PivotWithoutStrongMfa),
        Some(20)
    );
    f.dependents.pop();
    assert_eq!(hit(&score(f), Factor::PivotWithoutStrongMfa), None);
}

#[test]
fn comptes_supprimes_ignores_et_cas_vide() {
    let r = Rules::v1();
    assert_eq!(compute(&[], &r).global, 100.0);
    let mut d = facts(1, Criticality::Critical);
    d.status = AccountStatus::Deleted;
    d.mfa.clear();
    let rep = compute(&[d], &r);
    assert!(rep.accounts.is_empty());
    assert_eq!(rep.global, 100.0);
}

#[test]
fn heritage_de_criticite() {
    let mut gmail = facts(1, Criticality::Low);
    gmail.dependents = vec![2];
    let bank = facts(2, Criticality::Critical);
    let eff = effective_criticalities(&[gmail.clone(), bank]);
    assert_eq!(eff[&1], Criticality::Critical);
    // Transitif : A récupère B qui récupère C (critique).
    let mut a = facts(1, Criticality::Low);
    a.dependents = vec![2];
    let mut b = facts(2, Criticality::Low);
    b.dependents = vec![3];
    let c = facts(3, Criticality::High);
    let eff = effective_criticalities(&[a, b, c]);
    assert_eq!((eff[&1], eff[&2]), (Criticality::High, Criticality::High));
    // Un cycle ne boucle pas.
    let mut x = facts(1, Criticality::Low);
    x.dependents = vec![2];
    let mut y = facts(2, Criticality::Critical);
    y.dependents = vec![1];
    let eff = effective_criticalities(&[x, y]);
    assert_eq!(
        (eff[&1], eff[&2]),
        (Criticality::Critical, Criticality::Critical)
    );
}

#[test]
fn likelihood_plafonnee_a_100() {
    let mut f = facts(1, Criticality::Critical);
    f.pwned_count = Some(1);
    f.breach_after_change = true;
    f.reused = true;
    f.mfa.clear();
    f.strength = Some(0);
    let s = compute(&[f], &Rules::v1()).accounts.remove(0);
    assert_eq!(s.likelihood, 100);
    assert!(s.factors.iter().map(|h| h.points).sum::<u32>() > 100); // détail non plafonné (explicabilité)
}

fn arb_facts() -> impl Strategy<Value = AccountFacts> {
    (
        0u32..5,
        prop::sample::select(vec![
            Criticality::Critical,
            Criticality::High,
            Criticality::Medium,
            Criticality::Low,
        ]),
        prop::collection::vec(
            prop::sample::select(vec![
                MfaKind::Passkey,
                MfaKind::HardwareKey,
                MfaKind::Totp,
                MfaKind::Push,
                MfaKind::Sms,
                MfaKind::Email,
            ]),
            0..4,
        ),
        prop::option::of(0u32..1000),
        prop::option::of(0u8..5),
        any::<bool>(),
        any::<bool>(),
        prop::collection::vec(100u32..110, 0..6),
        prop::sample::select(vec![
            AccountStatus::Active,
            AccountStatus::Unused,
            AccountStatus::Deleted,
        ]),
    )
        .prop_map(
            |(
                id,
                criticality,
                mfa,
                pwned_count,
                strength,
                reused,
                breach_after_change,
                dependents,
                status,
            )| AccountFacts {
                id,
                status,
                criticality,
                mfa,
                pwned_count,
                strength,
                reused,
                breach_after_change,
                dependents,
            },
        )
}

proptest! {
    #[test]
    fn le_score_reste_dans_0_100(list in prop::collection::vec(arb_facts(), 0..12)) {
        let mut list = list;
        for (i, f) in list.iter_mut().enumerate() { f.id = i as u32; f.dependents.iter_mut().for_each(|d| *d %= 12); }
        let rep = compute(&list, &Rules::v1());
        prop_assert!((0.0..=100.0).contains(&rep.global), "{}", rep.global);
        for s in &rep.accounts { prop_assert!(s.likelihood <= 100); }
    }

    /// Ajouter un facteur de risque ne fait jamais baisser la probabilité.
    #[test]
    fn ajouter_un_facteur_ne_baisse_jamais_le_risque(f in arb_facts()) {
        let r = Rules::v1();
        let l = |f: &AccountFacts| compute(std::slice::from_ref(f), &r).accounts.first().map_or(0, |s| s.likelihood);
        let base = l(&f);
        let mut g = f.clone(); g.pwned_count = Some(g.pwned_count.unwrap_or(0).max(1));
        prop_assert!(l(&g) >= base || f.status == AccountStatus::Deleted);
        let mut g = f.clone(); g.breach_after_change = true;
        prop_assert!(l(&g) >= base);
        let mut g = f.clone(); g.reused = true;
        prop_assert!(l(&g) >= base);
        let mut g = f.clone(); g.strength = Some(0);
        prop_assert!(l(&g) >= base);
        let mut g = f.clone(); g.status = if f.status == AccountStatus::Deleted { f.status } else { AccountStatus::Unused };
        prop_assert!(l(&g) >= base);
    }
}
