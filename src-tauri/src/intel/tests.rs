use secrecy::SecretString;
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use super::*;
use crate::crypto::KdfParams;
use crate::domain::{self, AccountInput, AccountStatus, Criticality};
use crate::vault::Vault;

fn pw(s: &str) -> SecretString {
    SecretString::from(s)
}

fn vault() -> (TempDir, Vault) {
    let d = TempDir::new().unwrap();
    let (v, _) = Vault::create(
        d.path(),
        &pw("correct horse battery staple"),
        KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
        },
    )
    .unwrap();
    (d, v)
}

fn account(v: &Vault, domain: &str, changed: Option<&str>, identity: Option<u32>) -> u32 {
    domain::save_account(
        v.conn(),
        None,
        &AccountInput {
            domain: domain.into(),
            label: domain.into(),
            identity_id: identity,
            username: None,
            criticality: Criticality::Medium,
            recovery_identity_id: None,
            pwd_changed_at: changed.map(Into::into),
            status: AccountStatus::Active,
            notes: None,
            tags: vec![],
            mfa: vec![],
        },
    )
    .unwrap()
    .id
}

fn breach(name: &str, domain: &str, date: &str, classes: &[&str]) -> RawBreach {
    serde_json::from_value(serde_json::json!({
        "Name": name, "Title": name, "Domain": domain, "BreachDate": date,
        "AddedDate": "2024-06-01T00:00:00Z", "DataClasses": classes, "IsVerified": true
    }))
    .unwrap()
}

// ---------- Pwned Passwords ----------

#[test]
fn prefixe_sha1_connu() {
    let (prefix, suffix) = sha1_split(&pw("password"));
    assert_eq!(prefix, "5BAA6");
    assert_eq!(suffix.as_str(), "1E4C9B93F3F0682250B6CF8331B7EE68FD8");
}

fn range_body(extra: &str) -> String {
    // Lignes de padding (compte 0) mêlées aux vraies.
    format!(
        "0018A45C4D1DEF81644B54AB7F969B88D65:0\n{extra}\n00D4F6E8FA6EECAD2A3AA415EEC418D38EC:2\nFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF:0\n"
    )
}

#[tokio::test]
async fn pwned_trouve_et_envoie_seulement_le_prefixe() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/range/5BAA6"))
        .and(header("Add-Padding", "true"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(range_body("1E4C9B93F3F0682250B6CF8331B7EE68FD8:9545824")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    assert_eq!(
        pwned_count(&client, &pw("password")).await.unwrap(),
        9_545_824
    );

    // Ni le mot de passe, ni son suffixe ne figurent dans la requête.
    let reqs = server.received_requests().await.unwrap();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].url.path(), "/range/5BAA6");
    assert_eq!(reqs[0].url.query(), None);
    let dump = format!("{:?}{:?}", reqs[0].url, reqs[0].headers);
    assert!(!dump.contains("1E4C9B93"));
}

#[tokio::test]
async fn pwned_absent_ou_padding_donne_zero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(range_body(
            // Le suffixe est présent mais avec un compte 0 : c'est du padding.
            "1E4C9B93F3F0682250B6CF8331B7EE68FD8:0",
        )))
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    assert_eq!(pwned_count(&client, &pw("password")).await.unwrap(), 0);
}

#[tokio::test]
async fn limite_de_debit_puis_succes() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(range_body("1E4C9B93F3F0682250B6CF8331B7EE68FD8:7")),
        )
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    assert_eq!(pwned_count(&client, &pw("password")).await.unwrap(), 7);
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn erreurs_serveur_abandonnent_apres_quatre_essais() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(503))
        .expect(4)
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    assert!(matches!(
        pwned_count(&client, &pw("x")).await,
        Err(IntelError::Http(503))
    ));

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429))
        .expect(4)
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    assert!(matches!(
        pwned_count(&client, &pw("x")).await,
        Err(IntelError::RateLimited)
    ));
}

#[tokio::test]
async fn delai_depasse_et_hors_ligne() {
    // Port fermé : connexion refusée → Offline.
    let client = IntelClient::for_mock("http://127.0.0.1:9");
    assert!(matches!(
        pwned_count(&client, &pw("x")).await,
        Err(IntelError::Offline)
    ));
}

#[tokio::test]
async fn redirections_et_hotes_non_autorises_refuses() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(302).insert_header("Location", "http://evil.example/"))
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    assert!(matches!(
        pwned_count(&client, &pw("x")).await,
        Err(IntelError::Http(302))
    ));
    assert!(matches!(
        client.get("http://evil.example/x", &[]).await,
        Err(IntelError::Forbidden)
    ));
    // En production : HTTPS et hôtes de la liste blanche uniquement.
    let prod = IntelClient::production();
    for url in [
        "http://api.pwnedpasswords.com/range/AAAAA",
        "https://example.com/",
        "https://api.pwnedpasswords.com.evil.test/",
    ] {
        assert!(
            matches!(prod.get(url, &[]).await, Err(IntelError::Forbidden)),
            "{url}"
        );
    }
}

