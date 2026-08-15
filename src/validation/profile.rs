use serde::{Deserialize, Serialize};

use super::url::validate_http_url;

pub const MAX_LINKS: usize = 8;
pub const MAX_DISPLAY_NAME_LEN: usize = 80;
pub const MAX_TITLE_LEN: usize = 100;
pub const MAX_OTHER_LABEL_LEN: usize = 32;
pub const MIN_USERNAME_LEN: usize = 3;
pub const MAX_USERNAME_LEN: usize = 30;

pub const RESERVED_USERNAMES: &[&str] = &[
    "admin",
    "administrator",
    "api",
    "app",
    "auth",
    "dashboard",
    "help",
    "login",
    "logout",
    "me",
    "null",
    "profile",
    "profiles",
    "root",
    "settings",
    "support",
    "undefined",
    "user",
    "users",
];

/// Validates and normalizes an optional username.
///
/// Rules:
/// - `None` or whitespace-only becomes `Ok(None)`
/// - Length must be between 3 and 30 characters
/// - Characters allowed: `[a-z0-9_-]`
/// - Normalized to lowercase
/// - Rejects reserved words (e.g. "admin", "api", "profile", "null", "undefined")
pub fn validate_username(username: Option<&str>) -> Result<Option<String>, &'static str> {
    let raw = match username {
        None => return Ok(None),
        Some(s) => s.trim(),
    };
    if raw.is_empty() {
        return Ok(None);
    }

    let lower = raw.to_lowercase();
    let char_count = lower.chars().count();
    if char_count < MIN_USERNAME_LEN {
        return Err("username must be at least 3 characters");
    }
    if char_count > MAX_USERNAME_LEN {
        return Err("username must be <= 30 characters");
    }

    for c in lower.chars() {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '_' && c != '-' {
            return Err("username may only contain lowercase letters, numbers, underscores, and hyphens");
        }
    }

    if RESERVED_USERNAMES.contains(&lower.as_str()) {
        return Err("username is reserved");
    }

    Ok(Some(lower))
}

/// A user-editable profile link. `label` is required for kind `other`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLink {
    pub kind: String,
    pub url: String,
    pub label: Option<String>,
}

/// Named platforms must live on their expected host.
/// website/other accept any http(s) host.
fn expected_hosts(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        "instagram" => Some(&["instagram.com"]),
        "linkedin" => Some(&["linkedin.com"]),
        "github" => Some(&["github.com"]),
        "x" => Some(&["x.com", "twitter.com"]),
        "youtube" => Some(&["youtube.com", "youtu.be"]),
        _ => None,
    }
}

pub fn is_valid_kind(kind: &str) -> bool {
    matches!(
        kind,
        "instagram" | "linkedin" | "github" | "website" | "x" | "youtube" | "other"
    )
}

/// Normalize an optional text field (display name / title).
/// Empty/whitespace becomes None. Length capped even when private.
pub fn normalize_text_field(
    value: &Option<String>,
    max_len: usize,
    field: &str,
) -> Result<Option<String>, String> {
    match value {
        None => Ok(None),
        Some(v) => {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }
            if trimmed.chars().count() > max_len {
                return Err(format!("{field} must be <= {max_len} chars"));
            }
            Ok(Some(trimmed.to_string()))
        }
    }
}

/// Trim stored link values, then validate kind, URL, host, uniqueness, and labels.
pub fn normalize_links(mut links: Vec<ProfileLink>) -> Result<Vec<ProfileLink>, String> {
    for link in &mut links {
        link.kind = link.kind.trim().to_lowercase();
        link.url = link.url.trim().to_string();
        link.label = link
            .label
            .take()
            .map(|label| label.trim().to_string())
            .filter(|label| !label.is_empty());
    }
    validate_links(&links)?;
    Ok(links)
}

