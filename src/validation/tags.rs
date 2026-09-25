//! Validation and canonicalization for custom response tags.
//!
//! Contract: labels trimmed, 1..=50 chars, no control characters, at most 20
//! distinct labels per response, dedup case-insensitively (ignoring
//! whitespace differences) preserving first occurrence. Labels matching an
//! already-assigned survey label case-insensitively and
//! whitespace-insensitively reuse the existing spelling so the per-survey
//! catalog stays canonical. New labels have internal whitespace collapsed to
//! single spaces.

pub const MAX_TAGS: usize = 20;
pub const MAX_TAG_LEN: usize = 50;

/// Normalize and validate a raw tag list. Empty input is valid (clears tags).
///
/// `existing_survey_tags` holds labels already assigned anywhere in the survey.
pub fn normalize_tags(
    raw: &[String],
    existing_survey_tags: &[String],
) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for input in raw {
        let label = input.trim();
        let len = label.chars().count();
        if len == 0 {
            return Err("tags must not be empty or whitespace".to_string());
        }
        if len > MAX_TAG_LEN {
            return Err(format!(
                "tag longer than {MAX_TAG_LEN} characters: {:?}",
                snippet(label)
            ));
        }
        // Control check must run before whitespace collapsing: tabs and
        // newlines are control chars, and collapsing first would silently
        // accept them as spaces.
        if label.chars().any(|c| c.is_control()) {
            return Err(format!(
                "tag must not contain control characters: {:?}",
                snippet(label)
            ));
        }

        // Collapse internal whitespace runs to single spaces.
        let label = collapse_whitespace(label);

        // Canonicalize to existing survey spelling for case- and
        // whitespace-insensitive duplicates. Exact normalized match only —
        // no fuzzy merging of distinct labels.
        let key = label.to_lowercase();
        let label = existing_survey_tags
            .iter()
            .find(|e| collapse_whitespace(e.trim()).to_lowercase() == key)
            .cloned()
            .unwrap_or(label);

        if !seen.contains(&key) {
            seen.push(key);
            out.push(label);
        }
    }

    if out.len() > MAX_TAGS {
        return Err(format!("too many tags: {} (max {MAX_TAGS})", out.len()));
    }
    Ok(out)
}

fn snippet(s: &str) -> String {
    s.chars().take(20).collect()
}

/// Trim and collapse whitespace runs (incl. legacy multi-space data) to
/// single spaces: `"a   b"` -> `"a b"`.
fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn trims_and_keeps_valid() {
        let out = normalize_tags(&tags(&["  VIP  ", "follow-up"]), &[]).unwrap();
        assert_eq!(out, tags(&["VIP", "follow-up"]));
    }

    #[test]
    fn rejects_empty_after_trim() {
        assert!(normalize_tags(&tags(&["   "]), &[]).is_err());
    }

    #[test]
    fn rejects_over_length() {
        let long = "x".repeat(51);
        assert!(normalize_tags(&tags(&[&long]), &[]).is_err());
        let exact = "x".repeat(50);
        assert_eq!(
            normalize_tags(&tags(&[&exact]), &[]).unwrap(),
            tags(&[&exact])
        );
    }

    #[test]
    fn length_counts_chars_not_bytes() {
        // 50 two-byte chars = 100 bytes but 50 chars: valid.
        let uni = "é".repeat(50);
        assert!(normalize_tags(std::slice::from_ref(&uni), &[]).is_ok());
    }

    #[test]
    fn rejects_control_chars() {
        assert!(normalize_tags(&tags(&["bad\ttab"]), &[]).is_err());
        assert!(normalize_tags(&tags(&["bad\nnewline"]), &[]).is_err());
        assert!(normalize_tags(&tags(&["bad\u{0007}bell"]), &[]).is_err());
    }

    #[test]
    fn dedups_case_insensitively_keeping_first() {
        let out = normalize_tags(&tags(&["Follow-Up", "follow-up", "FOLLOW-UP"]), &[]).unwrap();
        assert_eq!(out, tags(&["Follow-Up"]));
    }

    #[test]
    fn canonicalizes_to_existing_survey_spelling() {
        let existing = tags(&["VIP"]);
        let out = normalize_tags(&tags(&["vip", "New"]), &existing).unwrap();
        assert_eq!(out, tags(&["VIP", "New"]));
    }

    #[test]
    fn reuses_existing_spelling_across_case_and_inner_whitespace() {
        let existing = tags(&["Interview Sent"]);
        let out = normalize_tags(&tags(&["interview   sent"]), &existing).unwrap();
        assert_eq!(out, tags(&["Interview Sent"]));
    }

    #[test]
    fn reuses_legacy_existing_spelling_with_multiple_spaces() {
        let existing = tags(&["Interview   Sent"]);
        let out = normalize_tags(&tags(&["interview sent"]), &existing).unwrap();
        assert_eq!(out, tags(&["Interview   Sent"]));
    }

    #[test]
    fn collapses_inner_whitespace_for_new_labels() {
        let out = normalize_tags(&tags(&["new   label", "a  b\u{00a0}c"]), &[]).unwrap();
        assert_eq!(out, tags(&["new label", "a b c"]));
    }

    #[test]
    fn dedups_across_case_and_whitespace_variants() {
        let out = normalize_tags(&tags(&["Follow  Up", "follow up", "FOLLOW   UP"]), &[]).unwrap();
        assert_eq!(out, tags(&["Follow Up"]));
    }

    #[test]
    fn distinct_labels_stay_distinct() {
        let existing = tags(&["Interview Sent"]);
        let out = normalize_tags(
            &tags(&["interview sent", "InterviewSent", "interview, sent"]),
            &existing,
        )
        .unwrap();
        assert_eq!(
            out,
            tags(&["Interview Sent", "InterviewSent", "interview, sent"])
        );
    }

    #[test]
    fn controls_rejected_before_whitespace_collapse() {
        // Tab/newline are whitespace: would collapse to a space if checked
        // in the wrong order, but must stay rejected.
        assert!(normalize_tags(&tags(&["ok\ttab"]), &[]).is_err());
        assert!(normalize_tags(&tags(&["line\nbreak"]), &[]).is_err());
        assert!(normalize_tags(&tags(&["bad\u{0007}bell"]), &[]).is_err());
    }

    #[test]
    fn empty_input_clears() {
        assert_eq!(normalize_tags(&[], &[]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn rejects_more_than_max_tags_after_dedup() {
        let many: Vec<String> = (0..21).map(|i| format!("tag{i}")).collect();
        assert!(normalize_tags(&many, &[]).is_err());
        // 21 raw labels that dedup to 20 are fine.
        let dup: Vec<String> = (0..20)
            .flat_map(|i| vec![format!("tag{i}"), format!("TAG{i}")])
            .collect();
        assert_eq!(normalize_tags(&dup, &[]).unwrap().len(), 20);
    }
}
