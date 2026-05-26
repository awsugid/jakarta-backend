/// Normalize an email address: trim whitespace and convert to lowercase.
/// Does NOT remove dots or plus aliases (Gmail-specific rules can surprise users).
pub fn normalize_email(input: &str) -> String {
    input.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_normalization() {
        assert_eq!(normalize_email("  User@Example.COM  "), "user@example.com");
    }

    #[test]
    fn test_already_normalized() {
        assert_eq!(normalize_email("user@example.com"), "user@example.com");
    }

    #[test]
    fn test_plus_alias_preserved() {
        assert_eq!(
            normalize_email("user+alias@gmail.com"),
            "user+alias@gmail.com"
        );
    }

    #[test]
    fn test_dots_preserved() {
        assert_eq!(
            normalize_email("first.last@gmail.com"),
            "first.last@gmail.com"
        );
    }

    #[test]
    fn test_empty_after_trim() {
        assert_eq!(normalize_email("   "), "");
    }

    #[test]
    fn test_tab_and_newline_trimmed() {
        assert_eq!(normalize_email("\tuser@example.com\n"), "user@example.com");
    }

    #[test]
    fn test_mixed_case_domain() {
        assert_eq!(normalize_email("user@ExAmPlE.CoM"), "user@example.com");
    }

    #[test]
    fn test_subdomain_preserved() {
        assert_eq!(
            normalize_email("user@mail.example.com"),
            "user@mail.example.com"
        );
    }

    #[test]
    fn test_numeric_local_part() {
        assert_eq!(normalize_email("12345@example.com"), "12345@example.com");
    }

    #[test]
    fn test_plus_alias_with_dots() {
        assert_eq!(
            normalize_email("First.Last+Tag@Example.COM"),
            "first.last+tag@example.com"
        );
    }

    #[test]
    fn test_hyphen_in_domain() {
        assert_eq!(normalize_email("user@my-domain.com"), "user@my-domain.com");
    }

    #[test]
    fn test_underscore_in_local_part() {
        assert_eq!(
            normalize_email("my_name@example.com"),
            "my_name@example.com"
        );
    }

    #[test]
    fn test_single_character_local() {
        assert_eq!(normalize_email("a@b.cc"), "a@b.cc");
    }

    #[test]
    fn test_long_domain() {
        assert_eq!(
            normalize_email("user@sub.domain.example.co.uk"),
            "user@sub.domain.example.co.uk"
        );
    }
}
