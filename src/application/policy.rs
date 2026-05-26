use crate::storage::d1::ApplicationForm;

/// Return the current UTC time as an ISO 8601 string like "2026-07-01T00:00:00.000Z".
///
/// Uses `js_sys::Date` which works in wasm32-unknown-unknown.
pub fn utc_now_string() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
}

/// Check if a form is currently open for new applications.
pub fn is_form_open(form: &ApplicationForm) -> bool {
    is_form_open_at(form, &utc_now_string())
}

/// Pure-Rust version of `is_form_open` that accepts the current time as a parameter.
/// This is the testable core that doesn't depend on JS/WASM.
pub fn is_form_open_at(form: &ApplicationForm, now: &str) -> bool {
    if !form.is_active {
        return false;
    }

    if let Some(ref opens_at) = form.opens_at {
        if opens_at.as_str() > now {
            return false;
        }
    }
    if let Some(ref closes_at) = form.closes_at {
        if closes_at.as_str() < now {
            return false;
        }
    }

    true
}

/// Check if existing applications can still be edited.
pub fn is_form_editable(form: &ApplicationForm) -> bool {
    is_form_editable_at(form, &utc_now_string())
}

/// Pure-Rust version of `is_form_editable` that accepts the current time as a parameter.
pub fn is_form_editable_at(form: &ApplicationForm, now: &str) -> bool {
    if !form.is_active {
        return false;
    }
    if let Some(ref editable_until) = form.editable_until {
        return editable_until.as_str() >= now;
    }
    // No editable_until set: editable as long as form is open
    is_form_open_at(form, now)
}

/// Check if form is archived (past its archive date).
pub fn is_form_archived(form: &ApplicationForm) -> bool {
    is_form_archived_at(form, &utc_now_string())
}

/// Pure-Rust version of `is_form_archived` that accepts the current time as a parameter.
pub fn is_form_archived_at(form: &ApplicationForm, now: &str) -> bool {
    if let Some(ref archive_after) = form.archive_after {
        return archive_after.as_str() < now;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::d1::ApplicationForm;

    fn make_form() -> ApplicationForm {
        ApplicationForm {
            id: "test".to_string(),
            kind: "volunteer".to_string(),
            slug: "test".to_string(),
            title: "Test Form".to_string(),
            description: None,
            formbricks_survey_id: "survey-123".to_string(),
            formbricks_public_url: None,
            email_question_id: "q-email".to_string(),
            linkedin_question_id: "q-linkedin".to_string(),
            is_active: true,
            opens_at: None,
            closes_at: None,
            editable_until: None,
            archive_after: None,
            display_order: 0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    // ---- is_form_open_at ----

    #[test]
    fn test_open_active_no_dates() {
        let form = make_form();
        assert!(is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_inactive_never_open() {
        let mut form = make_form();
        form.is_active = false;
        assert!(!is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_opens_at_in_future() {
        let mut form = make_form();
        form.opens_at = Some("2026-07-01T00:00:00Z".to_string());
        assert!(!is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_opens_at_in_past() {
        let mut form = make_form();
        form.opens_at = Some("2026-05-01T00:00:00Z".to_string());
        assert!(is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_opens_at_exact_now() {
        let mut form = make_form();
        form.opens_at = Some("2026-06-01T00:00:00Z".to_string());
        assert!(is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_closes_at_in_future() {
        let mut form = make_form();
        form.closes_at = Some("2026-07-01T00:00:00Z".to_string());
        assert!(is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_closes_at_in_past() {
        let mut form = make_form();
        form.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        assert!(!is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_closes_at_exact_now() {
        let mut form = make_form();
        form.closes_at = Some("2026-06-01T00:00:00Z".to_string());
        // closes_at == now: original code uses closes_at < now to reject,
        // so equal means the form IS still open
        assert!(is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_both_dates_in_range() {
        let mut form = make_form();
        form.opens_at = Some("2026-05-01T00:00:00Z".to_string());
        form.closes_at = Some("2026-07-01T00:00:00Z".to_string());
        assert!(is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_both_dates_before_range() {
        let mut form = make_form();
        form.opens_at = Some("2026-07-01T00:00:00Z".to_string());
        form.closes_at = Some("2026-08-01T00:00:00Z".to_string());
        assert!(!is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_open_with_both_dates_after_range() {
        let mut form = make_form();
        form.opens_at = Some("2026-01-01T00:00:00Z".to_string());
        form.closes_at = Some("2026-03-01T00:00:00Z".to_string());
        assert!(!is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_inactive_with_valid_dates_still_not_open() {
        let mut form = make_form();
        form.is_active = false;
        form.opens_at = Some("2026-05-01T00:00:00Z".to_string());
        form.closes_at = Some("2026-07-01T00:00:00Z".to_string());
        assert!(!is_form_open_at(&form, "2026-06-01T00:00:00Z"));
    }

    // ---- is_form_editable_at ----

    #[test]
    fn test_editable_open_form_no_deadline() {
        let form = make_form();
        assert!(is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_with_deadline_in_future() {
        let mut form = make_form();
        form.editable_until = Some("2026-07-01T00:00:00Z".to_string());
        assert!(is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_with_deadline_in_past() {
        let mut form = make_form();
        form.editable_until = Some("2026-05-01T00:00:00Z".to_string());
        assert!(!is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_with_deadline_exact_now() {
        let mut form = make_form();
        form.editable_until = Some("2026-06-01T00:00:00Z".to_string());
        // editable_until == now: still editable (>=)
        assert!(is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_closed_form_without_editable_until() {
        let mut form = make_form();
        form.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        // Form is closed (past closes_at), no editable_until => not editable
        assert!(!is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_closed_form_with_extended_deadline() {
        let mut form = make_form();
        form.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        form.editable_until = Some("2026-07-01T00:00:00Z".to_string());
        // Form is closed but editable_until extends editing
        assert!(is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_editable_inactive_form_not_editable() {
        let mut form = make_form();
        form.is_active = false;
        form.editable_until = Some("2026-07-01T00:00:00Z".to_string());
        assert!(!is_form_editable_at(&form, "2026-06-01T00:00:00Z"));
    }

    // ---- is_form_archived_at ----

    #[test]
    fn test_archived_no_archive_date() {
        let form = make_form();
        assert!(!is_form_archived_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_archived_archive_in_future() {
        let mut form = make_form();
        form.archive_after = Some("2026-07-01T00:00:00Z".to_string());
        assert!(!is_form_archived_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_archived_archive_in_past() {
        let mut form = make_form();
        form.archive_after = Some("2026-05-01T00:00:00Z".to_string());
        assert!(is_form_archived_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_archived_archive_exact_now() {
        let mut form = make_form();
        form.archive_after = Some("2026-06-01T00:00:00Z".to_string());
        // archive_after == now: NOT archived (must be strictly less)
        assert!(!is_form_archived_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn test_archived_inactive_form_with_archive_date() {
        let mut form = make_form();
        form.is_active = false;
        form.archive_after = Some("2026-05-01T00:00:00Z".to_string());
        // archive check doesn't look at is_active
        assert!(is_form_archived_at(&form, "2026-06-01T00:00:00Z"));
    }
}
