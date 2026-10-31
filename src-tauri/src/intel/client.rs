use std::time::Duration;

use reqwest::{StatusCode, Url, redirect::Policy};

use super::IntelError;

pub const USER_AGENT: &str = concat!("Vigie/", env!("CARGO_PKG_VERSION"));
const MAX_BODY: usize = 20 * 1024 * 1024;
const MAX_ATTEMPTS: u32 = 4;
const MAX_WAIT: Duration = Duration::from_secs(60);

/// Hôtes autorisés. Toute autre destination est refusée avant l'envoi.
pub const ALLOWED_HOSTS: [&str; 3] = [
    "api.pwnedpasswords.com",
    "haveibeenpwned.com",
    "api.2fa.directory",
];

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub pwned: String,
    pub hibp: String,
    /// Catalogue 2fa.directory.
    pub tfa: String,
}

impl Endpoints {
    pub fn production() -> Self {
        Self {
            pwned: "https://api.pwnedpasswords.com".into(),
            hibp: "https://haveibeenpwned.com/api/v3".into(),
            tfa: "https://api.2fa.directory/v3".into(),
        }
    }
}

pub struct IntelClient {
    http: reqwest::Client,
    pub endpoints: Endpoints,
    allowed: Vec<String>,
    insecure_for_tests: bool,
    backoff_base: Duration,
}

impl IntelClient {
    pub fn production() -> Self {
        Self::build(
            Endpoints::production(),
            ALLOWED_HOSTS.map(String::from).to_vec(),
            false,
            Duration::from_secs(1),
        )
    }

    /// Client de test : un serveur local, HTTP autorisé, backoff court.
    #[cfg(test)]
    pub fn for_mock(base: &str) -> Self {
        let host = Url::parse(base).unwrap().host_str().unwrap().to_owned();
        Self::build(
            Endpoints {
                pwned: base.into(),
                hibp: format!("{base}/api/v3"),
                tfa: format!("{base}/tfa"),
            },
            vec![host],
            true,
            Duration::from_millis(5),
        )
    }

    fn build(
        endpoints: Endpoints,
        allowed: Vec<String>,
        insecure: bool,
        backoff: Duration,
    ) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .redirect(Policy::none())
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .https_only(!insecure)
            .build()
            .expect("client HTTP");
        Self {
            http,
            endpoints,
            allowed,
            insecure_for_tests: insecure,
            backoff_base: backoff,
        }
    }

    fn check(&self, url: &str) -> Result<Url, IntelError> {
        let u = Url::parse(url).map_err(|_| IntelError::Forbidden)?;
        let scheme_ok = u.scheme() == "https" || (self.insecure_for_tests && u.scheme() == "http");
        let host_ok = u
            .host_str()
            .is_some_and(|h| self.allowed.iter().any(|a| a == h));
        if scheme_ok && host_ok {
            Ok(u)
        } else {
            Err(IntelError::Forbidden)
        }
    }

    /// GET avec backoff exponentiel. `Ok(None)` pour un 404 (ressource absente).
    pub async fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
    ) -> Result<Option<Vec<u8>>, IntelError> {
        let url = self.check(url)?;
        let mut last = IntelError::Offline;
        for attempt in 0..MAX_ATTEMPTS {
            let mut req = self.http.get(url.clone());
            for (k, v) in headers {
                req = req.header(*k, *v);
            }
            let wait = match req.send().await {
                Err(e) => {
                    last = if e.is_timeout() || e.is_connect() || e.is_request() {
                        IntelError::Offline
                    } else {
                        IntelError::BadResponse("erreur réseau".into())
                    };
                    self.backoff_base * 2u32.pow(attempt)
                }
                Ok(resp) => match resp.status() {
                    s if s.is_success() => {
                        if resp.content_length().is_some_and(|l| l as usize > MAX_BODY) {
                            return Err(IntelError::BadResponse("réponse trop volumineuse".into()));
                        }
                        let bytes = resp.bytes().await.map_err(|_| IntelError::Offline)?;
                        if bytes.len() > MAX_BODY {
                            return Err(IntelError::BadResponse("réponse trop volumineuse".into()));
                        }
                        return Ok(Some(bytes.to_vec()));
                    }
                    StatusCode::NOT_FOUND => return Ok(None),
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                        return Err(IntelError::Unauthorized);
                    }
                    StatusCode::TOO_MANY_REQUESTS => {
                        last = IntelError::RateLimited;
                        retry_after(&resp).unwrap_or(self.backoff_base * 2u32.pow(attempt))
                    }
                    s if s.is_server_error() => {
                        last = IntelError::Http(s.as_u16());
                        self.backoff_base * 2u32.pow(attempt)
                    }
                    s => return Err(IntelError::Http(s.as_u16())),
                },
            };
            if attempt + 1 < MAX_ATTEMPTS {
                tokio::time::sleep(wait.min(MAX_WAIT)).await;
            }
        }
        Err(last)
    }
}

fn retry_after(resp: &reqwest::Response) -> Option<Duration> {
    let secs: u64 = resp
        .headers()
        .get("retry-after")?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(Duration::from_secs(secs))
}