#[tokio::test]
async fn cache_de_plages() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(range_body("1E4C9B93F3F0682250B6CF8331B7EE68FD8:3")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    let mut cache = Default::default();
    for _ in 0..3 {
        assert_eq!(
            pwned_count_cached(&client, &pw("password"), &mut cache)
                .await
                .unwrap(),
            3
        );
    }
}

// ---------- Catalogue et corrélation ----------

#[tokio::test]
async fn catalogue_telecharge_puis_stocke() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/breaches"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            { "Name": "Exemple", "Title": "Exemple", "Domain": "www.exemple.com", "BreachDate": "2024-05-02",
              "DataClasses": ["Email addresses", "Passwords"], "IsVerified": true },
            { "Name": "Faux", "Domain": "faux.com", "BreachDate": "2024-01-01", "DataClasses": ["Passwords"], "IsFabricated": true },
            { "Name": "Spam", "Domain": "spam.com", "BreachDate": "2024-01-01", "DataClasses": ["Email addresses"], "IsSpamList": true },
        ])))
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    let list = client.fetch_catalogue().await.unwrap();
    assert_eq!(list.len(), 3);
    let (_d, v) = vault();
    assert_eq!(store_catalogue(v.conn(), &list).unwrap(), 1);
    let domain: String = v
        .conn()
        .query_row("SELECT domain FROM breach", [], |r| r.get(0))
        .unwrap();
    assert_eq!(domain, "exemple.com");
    let synced: String = v
        .conn()
        .query_row(
            "SELECT value FROM setting WHERE key='breaches_synced_at'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(synced.ends_with('Z'));
}

#[test]
fn us32_breche_posterieure_au_changement() {
    let (_d, v) = vault();
    let a = account(&v, "exemple.com", Some("2023-01-10"), None);
    store_catalogue(
        v.conn(),
        &[breach(
            "Exemple",
            "exemple.com",
            "2024-05-02",
            &["Email addresses", "Passwords"],
        )],
    )
    .unwrap();
    let s = update_matches(v.conn()).unwrap();
    assert_eq!((s.total, s.relevant, s.newly_relevant), (1, 1, 1));
    let m = breach_matches(v.conn()).unwrap();
    assert_eq!(
        (m[0].account_id, m[0].relevant, m[0].kind.as_str()),
        (a, true, "domain")
    );
    // Recalcul idempotent : plus rien de « nouveau ».
    assert_eq!(update_matches(v.conn()).unwrap().newly_relevant, 0);
}

#[test]
fn pertinence_selon_dates_et_donnees() {
    let (_d, v) = vault();
    account(&v, "ancienne.com", Some("2025-01-01"), None); // mdp changé après la brèche
    account(&v, "inconnue.com", None, None); // date inconnue → pertinent
    account(&v, "courriels.com", Some("2020-01-01"), None); // pas de mots de passe exposés
    account(&v, "sansmatch.com", None, None);
    store_catalogue(
        v.conn(),
        &[
            breach("Ancienne", "ancienne.com", "2024-05-02", &["Passwords"]),
            breach("Inconnue", "inconnue.com", "2024-05-02", &["Passwords"]),
            breach(
                "Courriels",
                "courriels.com",
                "2024-05-02",
                &["Email addresses"],
            ),
        ],
    )
    .unwrap();
    update_matches(v.conn()).unwrap();
    let rel: std::collections::HashMap<String, bool> = breach_matches(v.conn())
        .unwrap()
        .into_iter()
        .map(|m| (m.breach_name, m.relevant))
        .collect();
    assert!(!rel["Ancienne"]);
    assert!(rel["Inconnue"]);
    assert!(!rel["Courriels"]);
    assert_eq!(rel.len(), 3);
}

#[test]
fn acquittement_conserve_et_brèche_retiree() {
    let (_d, v) = vault();
    let a = account(&v, "exemple.com", None, None);
    let b = breach("Exemple", "exemple.com", "2024-05-02", &["Passwords"]);
    store_catalogue(v.conn(), std::slice::from_ref(&b)).unwrap();
    update_matches(v.conn()).unwrap();
    acknowledge(v.conn(), "Exemple", a, "domain").unwrap();
    // Nouvelle synchro : l'acquittement survit.
    store_catalogue(v.conn(), &[b]).unwrap();
    update_matches(v.conn()).unwrap();
    assert!(
        breach_matches(v.conn()).unwrap()[0]
            .acknowledged_at
            .is_some()
    );
    // La brèche disparaît du catalogue : la correspondance aussi.
    store_catalogue(v.conn(), &[]).unwrap();
    assert!(breach_matches(v.conn()).unwrap().is_empty());
}

