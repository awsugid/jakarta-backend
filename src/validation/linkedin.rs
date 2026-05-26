/// Normalize a LinkedIn profile URL to canonical form.
/// Returns Ok(normalized_url) if valid, Err(reason) if invalid.
pub fn normalize_linkedin_url(input: &str) -> Result<String, String> {
    // 1. Trim whitespace
    let trimmed = input.trim();

    // 2. If empty, return Err
    if trimmed.is_empty() {
        return Err("empty input".to_string());
    }

    // 3. Strip scheme if present (http:// or https://)
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);

    // 4. Strip query string (?) and fragment (#)
    let without_query = without_scheme.split('?').next().unwrap_or(without_scheme);
    let without_fragment = without_query.split('#').next().unwrap_or(without_query);

    // 5. Strip trailing slash
    let stripped = without_fragment
        .strip_suffix('/')
        .unwrap_or(without_fragment);

    // 6. Parse host + path
    // Find the first '/' to separate host from path
    let (host, path) = match stripped.find('/') {
        Some(idx) => (&stripped[..idx], &stripped[idx..]),
        None => return Err("no path found".to_string()),
    };

    // 7. Validate host is linkedin.com or www.linkedin.com
    let host_lower = host.to_lowercase();
    if host_lower != "linkedin.com" && host_lower != "www.linkedin.com" {
        return Err(format!("invalid domain: {}", host));
    }

    // 8. Validate path starts with /in/ and has a slug
    if !path.starts_with("/in/") {
        return Err("path must start with /in/".to_string());
    }

    let slug = &path[4..]; // skip "/in/"
    if slug.is_empty() {
        return Err("empty profile slug".to_string());
    }

    // 9. Reject if path matches /company/, /jobs/, /feed/, /school/, etc.
    let path_lower = path.to_lowercase();
    let forbidden_prefixes = ["/company/", "/jobs/", "/feed", "/school/"];
    for prefix in &forbidden_prefixes {
        if path_lower.starts_with(prefix) {
            return Err(format!("forbidden path prefix: {}", prefix));
        }
    }

    // 10. Return lowercase canonical: linkedin.com/in/{slug}
    let slug_lower = slug.to_lowercase();
    Ok(format!("linkedin.com/in/{}", slug_lower))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_profile() {
        assert_eq!(
            normalize_linkedin_url("https://www.linkedin.com/in/johndoe").unwrap(),
            "linkedin.com/in/johndoe"
        );
    }

    #[test]
    fn test_without_www() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/janedoe").unwrap(),
            "linkedin.com/in/janedoe"
        );
    }

    #[test]
    fn test_with_query_and_fragment() {
        assert_eq!(
            normalize_linkedin_url(
                "https://www.linkedin.com/in/johndoe?trk=profile&utm_source=share#section"
            )
            .unwrap(),
            "linkedin.com/in/johndoe"
        );
    }

    #[test]
    fn test_http_scheme() {
        assert_eq!(
            normalize_linkedin_url("http://linkedin.com/in/testuser/").unwrap(),
            "linkedin.com/in/testuser"
        );
    }

    #[test]
    fn test_no_scheme() {
        assert_eq!(
            normalize_linkedin_url("linkedin.com/in/testuser").unwrap(),
            "linkedin.com/in/testuser"
        );
    }

    #[test]
    fn test_uppercase() {
        assert_eq!(
            normalize_linkedin_url("https://LinkedIn.com/in/JohnDoe").unwrap(),
            "linkedin.com/in/johndoe"
        );
    }

    #[test]
    fn test_reject_company_url() {
        assert!(normalize_linkedin_url("https://linkedin.com/company/google").is_err());
    }

    #[test]
    fn test_reject_jobs_url() {
        assert!(normalize_linkedin_url("https://linkedin.com/jobs/view/123").is_err());
    }

    #[test]
    fn test_reject_feed_url() {
        assert!(normalize_linkedin_url("https://linkedin.com/feed").is_err());
    }

    #[test]
    fn test_reject_school_url() {
        assert!(normalize_linkedin_url("https://linkedin.com/school/mit").is_err());
    }

    #[test]
    fn test_reject_wrong_domain() {
        assert!(normalize_linkedin_url("https://example.com/in/johndoe").is_err());
    }

    #[test]
    fn test_reject_empty_slug() {
        assert!(normalize_linkedin_url("https://linkedin.com/in/").is_err());
    }

    #[test]
    fn test_reject_no_in_path() {
        assert!(normalize_linkedin_url("https://linkedin.com/profile/johndoe").is_err());
    }

    #[test]
    fn test_reject_empty_input() {
        assert!(normalize_linkedin_url("").is_err());
    }

    #[test]
    fn test_reject_whitespace_only() {
        assert!(normalize_linkedin_url("   ").is_err());
    }

    #[test]
    fn test_accepts_hyphenated_slug() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/john-doe-123").unwrap(),
            "linkedin.com/in/john-doe-123"
        );
    }

    // ---- Edge case tests ----

    #[test]
    fn test_trailing_slash_removed() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/user/").unwrap(),
            "linkedin.com/in/user"
        );
    }

    #[test]
    fn test_multiple_trailing_slashes() {
        // The implementation only strips one trailing slash via strip_suffix,
        // so "user//" becomes "user/" which has a non-empty path after /in/.
        // Actually, "linkedin.com/in/user//" -> strip_suffix('/') -> "linkedin.com/in/user/"
        // path is "/in/user/", slug is "user/" which is non-empty => ok
        let result = normalize_linkedin_url("https://linkedin.com/in/user//");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "linkedin.com/in/user/");
    }

    #[test]
    fn test_slug_with_underscores() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/john_doe").unwrap(),
            "linkedin.com/in/john_doe"
        );
    }

    #[test]
    fn test_slug_with_numbers() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/john123").unwrap(),
            "linkedin.com/in/john123"
        );
    }

    #[test]
    fn test_reject_non_linkedin_domain() {
        assert!(normalize_linkedin_url("https://notlinkedin.com/in/user").is_err());
    }

    #[test]
    fn test_just_linkedin_com() {
        // No path at all => "no path found"
        assert!(normalize_linkedin_url("https://linkedin.com").is_err());
    }

    #[test]
    fn test_just_linkedin_com_with_trailing_slash() {
        // Path is just "/", slug is "" => "empty profile slug"
        assert!(normalize_linkedin_url("https://linkedin.com/").is_err());
    }

    #[test]
    fn test_reject_feed_with_no_path_beyond() {
        // "/feed" starts with "/feed" forbidden prefix
        assert!(normalize_linkedin_url("https://linkedin.com/feed").is_err());
    }

    #[test]
    fn test_preserves_slug_case_lowered() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/JohnDoe").unwrap(),
            "linkedin.com/in/johndoe"
        );
    }

    #[test]
    fn test_very_long_slug() {
        let long_slug = "a".repeat(200);
        let url = format!("https://linkedin.com/in/{}", long_slug);
        assert_eq!(
            normalize_linkedin_url(&url).unwrap(),
            format!("linkedin.com/in/{}", long_slug)
        );
    }

    #[test]
    fn test_slug_with_special_chars() {
        // LinkedIn allows hyphens, underscores, and numbers
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/john_doe-123").unwrap(),
            "linkedin.com/in/john_doe-123"
        );
    }

    #[test]
    fn test_with_query_params() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/user?trk=profile").unwrap(),
            "linkedin.com/in/user"
        );
    }

    #[test]
    fn test_with_fragment() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/user#about").unwrap(),
            "linkedin.com/in/user"
        );
    }

    #[test]
    fn test_edge_case_query_and_fragment_together() {
        assert_eq!(
            normalize_linkedin_url("https://linkedin.com/in/user?a=1#section").unwrap(),
            "linkedin.com/in/user"
        );
    }

    #[test]
    fn test_whitespace_slug_trimmed_to_empty() {
        // "https://linkedin.com/in/ " -> trim() strips trailing space -> "https://linkedin.com/in/"
        // -> strip trailing slash -> "linkedin.com/in" -> path "/in", slug "" -> Err
        let result = normalize_linkedin_url("https://linkedin.com/in/ ");
        assert!(result.is_err());
    }

    #[test]
    fn test_www_stripped_from_canonical() {
        assert_eq!(
            normalize_linkedin_url("https://www.linkedin.com/in/test").unwrap(),
            "linkedin.com/in/test"
        );
    }
}