/// Validate the full editable link list (kind, url, host, uniqueness, labels).
pub fn validate_links(links: &[ProfileLink]) -> Result<(), String> {
    if links.len() > MAX_LINKS {
        return Err(format!("links must be <= {MAX_LINKS} items"));
    }

    let mut seen_kinds: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut other_labels: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for (i, link) in links.iter().enumerate() {
        let ctx = |msg: String| format!("links[{i}]: {msg}");

        if !is_valid_kind(&link.kind) {
            return Err(ctx(format!("unsupported kind \"{}\"", link.kind)));
        }

        let url = validate_http_url(&link.url).map_err(ctx)?;

        if let Some(hosts) = expected_hosts(&link.kind) {
            let host = super::url::url_host(&url)
                .ok_or_else(|| ctx("url must have a host".to_string()))?;
            if !hosts.contains(&host.as_str()) {
                return Err(ctx(format!(
                    "kind \"{}\" url must be on {}",
                    link.kind,
                    hosts.join(" or ")
                )));
            }
        }

        if link.kind == "other" {
            let label = link
                .label
                .as_deref()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .ok_or_else(|| ctx("kind \"other\" requires a label".to_string()))?;
            if label.chars().count() > MAX_OTHER_LABEL_LEN {
                return Err(ctx(format!("label must be <= {MAX_OTHER_LABEL_LEN} chars")));
            }
            if !other_labels.insert(label) {
                return Err(ctx("duplicate label for \"other\" link".to_string()));
            }
        } else if !seen_kinds.insert(&link.kind) {
            return Err(ctx(format!("duplicate kind \"{}\"", link.kind)));
        }
    }

    Ok(())
}