#[test]
fn comptes_supprimes_ignores() {
    let (_d, v) = vault();
    let a = account(&v, "exemple.com", None, None);
    v.conn()
        .execute("UPDATE account SET status='deleted' WHERE id=?1", [a])
        .unwrap();
    store_catalogue(
        v.conn(),
        &[breach(
            "Exemple",
            "exemple.com",
            "2024-05-02",
            &["Passwords"],
        )],
    )
    .unwrap();
    assert_eq!(update_matches(v.conn()).unwrap().total, 0);
}

// ---------- Courriel ----------

#[tokio::test]
async fn courriel_avec_cle_api() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/breachedaccount/alice%40example.test"))
        .and(header("hibp-api-key", "cle-fictive"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([{ "Name": "Adobe" }, { "Name": "Inconnue" }])),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/breachedaccount/propre%40example.test"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v3/breachedaccount/mauvaise%40example.test"))
        .respond_with(|_: &Request| ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    let key = pw("cle-fictive");
    let names = check_email(&client, &key, "alice@example.test")
        .await
        .unwrap();
    assert_eq!(names, ["Adobe", "Inconnue"]);
    assert!(
        check_email(&client, &key, "propre@example.test")
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        check_email(&client, &key, "mauvaise@example.test").await,
        Err(IntelError::Unauthorized)
    ));

    let (_d, v) = vault();
    v.conn()
        .execute(
            "INSERT INTO identity(kind, value) VALUES ('email','alice@example.test')",
            [],
        )
        .unwrap();
    let id: u32 = v
        .conn()
        .query_row("SELECT id FROM identity", [], |r| r.get(0))
        .unwrap();
    let acc = account(&v, "adobe.com", Some("2020-01-01"), Some(id));
    store_catalogue(
        v.conn(),
        &[breach("Adobe", "adobe.com", "2013-10-04", &["Passwords"])],
    )
    .unwrap();
    // La brèche est antérieure au changement de mot de passe : correspondance non pertinente.
    assert_eq!(store_email_matches(v.conn(), id, &names).unwrap(), 1);
    let m = breach_matches(v.conn()).unwrap();
    assert_eq!(
        (m[0].account_id, m[0].kind.as_str(), m[0].relevant),
        (acc, "email", false)
    );
}

#[test]
fn trousseau_en_memoire() {
    let s = MemoryStore::default();
    assert!(s.get("k").unwrap().is_none());
    s.set("k", &pw("v")).unwrap();
    use secrecy::ExposeSecret;
    assert_eq!(s.get("k").unwrap().unwrap().expose_secret(), "v");
    s.delete("k").unwrap();
    assert!(s.get("k").unwrap().is_none());
}

// ---------- 2fa.directory ----------

#[tokio::test]
async fn catalogue_mfa_met_a_jour_les_services_de_l_inventaire() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tfa/all.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
            ["GitHub", { "domain": "github.com", "tfa": ["totp", "u2f", "sms"], "passwordless": "allowed",
                         "additional-domains": ["githubapp.com"] }],
            ["Inconnu", { "domain": "pasdansinventaire.com", "tfa": ["totp"] }],
            ["Banque", { "domain": "https://www.banque.ca/", "tfa": ["sms", "email"] }],
            ["Cassé", 42],
            { "domain": "forme-objet.org", "tfa": ["totp"], "passkeys": true },
        ])))
        .mount(&server)
        .await;
    let client = IntelClient::for_mock(&server.uri());
    let entries = client.fetch_tfa().await.unwrap();
    assert_eq!(entries.len(), 4);
    assert!(entries[0].passkeys);
    assert_eq!(entries[0].domains, ["github.com", "githubapp.com"]);
    assert!(!entries[2].passkeys);
    assert!(entries[3].passkeys);

    let (_d, v) = vault();
    account(&v, "github.com", None, None);
    account(&v, "banque.ca", None, None);
    assert_eq!(store_tfa(v.conn(), &entries).unwrap(), 2);
    let (methods, passkeys): (String, bool) = v
        .conn()
        .query_row(
            "SELECT supported_mfa, passkeys FROM service WHERE domain='github.com'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(methods, r#"["totp","u2f","sms"]"#);
    assert!(passkeys);
    // Un service absent de l'inventaire n'est pas créé.
    let n: u32 = v
        .conn()
        .query_row(
            "SELECT count(*) FROM service WHERE domain='pasdansinventaire.com'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0);
}

#[test]
fn catalogue_mfa_format_inattendu() {
    assert!(parse_tfa(&serde_json::json!({ "pas": "une liste" })).is_err());
    assert!(parse_tfa(&serde_json::json!([])).unwrap().is_empty());
}
