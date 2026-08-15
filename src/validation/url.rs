/// Shared URL validation for user-supplied links.
///
/// No `url` crate in this worker; minimal http(s) parsing is enough for the
/// trust boundary: scheme allow-list, host presence, no credentials.
pub const MAX_URL_LEN: usize = 2048;

/// Validate an http(s) URL. Returns the trimmed URL on success.
pub fn validate_http_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("url must not be empty".to_string());
    }
    if trimmed.chars().count() > MAX_URL_LEN {
        return Err(format!("url must be <= {MAX_URL_LEN} chars"));
    }
    if trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("url must not contain whitespace or control characters".to_string());
    }
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("url must use http or https scheme".to_string());
    }
    if url_host(trimmed).is_none() {
        return Err("url must have a host".to_string());
    }
    Ok(trimmed.to_string())
}

/// Extract the lowercase hostname from an http(s) URL.
/// Strips the port and a leading `www.`; returns None when the URL has no
/// usable host (empty, credentials present, or malformed).
pub fn url_host(input: &str) -> Option<String> {
    let trimmed = input.trim();
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .or_else(|| trimmed.strip_prefix("HTTPS://"))
        .or_else(|| trimmed.strip_prefix("HTTP://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() {
        return None;
    }
    // Reject userinfo/credentials: anything before '@' would be user:pass.
    if authority.contains('@') {
        return None;
    }
    let host = authority.split(':').next()?; // drop port
    if host.is_empty() {
        return None;
    }
    let lower = host.to_lowercase();
    let stripped = lower.strip_prefix("www.").unwrap_or(&lower);
    Some(stripped.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_https() {
        assert_eq!(
            validate_http_url("https://example.com/page").unwrap(),
            "https://example.com/page"
        );
    }

    #[test]
    fn accepts_http_with_port_and_trims() {
        assert_eq!(
            validate_http_url("  http://localhost:8080/x ").unwrap(),
            "http://localhost:8080/x"
        );
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_http_url("   ").is_err());
    }

    #[test]
    fn rejects_other_schemes() {
        for bad in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "ftp://example.com",
            "example.com/no-scheme",
        ] {
            assert!(validate_http_url(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn rejects_credentials() {
        assert!(validate_http_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn rejects_whitespace_inside() {
        assert!(validate_http_url("https://example.com/a b").is_err());
    }

    #[test]
    fn rejects_missing_host() {
        assert!(validate_http_url("https:///path").is_err());
    }

    #[test]
    fn rejects_oversized() {
        let url = format!("https://example.com/{}", "a".repeat(MAX_URL_LEN));
        assert!(validate_http_url(&url).is_err());
    }

    #[test]
    fn host_extraction_variants() {
        assert_eq!(
            url_host("https://GitHub.COM/user"),
            Some("github.com".into())
        );
        assert_eq!(
            url_host("https://www.instagram.com/p/1/"),
            Some("instagram.com".into())
        );
        assert_eq!(url_host("https://x.com/a"), Some("x.com".into()));
        assert_eq!(url_host("http://localhost:4321"), Some("localhost".into()));
        assert_eq!(url_host("not a url"), None);
        assert_eq!(url_host("https://user@host/"), None);
    }
}
