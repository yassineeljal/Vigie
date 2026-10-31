use secrecy::SecretString;
use tempfile::TempDir;

use super::*;
use crate::crypto::KdfParams;
use crate::domain::{
    self, AccountInput, AccountStatus, IdentityInput, IdentityKind, MfaKind, MfaMethod,
};
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

fn account(
    c: &Connection,
    domain: &str,
    crit: Criticality,
    ident: Option<u32>,
    mfa: &[MfaKind],
) -> u32 {
    domain::save_account(
        c,
        None,
        &AccountInput {
            domain: domain.into(),
            label: domain.into(),
            identity_id: None,
            username: None,
            criticality: crit,
            recovery_identity_id: ident,
            pwd_changed_at: None,
            status: AccountStatus::Active,
            notes: None,
            tags: vec![],
            mfa: mfa
                .iter()
                .map(|k| MfaMethod {
                    kind: *k,
                    backup_codes: true,
                })
                .collect(),
        },
    )
    .unwrap()
    .id
}

fn identity(c: &Connection, value: &str, owner: Option<u32>) -> u32 {
    domain::save_identity(
        c,
        None,
        &IdentityInput {
            kind: IdentityKind::Email,
            value: value.into(),
            account_id: owner,
        },
    )
    .unwrap()
    .id
}

#[test]
fn historique_un_point_par_jour() {
    let (_d, v) = vault();
    let c = v.conn();
    let rules = Rules::v1();
    account(c, "a.com", Criticality::High, None, &[]);
    let d1 = dashboard(c, &rules).unwrap();
    assert_eq!(d1.history.len(), 1);
    // Une action faite dans la journée met à jour le point du jour au lieu d'en ajouter un.
    let plan = actions::list(c, &rules, PlanView::Impact, ActionStatus::Todo).unwrap();
    actions::complete(c, &rules, plan[0].id, Some(MfaKind::Totp)).unwrap();
    let d2 = dashboard(c, &rules).unwrap();
    assert_eq!(d2.history.len(), 1);
    assert!(d2.score > d1.score);
    assert!((d2.history[0].score - d2.score).abs() < 1e-9);

    // Un point plus ancien permet de calculer la variation.
    c.execute("UPDATE score_snapshot SET taken_at = strftime('%Y-%m-%dT%H:%M:%SZ','now','-10 days'), global_score = 40", []).unwrap();
    let d3 = dashboard(c, &rules).unwrap();
    assert_eq!(d3.history.len(), 2);
    assert!((d3.delta_30d.unwrap() - (d3.score - 40.0)).abs() < 1e-9);
}

#[test]
fn tableau_de_bord_agrege() {
    let (_d, v) = vault();
    let c = v.conn();
    let rules = Rules::v1();
    assert_eq!(dashboard(c, &rules).unwrap().score, 100.0);
    account(c, "gmail.com", Criticality::Critical, None, &[]);
    account(c, "tel.com", Criticality::Low, None, &[MfaKind::Sms]);
    account(c, "ok.com", Criticality::Medium, None, &[MfaKind::Passkey]);
    let d = dashboard(c, &rules).unwrap();
    assert_eq!(d.accounts_total, 3);
    assert_eq!(d.without_mfa, 1);
    assert_eq!(d.critical_without_strong_mfa, 1);
    assert_eq!(d.by_criticality.iter().map(|x| x.count).sum::<u32>(), 3);
    assert_eq!(d.risk_buckets.iter().map(|x| x.count).sum::<u32>(), 3);
    assert!(d.top_actions.len() <= 5 && d.pending_actions >= 2);
    assert!(d.potential_gain > 0.0);
    assert!(
        d.top_factors
            .windows(2)
            .all(|w| w[0].accounts >= w[1].accounts)
    );
}

#[test]
fn graphe_pivot_et_point_de_defaillance() {
    let (_d, v) = vault();
    let c = v.conn();
    let rules = Rules::v1();
    let gmail = account(c, "gmail.com", Criticality::Critical, None, &[]);
    let mail = identity(c, "moi@gmail.com", Some(gmail));
    let orphan = identity(c, "vieux@hotmail.test", None);
    let mut ids = vec![];
    for d in ["a.com", "b.com", "c.com"] {
        ids.push(account(
            c,
            d,
            Criticality::Low,
            Some(mail),
            &[MfaKind::Totp],
        ));
    }
    let x = account(c, "x.com", Criticality::Low, Some(orphan), &[]);
    let y = account(c, "y.com", Criticality::Low, Some(orphan), &[]);
    account(c, "isole.com", Criticality::Low, None, &[]); // aucune dépendance : absent du graphe

    let g = recovery_graph(c, &rules).unwrap();
    let node = |id: &str| g.nodes.iter().find(|n| n.id == id).unwrap();
    let gm = node(&format!("a{gmail}"));
    assert_eq!(
        (gm.dependents, gm.pivot, gm.single_point_of_failure),
        (3, true, true)
    );
    // Héritage : Gmail récupère des comptes de criticité faible mais reste critique.
    assert_eq!(gm.criticality, Some(Criticality::Critical));
    assert_eq!(gm.level, 1);
    assert!(g.nodes.iter().all(|n| n.label != "isole.com"));

    let orph = node(&format!("i{orphan}"));
    assert_eq!((orph.dependents, orph.single_point_of_failure), (2, true));
    assert_eq!(g.edges.len(), 5);
    assert!(g.edges.iter().all(|e| e.via == "recovery"));
    assert!(
        g.edges
            .iter()
            .any(|e| e.from == format!("a{x}") && e.to == format!("i{orphan}"))
    );
    let _ = (ids, y);
}

#[test]
fn graphe_robuste_aux_cycles_et_comptes_supprimes() {
    let (_d, v) = vault();
    let c = v.conn();
    let rules = Rules::v1();
    let a = account(c, "a.com", Criticality::Low, None, &[]);
    let b = account(c, "b.com", Criticality::Low, None, &[]);
    let ia = identity(c, "a@a.test", Some(a));
    let ib = identity(c, "b@b.test", Some(b));
    for (acc, ident) in [(a, ib), (b, ia)] {
        let mut input = AccountInput::from(&domain::get_account(c, acc).unwrap());
        input.recovery_identity_id = Some(ident);
        domain::save_account(c, Some(acc), &input).unwrap();
    }
    let g = recovery_graph(c, &rules).unwrap();
    assert_eq!(g.nodes.len(), 2);
    assert!(g.nodes.iter().all(|n| n.level < 10));
    // Compte supprimé : plus de nœud ni d'arête.
    c.execute("UPDATE account SET status='deleted' WHERE id=?1", [b])
        .unwrap();
    let g = recovery_graph(c, &rules).unwrap();
    assert!(g.nodes.is_empty() && g.edges.is_empty());
}