/// Parse a stored links_json value. Corrupt rows degrade to an empty list
/// rather than failing the whole lookup.
/// ponytail: if corrupt rows ever need visibility, log via console here.
pub fn parse_links_json(raw: &str) -> Vec<ProfileLink> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(kind: &str, url: &str) -> ProfileLink {
        ProfileLink {
            kind: kind.to_string(),
            url: url.to_string(),
            label: None,
        }
    }

    fn other(url: &str, label: &str) -> ProfileLink {
        ProfileLink {
            kind: "other".to_string(),
            url: url.to_string(),
            label: Some(label.to_string()),
        }
    }

    #[test]
    fn accepts_each_named_platform() {
        let links = vec![
            link("instagram", "https://www.instagram.com/awsjakarta"),
            link("linkedin", "https://linkedin.com/in/johndoe"),
            link("github", "https://github.com/octocat"),
            link("x", "https://x.com/awsjakarta"),
            link("youtube", "https://youtube.com/@awsjakarta"),
            link("website", "https://blog.example.com"),
        ];
        assert!(validate_links(&links).is_ok());
    }

    #[test]
    fn accepts_twitter_host_for_x() {
        assert!(validate_links(&[link("x", "https://twitter.com/awsjakarta")]).is_ok());
    }

    #[test]
    fn accepts_youtu_be() {
        assert!(validate_links(&[link("youtube", "https://youtu.be/abc123")]).is_ok());
    }

    #[test]
    fn accepts_empty_list() {
        assert!(validate_links(&[]).is_ok());
    }

    #[test]
    fn rejects_more_than_max() {
        let links: Vec<_> = (0..=MAX_LINKS)
            .map(|i| other(&format!("https://example.com/{i}"), &format!("L{i}")))
            .collect();
        let err = validate_links(&links).unwrap_err();
        assert!(err.contains("<= 8"));
    }

    #[test]
    fn rejects_unknown_kind() {
        assert!(validate_links(&[link("myspace", "https://myspace.com/u")]).is_err());
    }

    #[test]
    fn rejects_wrong_host_for_kind() {
        assert!(validate_links(&[link("github", "https://gitlab.com/u")]).is_err());
        assert!(validate_links(&[link("instagram", "https://example.com")]).is_err());
    }

    #[test]
    fn rejects_bad_scheme() {
        assert!(validate_links(&[link("website", "javascript:alert(1)")]).is_err());
    }

    #[test]
    fn rejects_duplicate_named_kind() {
        let links = vec![
            link("github", "https://github.com/a"),
            link("github", "https://github.com/b"),
        ];
        assert!(validate_links(&links).unwrap_err().contains("duplicate"));
    }

    #[test]
    fn other_requires_label() {
        let mut l = other("https://example.com", "");
        l.label = None;
        assert!(validate_links(&[l])
            .unwrap_err()
            .contains("requires a label"));
    }

    #[test]
    fn other_label_max_len() {
        let long = "x".repeat(MAX_OTHER_LABEL_LEN + 1);
        assert!(validate_links(&[other("https://example.com", &long)])
            .unwrap_err()
            .contains("<= 32"));
    }

    #[test]
    fn other_labels_must_be_unique() {
        let links = vec![
            other("https://a.example.com", "Blog"),
            other("https://b.example.com", "Blog"),
        ];
        assert!(validate_links(&links)
            .unwrap_err()
            .contains("duplicate label"));
    }

    #[test]
    fn multiple_other_links_with_unique_labels_ok() {
        let links = vec![
            other("https://a.example.com", "Blog"),
            other("https://b.example.com", "Podcast"),
        ];
        assert!(validate_links(&links).is_ok());
    }

    #[test]
    fn text_field_normalization() {
        assert_eq!(
            normalize_text_field(&None, 80, "displayName").unwrap(),
            None
        );
        assert_eq!(
            normalize_text_field(&Some("  Avei  ".into()), 80, "displayName").unwrap(),
            Some("Avei".into())
        );
        assert_eq!(
            normalize_text_field(&Some("   ".into()), 80, "displayName").unwrap(),
            None
        );
        let long = "a".repeat(81);
        assert!(normalize_text_field(&Some(long), 80, "displayName").is_err());
    }

    #[test]
    fn normalizes_link_values_before_storage() {
        let normalized = normalize_links(vec![ProfileLink {
            kind: " GitHub ".into(),
            url: "  https://github.com/octocat  ".into(),
            label: Some("   ".into()),
        }])
        .unwrap();
        assert_eq!(normalized[0].kind, "github");
        assert_eq!(normalized[0].url, "https://github.com/octocat");
        assert_eq!(normalized[0].label, None);
    }

    #[test]
    fn links_json_round_trip() {
        let links = vec![
            link("github", "https://github.com/octocat"),
            other("https://example.com", "Blog"),
        ];
        let json = serde_json::to_string(&links).unwrap();
        let parsed = parse_links_json(&json);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].label.as_deref(), Some("Blog"));
        assert_eq!(parsed[0].kind, "github");
    }

    #[test]
    fn links_json_uses_camel_case() {
        let l = other("https://example.com", "Blog");
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"label\""));
    }

    #[test]
    fn corrupt_links_json_degrades_to_empty() {
        assert!(parse_links_json("not json [").is_empty());
    }

    #[test]
    fn validate_username_none_and_empty() {
        assert_eq!(validate_username(None), Ok(None));
        assert_eq!(validate_username(Some("")), Ok(None));
        assert_eq!(validate_username(Some("   ")), Ok(None));
    }

    #[test]
    fn validate_username_valid_formats() {
        assert_eq!(
            validate_username(Some("avei")),
            Ok(Some("avei".to_string()))
        );
        assert_eq!(
            validate_username(Some("Avei_123")),
            Ok(Some("avei_123".to_string()))
        );
        assert_eq!(
            validate_username(Some("  user-name_99  ")),
            Ok(Some("user-name_99".to_string()))
        );
        assert_eq!(
            validate_username(Some("abc")),
            Ok(Some("abc".to_string()))
        );
        let max_len_username = "a".repeat(30);
        assert_eq!(
            validate_username(Some(&max_len_username)),
            Ok(Some(max_len_username))
        );
    }

    #[test]
    fn validate_username_too_short() {
        assert!(validate_username(Some("a")).is_err());
        assert!(validate_username(Some("ab")).is_err());
        assert!(validate_username(Some("  ab  ")).is_err());
    }

    #[test]
    fn validate_username_too_long() {
        let too_long = "a".repeat(31);
        assert!(validate_username(Some(&too_long)).is_err());
    }

    #[test]
    fn validate_username_invalid_characters() {
        assert!(validate_username(Some("user name")).is_err());
        assert!(validate_username(Some("user@name")).is_err());
        assert!(validate_username(Some("user.name")).is_err());
        assert!(validate_username(Some("user#123")).is_err());
        assert!(validate_username(Some("user!name")).is_err());
        assert!(validate_username(Some("user$")).is_err());
        assert!(validate_username(Some("user/name")).is_err());
    }

    #[test]
    fn validate_username_rejects_reserved_words() {
        let reserved = ["admin", "api", "profile", "profiles", "null", "undefined", "ADMIN", "Api", "Me", "root", "auth"];
        for word in reserved {
            assert!(
                validate_username(Some(word)).is_err(),
                "expected '{word}' to be rejected as reserved"
            );
        }
    }
}

