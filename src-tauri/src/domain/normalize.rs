use super::DomainError;

/// Ramène une URL ou un nom d'hôte à son domaine enregistrable (Public Suffix List) :
/// `https://login.compte.exemple.co.uk/x?y` → `exemple.co.uk` (US-16).
pub fn normalize_domain(input: &str) -> Result<String, DomainError> {
    let invalid = || DomainError::InvalidDomain(input.trim().chars().take(80).collect());
    let mut s = input.trim().to_lowercase();
    if let Some((_, rest)) = s.split_once("://") {
        s = rest.to_owned();
    }
    let end = s.find(['/', '?', '#']).unwrap_or(s.len());
    s.truncate(end);
    if let Some((_, host)) = s.rsplit_once('@') {
        s = host.to_owned();
    }
    if let Some((host, port)) = s.rsplit_once(':')
        && !port.is_empty()
        && port.bytes().all(|b| b.is_ascii_digit())
    {
        s = host.to_owned();
    }
    let host = s.trim_matches('.');
    if host.is_empty() || host.len() > 253 || host.parse::<std::net::IpAddr>().is_ok() {
        return Err(invalid());
    }
    if !host
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    {
        return Err(invalid());
    }
    psl::domain_str(host).map(str::to_owned).ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cas_nominaux() {
        for (input, expected) in [
            ("exemple.com", "exemple.com"),
            ("https://www.Exemple.com/path?q=1", "exemple.com"),
            ("login.compte.exemple.co.uk", "exemple.co.uk"),
            ("http://user:pw@mail.google.com:8080/inbox", "google.com"),
            ("  GitHub.com.  ", "github.com"),
            ("foo.github.io", "foo.github.io"),
        ] {
            assert_eq!(normalize_domain(input).unwrap(), expected, "{input}");
        }
    }

    #[test]
    fn entrees_refusees() {
        for input in [
            "",
            "   ",
            "localhost",
            "192.168.0.1",
            "com",
            "co.uk",
            "a b.com",
            "http://",
        ] {
            assert!(normalize_domain(input).is_err(), "{input}");
        }
    }

    #[test]
    fn jamais_de_panique_sur_entrees_arbitraires() {
        for input in [
            "@@@",
            ":::",
            "http://:80",
            "é.com",
            "\0",
            "a..b",
            "-.-",
            "/////",
        ] {
            let _ = normalize_domain(input);
        }
    }
}
