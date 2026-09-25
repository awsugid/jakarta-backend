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

/// Check if a form is currently open for new applications (strict: inactive
/// forms are never open — speaker semantics). Kind-aware callers use
/// `accepting_for_kind` instead.
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

/// Check if existing applications can still be edited (strict: inactive
/// forms are never editable — speaker semantics). `editable_for_kind` is the
/// kind-aware entry point.
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

// ---------------------------------------------------------------------------
// Volunteer Talent Pool mode
// ---------------------------------------------------------------------------
// For volunteer forms `is_active = false` means "not recruiting" (Talent Pool)
// rather than "closed": submissions stay open and existing responses stay
// editable, gated only by dates. Speaker forms keep the strict `is_active`
// semantics above.

/// Volunteer-aware submission gate: volunteers submit while their date window
/// is open even when inactive (Talent Pool); speakers require the strict
/// `is_form_open` (is_active + dates).
pub fn accepting_for_kind(form: &ApplicationForm) -> bool {
    accepting_for_kind_at(form, &utc_now_string())
}

/// Pure-Rust version of `accepting_for_kind`.
pub fn accepting_for_kind_at(form: &ApplicationForm, now: &str) -> bool {
    if form.kind == "volunteer" {
        is_form_accepting_submissions_at(form, now)
    } else {
        is_form_open_at(form, now)
    }
}

/// Volunteer Talent Pool submission window: date checks only, `is_active`
/// deliberately ignored. Pure-Rust core (no JS clock) so it stays testable.
pub fn is_form_accepting_submissions_at(form: &ApplicationForm, now: &str) -> bool {
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

/// Volunteer-aware editability: Talent Pool responses stay editable; speaker
/// responses keep the strict `is_form_editable` gate.
pub fn editable_for_kind(form: &ApplicationForm) -> bool {
    editable_for_kind_at(form, &utc_now_string())
}

/// Pure-Rust version of `editable_for_kind`.
pub fn editable_for_kind_at(form: &ApplicationForm, now: &str) -> bool {
    if form.kind == "volunteer" {
        is_form_editable_ignoring_active_at(form, now)
    } else {
        is_form_editable_at(form, now)
    }
}

/// `is_form_editable` without the `is_active` gate (volunteer Talent Pool).
/// Pure-Rust core (no JS clock) so it stays testable.
pub fn is_form_editable_ignoring_active_at(form: &ApplicationForm, now: &str) -> bool {
    if let Some(ref editable_until) = form.editable_until {
        return editable_until.as_str() >= now;
    }
    // No editable_until set: editable as long as submissions are accepted
    is_form_accepting_submissions_at(form, now)
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

    // ---- Talent Pool: accepting_for_kind / is_form_accepting_submissions_at ----

    #[test]
    fn talent_pool_inactive_volunteer_still_accepts_within_dates() {
        let mut form = make_form(); // kind = volunteer
        form.is_active = false;
        assert!(is_form_accepting_submissions_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
    }

    #[test]
    fn talent_pool_dates_still_gate_volunteer() {
        let mut form = make_form();
        form.is_active = false;
        form.opens_at = Some("2026-07-01T00:00:00Z".to_string());
        assert!(!is_form_accepting_submissions_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
        form.opens_at = None;
        form.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        assert!(!is_form_accepting_submissions_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
    }

    #[test]
    fn accepting_for_kind_volunteer_ignores_active_speaker_does_not() {
        let mut volunteer = make_form(); // kind = volunteer
        volunteer.is_active = false;
        assert!(accepting_for_kind_at(&volunteer, "2026-06-01T00:00:00Z"));

        let mut speaker = make_form();
        speaker.kind = "speaker".to_string();
        speaker.is_active = false;
        assert!(!accepting_for_kind_at(&speaker, "2026-06-01T00:00:00Z"));
        speaker.is_active = true;
        assert!(accepting_for_kind_at(&speaker, "2026-06-01T00:00:00Z"));
    }

    // ---- editable_for_kind / is_form_editable_ignoring_active_at ----

    #[test]
    fn editable_ignoring_active_volunteer_talent_pool() {
        let mut form = make_form();
        form.is_active = false;
        assert!(is_form_editable_ignoring_active_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
        assert!(editable_for_kind_at(&form, "2026-06-01T00:00:00Z"));
    }

    #[test]
    fn editable_ignoring_active_still_honors_editable_until() {
        let mut form = make_form();
        form.is_active = false;
        form.editable_until = Some("2026-05-01T00:00:00Z".to_string());
        assert!(!is_form_editable_ignoring_active_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
        form.editable_until = Some("2026-07-01T00:00:00Z".to_string());
        assert!(is_form_editable_ignoring_active_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
    }

    #[test]
    fn editable_ignoring_active_falls_back_to_date_window() {
        let mut form = make_form();
        form.is_active = false;
        form.closes_at = Some("2026-05-01T00:00:00Z".to_string());
        // Closed window + no editable_until => not editable
        assert!(!is_form_editable_ignoring_active_at(
            &form,
            "2026-06-01T00:00:00Z"
        ));
    }

    #[test]
    fn editable_for_kind_speaker_keeps_strict_gate() {
        let mut speaker = make_form();
        speaker.kind = "speaker".to_string();
        speaker.is_active = false;
        speaker.editable_until = Some("2026-07-01T00:00:00Z".to_string());
        assert!(!editable_for_kind_at(&speaker, "2026-06-01T00:00:00Z"));
    }
}
