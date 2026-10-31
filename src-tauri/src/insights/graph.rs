//! Graphe de dépendances de récupération (US-52) : qui récupère quoi.
//! Les arêtes vont du compte dépendant vers le compte (ou l'identité) dont il dépend.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;
use serde::Serialize;
use specta::Type;

use crate::domain::{AccountStatus, Criticality, DomainError, MfaKind};
use crate::scoring::{self, Rules};

type Result<T> = std::result::Result<T, DomainError>;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Account,
    /// Courriel ou téléphone qui n'est lié à aucun compte de l'inventaire.
    UnlinkedIdentity,
}

#[derive(Debug, Serialize, Type)]
pub struct GraphNode {
    /// `a<id>` pour un compte, `i<id>` pour une identité non liée.
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub criticality: Option<Criticality>,
    /// Nombre de comptes qui dépendent de ce nœud.
    pub dependents: u32,
    /// Compte pivot : récupère au moins 3 comptes.
    pub pivot: bool,
    /// Point de défaillance unique : dépendants et aucune MFA forte (ou identité non protégée).
    pub single_point_of_failure: bool,
    /// Niveau de profondeur pour la disposition (0 = ne dépend de personne).
    pub level: u32,
}

#[derive(Debug, Serialize, Type)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    /// `recovery` (identité de récupération) ou `login` (identité de connexion, à défaut).
    pub via: String,
}

#[derive(Debug, Serialize, Type)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn recovery_graph(conn: &Connection, rules: &Rules) -> Result<GraphData> {
    let facts: HashMap<u32, scoring::AccountFacts> = scoring::load_facts(conn)?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let eff = scoring::effective_criticalities(&facts.values().cloned().collect::<Vec<_>>());

    struct Acc {
        id: u32,
        label: String,
        status: AccountStatus,
        login: Option<u32>,
        recovery: Option<u32>,
    }
    let accounts: Vec<Acc> = conn
        .prepare("SELECT id, label, status, identity_id, recovery_identity_id FROM account")?
        .query_map([], |r| {
            Ok(Acc {
                id: r.get(0)?,
                label: r.get(1)?,
                status: AccountStatus::parse(&r.get::<_, String>(2)?)
                    .ok_or(rusqlite::Error::InvalidQuery)?,
                login: r.get(3)?,
                recovery: r.get(4)?,
            })
        })?
        .collect::<std::result::Result<_, _>>()?;
    let idents: HashMap<u32, (String, Option<u32>)> = conn
        .prepare("SELECT id, value, account_id FROM identity")?
        .query_map([], |r| {
            Ok((
                r.get::<_, u32>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, Option<u32>>(2)?),
            ))
        })?
        .collect::<std::result::Result<_, _>>()?;

    let live: HashSet<u32> = accounts
        .iter()
        .filter(|a| a.status != AccountStatus::Deleted)
        .map(|a| a.id)
        .collect();
    let mut edges = Vec::new();
    let mut unlinked: HashSet<u32> = HashSet::new();
    for a in accounts.iter().filter(|a| live.contains(&a.id)) {
        let (ident, via) = match (a.recovery, a.login) {
            (Some(i), _) => (i, "recovery"),
            (None, Some(i)) => (i, "login"),
            _ => continue,
        };
        let Some((_, owner)) = idents.get(&ident) else {
            continue;
        };
        match owner {
            Some(p) if *p != a.id && live.contains(p) => edges.push(GraphEdge {
                from: format!("a{}", a.id),
                to: format!("a{p}"),
                via: via.into(),
            }),
            Some(_) => {}
            None => {
                unlinked.insert(ident);
                edges.push(GraphEdge {
                    from: format!("a{}", a.id),
                    to: format!("i{ident}"),
                    via: via.into(),
                });
            }
        }
    }

    // Nœuds : comptes touchés par au moins une arête + identités non liées.
    let mut involved: HashSet<String> = HashSet::new();
    let mut dependents: HashMap<String, u32> = HashMap::new();
    for e in &edges {
        involved.insert(e.from.clone());
        involved.insert(e.to.clone());
        *dependents.entry(e.to.clone()).or_default() += 1;
    }
    let levels = levels(&edges);

    let mut nodes = Vec::new();
    for a in accounts.iter().filter(|a| live.contains(&a.id)) {
        let id = format!("a{}", a.id);
        if !involved.contains(&id) {
            continue;
        }
        let f = &facts[&a.id];
        let deps = dependents.get(&id).copied().unwrap_or(0);
        let strong = f
            .mfa
            .iter()
            .any(|k| matches!(k, MfaKind::Passkey | MfaKind::HardwareKey | MfaKind::Totp));
        nodes.push(GraphNode {
            kind: NodeKind::Account,
            label: a.label.clone(),
            criticality: Some(eff[&a.id]),
            dependents: deps,
            pivot: deps >= rules.pivot.min_dependents,
            single_point_of_failure: deps >= 1 && !strong,
            level: levels.get(&id).copied().unwrap_or(0),
            id,
        });
    }
    for i in unlinked {
        let id = format!("i{i}");
        let deps = dependents.get(&id).copied().unwrap_or(0);
        nodes.push(GraphNode {
            kind: NodeKind::UnlinkedIdentity,
            label: idents[&i].0.clone(),
            criticality: None,
            dependents: deps,
            pivot: deps >= rules.pivot.min_dependents,
            // Une identité hors inventaire ne peut pas être sécurisée depuis Vigie.
            single_point_of_failure: deps >= 2,
            level: levels.get(&id).copied().unwrap_or(0),
            id,
        });
    }
    nodes.sort_by(|a, b| {
        a.level
            .cmp(&b.level)
            .then(b.dependents.cmp(&a.dependents))
            .then(a.label.cmp(&b.label))
    });
    Ok(GraphData { nodes, edges })
}

/// Niveau = longueur du plus long chemin depuis un nœud qui ne dépend de rien. Borné pour les cycles.
fn levels(edges: &[GraphEdge]) -> HashMap<String, u32> {
    let mut level: HashMap<String, u32> = HashMap::new();
    for e in edges {
        level.entry(e.from.clone()).or_insert(0);
        level.entry(e.to.clone()).or_insert(0);
    }
    for _ in 0..level.len() {
        let mut changed = false;
        for e in edges {
            let from = level[&e.from];
            if level[&e.to] < from + 1 && from < level.len() as u32 {
                level.insert(e.to.clone(), from + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    level
}
