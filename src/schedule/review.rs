//! Pure publish-review logic: no worker/D1 types, fully unit-testable.
//!
//! `review_snapshot` takes the admin workspace state (rooms, schedule items,
//! speaker directory, previously published items) and produces the review
//! report plus the frozen candidate list that a publish would snapshot.

use std::collections::HashMap;

use serde::Serialize;

use crate::schedule::types::{EventFormat, EventRoom, EventScheduleItem, ScheduleSpeaker};

/// Minimal RFC3339 parser (no external deps): YYYY-MM-DDTHH:MM[:SS[.fff]]
/// followed by Z or ±HH:MM. Returns unix epoch seconds.
pub fn parse_rfc3339_epoch(s: &str) -> Option<i64> {
    let s = s.trim();
    let bytes = s.as_bytes();
    if bytes.len() < 17 {
        return None;
    }
    let num = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse::<i64>().ok() };
    let year = num(0, 4)?;
    let month = num(5, 7)?;
    let day = num(8, 10)?;
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let sep = bytes[10];
    if sep != b'T' && sep != b't' && sep != b' ' {
        return None;
    }
    let hour = num(11, 13)?;
    let minute = num(14, 16)?;
    if bytes[13] != b':' || hour > 23 || minute > 59 {
        return None;
    }
    let mut idx = 16;
    let mut second = 0i64;
    if bytes.get(idx) == Some(&b':') {
        second = num(17, 19)?;
        if second > 59 {
            return None;
        }
        idx = 19;
        if bytes.get(idx) == Some(&b'.') {
            // Validate fractional digits; sub-second precision is ignored for
            // overlap math (second resolution is sufficient).
            let start = idx + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end == start {
                return None;
            }
            idx = end;
        }
    }
    let offset_secs = match bytes.get(idx) {
        Some(b'Z') | Some(b'z') => {
            idx += 1;
            0i64
        }
        Some(b'+') | Some(b'-') => {
            let sign = if bytes[idx] == b'-' { -1 } else { 1 };
            let oh = num(idx + 1, idx + 3)?;
            let om = num(idx + 4, idx + 6)?;
            if bytes.get(idx + 3) != Some(&b':') || oh > 23 || om > 59 {
                return None;
            }
            idx += 6;
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    if idx != bytes.len() {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hour * 3600 + minute * 60 + second - offset_secs)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedItem {
    pub id: String,
    pub title: String,
    /// Machine-readable exclusion reason: `draft` (not marked for publication).
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictItem {
    /// One of: missing-fields | invalid-range | unknown-room | unknown-speaker |
    /// room-overlap | speaker-double-booking.
    #[serde(rename = "type")]
    pub conflict_type: String,
    pub item_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiffEntry {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReviewReport {
    pub can_publish: bool,
    pub included_count: usize,
    pub excluded: Vec<ExcludedItem>,
    pub conflicts: Vec<ConflictItem>,
    pub added: Vec<DiffEntry>,
    pub updated: Vec<DiffEntry>,
    pub removed: Vec<DiffEntry>,
}

/// Frozen publish candidates (directory-resolved speakers, resolved room names)
/// plus the review report.
#[derive(Debug, Clone)]
pub struct ReviewOutput {
    pub report: ReviewReport,
    pub frozen: Vec<EventScheduleItem>,
}

fn speaker_key(s: &ScheduleSpeaker) -> String {
    s.id.as_deref()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| s.name.trim().to_lowercase())
}

fn item_times(item: &EventScheduleItem) -> Option<Result<(i64, i64), ConflictItem>> {
    match (&item.starts_at, &item.ends_at) {
        (Some(s), Some(e)) => match (parse_rfc3339_epoch(s), parse_rfc3339_epoch(e)) {
            (Some(ss), Some(ee)) if ss < ee => Some(Ok((ss, ee))),
            _ => Some(Err(ConflictItem {
                conflict_type: "invalid-range".into(),
                item_id: Some(item.id.clone()),
                detail: format!("invalid time range: {s} .. {e}"),
            })),
        },
        (None, None) => None,
        _ => Some(Err(ConflictItem {
            conflict_type: "missing-fields".into(),
            item_id: Some(item.id.clone()),
            detail: "scheduled item needs both startsAt and endsAt".into(),
        })),
    }
}

fn fingerprint(item: &EventScheduleItem) -> serde_json::Value {
    serde_json::json!({
        "title": item.title,
        "description": item.description,
        "speakers": item.speakers,
        "roomIds": item.room_ids,
        "allRooms": item.all_rooms,
        "formatId": item.format_id,
        "track": item.track,
        "startsAt": item.starts_at,
        "endsAt": item.ends_at,
        "sessionType": item.session_type,
        "isServiceSession": item.is_service_session,
    })
}

/// Compute the review report and frozen candidates for a publish.
///
/// - Inactive items are excluded as drafts (never block publish).
/// - Active items must be complete and conflict-free, else publish is blocked.
/// - Speaker details for directory-linked items are re-resolved from the
///   directory at publish time (freeze); missing directory ids are conflicts.
/// - Linked talk formats must exist and be active (unknown/archived = conflict).
pub fn review_snapshot(
    rooms: &[EventRoom],
    formats: &[EventFormat],
    items: &[EventScheduleItem],
    directory: &HashMap<String, ScheduleSpeaker>,
    previous: &[EventScheduleItem],
) -> ReviewOutput {
    let room_name: HashMap<&str, &EventRoom> = rooms.iter().map(|r| (r.id.as_str(), r)).collect();
    let format_name: HashMap<&str, &EventFormat> =
        formats.iter().map(|f| (f.id.as_str(), f)).collect();
    let active_rooms: Vec<&EventRoom> = {
        let mut v: Vec<&EventRoom> = rooms.iter().filter(|r| !r.archived).collect();
        v.sort_by_key(|r| r.display_order);
        v
    };

    let mut excluded = Vec::new();
    let mut conflicts = Vec::new();
    let mut candidates: Vec<(EventScheduleItem, (i64, i64))> = Vec::new();
    let mut frozen: Vec<EventScheduleItem> = Vec::new();

    for item in items {
        if !item.is_active {
            excluded.push(ExcludedItem {
                id: item.id.clone(),
                title: item.title.clone(),
                reason: "draft".into(),
            });
            continue;
        }

        let mut frozen_item = item.clone();

        // Format resolution: linked format must exist and be active.
        if let Some(fid) = item.format_id.as_deref().filter(|f| !f.is_empty()) {
            match format_name.get(fid) {
                Some(f) if !f.archived => frozen_item.format = Some(f.name.clone()),
                Some(_) => {
                    conflicts.push(ConflictItem {
                        conflict_type: "unknown-format".into(),
                        item_id: Some(item.id.clone()),
                        detail: format!("talk format {fid} is archived"),
                    });
                    continue;
                }
                None => {
                    conflicts.push(ConflictItem {
                        conflict_type: "unknown-format".into(),
                        item_id: Some(item.id.clone()),
                        detail: format!("talk format {fid} not found"),
                    });
                    continue;
                }
            }
        }

        // Speaker resolution: speaker_ids win; unknown ids block.
        if !item.speaker_ids.is_empty() {
            let mut resolved = Vec::new();
            let mut missing = Vec::new();
            for id in &item.speaker_ids {
                match directory.get(id) {
                    Some(sp) => resolved.push(sp.clone()),
                    None => missing.push(id.clone()),
                }
            }
            if !missing.is_empty() {
                conflicts.push(ConflictItem {
                    conflict_type: "unknown-speaker".into(),
                    item_id: Some(item.id.clone()),
                    detail: format!("speaker ids not in directory: {}", missing.join(", ")),
                });
                continue;
            }
            frozen_item.speakers = resolved;
        }

        // Rooms resolution.
        if item.all_rooms {
            if active_rooms.is_empty() {
                conflicts.push(ConflictItem {
                    conflict_type: "missing-fields".into(),
                    item_id: Some(item.id.clone()),
                    detail: "allRooms is set but the event has no active rooms".into(),
                });
                continue;
            }
            frozen_item.room_ids = Vec::new();
            frozen_item.rooms = Some(active_rooms.iter().map(|r| r.name.clone()).collect());
        } else {
            if item.room_ids.is_empty() {
                conflicts.push(ConflictItem {
                    conflict_type: "missing-fields".into(),
                    item_id: Some(item.id.clone()),
                    detail: "item is included in publication but has no room assignment".into(),
                });
                continue;
            }
            let mut unknown = Vec::new();
            let mut names = Vec::new();
            for id in &item.room_ids {
                match room_name.get(id.as_str()) {
                    Some(r) if !r.archived => names.push(r.name.clone()),
                    Some(_) => unknown.push(format!("{id} (archived)")),
                    None => unknown.push(id.clone()),
                }
            }
            if !unknown.is_empty() {
                conflicts.push(ConflictItem {
                    conflict_type: "unknown-room".into(),
                    item_id: Some(item.id.clone()),
                    detail: format!("room ids not usable: {}", unknown.join(", ")),
                });
                continue;
            }
            frozen_item.rooms = Some(names);
        }
        frozen_item.room = frozen_item.rooms.as_ref().and_then(|r| r.first().cloned());

        if item.title.trim().is_empty() {
            conflicts.push(ConflictItem {
                conflict_type: "missing-fields".into(),
                item_id: Some(item.id.clone()),
                detail: "title is required".into(),
            });
            continue;
        }
        let times = match item_times(item) {
            None => {
                conflicts.push(ConflictItem {
                    conflict_type: "missing-fields".into(),
                    item_id: Some(item.id.clone()),
                    detail: "item is included in publication but is unscheduled (missing startsAt/endsAt)".into(),
                });
                continue;
            }
            Some(Err(c)) => {
                conflicts.push(c);
                continue;
            }
            Some(Ok(t)) => t,
        };

        // Overlap checks against already-accepted candidates.
        for (other, other_times) in &candidates {
            let (os, oe) = *other_times;
            let time_overlap = times.0 < oe && os < times.1;
            if !time_overlap {
                continue;
            }
            let rooms_overlap = item.all_rooms
                || other.all_rooms
                || item.room_ids.iter().any(|r| other.room_ids.contains(r));
            if rooms_overlap {
                conflicts.push(ConflictItem {
                    conflict_type: "room-overlap".into(),
                    item_id: Some(item.id.clone()),
                    detail: format!("overlaps \"{}\" in the same room(s)", other.title),
                });
            }
            let keys: Vec<String> = frozen_item.speakers.iter().map(speaker_key).collect();
            let other_keys: Vec<String> = other.speakers.iter().map(speaker_key).collect();
            if keys.iter().any(|k| other_keys.contains(k)) {
                conflicts.push(ConflictItem {
                    conflict_type: "speaker-double-booking".into(),
                    item_id: Some(item.id.clone()),
                    detail: format!("shares a speaker with overlapping \"{}\"", other.title),
                });
            }
        }

        candidates.push((frozen_item.clone(), times));
        frozen.push(frozen_item);
    }

    // Diff vs previously published snapshot (by id, content fingerprint).
    let prev_map: HashMap<&str, &EventScheduleItem> =
        previous.iter().map(|p| (p.id.as_str(), p)).collect();
    let cand_map: HashMap<&str, &EventScheduleItem> =
        frozen.iter().map(|f| (f.id.as_str(), f)).collect();
    let mut added = Vec::new();
    let mut updated = Vec::new();
    for f in &frozen {
        match prev_map.get(f.id.as_str()) {
            None => added.push(DiffEntry {
                id: f.id.clone(),
                title: f.title.clone(),
            }),
            Some(p) => {
                if fingerprint(f) != fingerprint(p) {
                    updated.push(DiffEntry {
                        id: f.id.clone(),
                        title: f.title.clone(),
                    });
                }
            }
        }
    }
    let mut removed = Vec::new();
    for p in previous {
        if !cand_map.contains_key(p.id.as_str()) {
            removed.push(DiffEntry {
                id: p.id.clone(),
                title: p.title.clone(),
            });
        }
    }

    let report = ReviewReport {
        can_publish: conflicts.is_empty(),
        included_count: frozen.len(),
        excluded,
        conflicts,
        added,
        updated,
        removed,
    };
    ReviewOutput { report, frozen }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(id: &str, name: &str, order: i32, archived: bool) -> EventRoom {
        EventRoom {
            id: id.into(),
            event_slug: "e".into(),
            name: name.into(),
            display_order: order,
            archived,
            revision: 1,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn item(
        id: &str,
        title: &str,
        active: bool,
        rooms: &[&str],
        all_rooms: bool,
        start: Option<&str>,
        end: Option<&str>,
        speakers: Vec<ScheduleSpeaker>,
    ) -> EventScheduleItem {
        EventScheduleItem {
            id: id.into(),
            event_slug: "e".into(),
            session_id: None,
            title: title.into(),
            description: None,
            speaker_id: None,
            speaker_name: None,
            speaker_role: None,
            speaker_company: None,
            speaker_avatar: None,
            speaker_social_url: None,
            speakers,
            room: None,
            track: None,
            room_ids: rooms.iter().map(|s| s.to_string()).collect(),
            speaker_ids: Vec::new(),
            all_rooms,
            languages: Vec::new(),
            levels: Vec::new(),
            rooms: None,
            starts_at: start.map(String::from),
            ends_at: end.map(String::from),
            session_type: None,
            format_id: None,
            format: None,
            is_service_session: false,
            is_active: active,
            display_order: 0,
            revision: 1,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn spk(id: &str, name: &str) -> ScheduleSpeaker {
        ScheduleSpeaker {
            id: Some(id.into()),
            name: name.into(),
            role: None,
            company: None,
            avatar: None,
            social_url: None,
        }
    }

    fn types(v: &[ConflictItem]) -> Vec<String> {
        v.iter().map(|c| c.conflict_type.clone()).collect()
    }

    #[test]
    fn parses_rfc3339_with_offsets() {
        let a = parse_rfc3339_epoch("2026-10-21T09:00:00Z").unwrap();
        let b = parse_rfc3339_epoch("2026-10-21T16:00:00+07:00").unwrap();
        assert_eq!(a, b);
        assert_eq!(parse_rfc3339_epoch("2026-10-21T09:00:00.500Z"), Some(a + 0));
        assert!(parse_rfc3339_epoch("2026-10-21").is_none());
        assert!(parse_rfc3339_epoch("2026-13-01T00:00:00Z").is_none());
        assert!(parse_rfc3339_epoch("2026-10-21T09:00:00").is_none());
        assert!(parse_rfc3339_epoch("garbage").is_none());
    }

    #[test]
    fn drafts_excluded_without_blocking() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let items = vec![
            item(
                "a",
                "Talk",
                true,
                &["r1"],
                false,
                Some("2026-10-21T09:00:00Z"),
                Some("2026-10-21T10:00:00Z"),
                vec![spk("s1", "Ada")],
            ),
            item(
                "b",
                "Imported draft",
                false,
                &[],
                false,
                None,
                None,
                vec![spk("s2", "Bob")],
            ),
        ];
        let out = review_snapshot(&rooms, &[], &items, &HashMap::new(), &[]);
        assert!(out.report.can_publish);
        assert_eq!(out.report.included_count, 1);
        assert_eq!(out.report.excluded.len(), 1);
        assert_eq!(out.report.excluded[0].reason, "draft");
        assert!(out.report.conflicts.is_empty());
    }

    #[test]
    fn included_but_unscheduled_blocks() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let items = vec![item(
            "a",
            "Talk",
            true,
            &["r1"],
            false,
            None,
            None,
            vec![spk("s1", "Ada")],
        )];
        let out = review_snapshot(&rooms, &[], &items, &HashMap::new(), &[]);
        assert!(!out.report.can_publish);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["missing-fields".to_string()]
        );
    }

    #[test]
    fn speakerless_nonservice_talk_publishes() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let items = vec![item(
            "a",
            "Talk",
            true,
            &["r1"],
            false,
            Some("2026-10-21T09:00:00Z"),
            Some("2026-10-21T10:00:00Z"),
            vec![],
        )];
        let out = review_snapshot(&rooms, &[], &items, &HashMap::new(), &[]);
        assert!(out.report.can_publish);
        assert_eq!(out.report.included_count, 1);
        assert!(out.report.conflicts.is_empty());
        assert!(out.frozen[0].speakers.is_empty());
    }

    #[test]
    fn room_overlap_and_independent_breaks() {
        let rooms = vec![
            room("r1", "Main", 0, false),
            room("r2", "Workshop", 1, false),
        ];
        let t = ("2026-10-21T09:00:00Z", "2026-10-21T10:00:00Z");
        let mut talk = item(
            "a",
            "Talk",
            true,
            &["r1"],
            false,
            Some(t.0),
            Some(t.1),
            vec![spk("s1", "Ada")],
        );
        talk.is_service_session = true;
        talk.speakers = vec![];
        let mut brk = item(
            "b",
            "Break",
            true,
            &["r1"],
            false,
            Some(t.0),
            Some(t.1),
            vec![],
        );
        brk.is_service_session = true;
        // Independent break in another room at the same time is fine.
        let mut brk2 = item(
            "c",
            "Break 2",
            true,
            &["r2"],
            false,
            Some(t.0),
            Some(t.1),
            vec![],
        );
        brk2.is_service_session = true;

        let out = review_snapshot(
            &rooms,
            &[],
            &[talk.clone(), brk.clone(), brk2],
            &HashMap::new(),
            &[],
        );
        assert!(!out.report.can_publish);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["room-overlap".to_string()]
        );

        // Break moved to r2: no conflict.
        let mut brk_moved = brk.clone();
        brk_moved.room_ids = vec!["r2".into()];
        let out = review_snapshot(&rooms, &[], &[talk, brk_moved], &HashMap::new(), &[]);
        assert!(out.report.can_publish);
    }

    #[test]
    fn all_rooms_session_overlaps_everything() {
        let rooms = vec![
            room("r1", "Main", 0, false),
            room("r2", "Workshop", 1, false),
        ];
        let mut keynote = item(
            "a",
            "Keynote",
            true,
            &[],
            true,
            Some("2026-10-21T09:00:00Z"),
            Some("2026-10-21T10:00:00Z"),
            vec![spk("s1", "Ada")],
        );
        keynote.is_service_session = true;
        keynote.speakers = vec![];
        let mut talk = item(
            "b",
            "Talk",
            true,
            &["r2"],
            false,
            Some("2026-10-21T09:30:00Z"),
            Some("2026-10-21T10:30:00Z"),
            vec![spk("s2", "Bob")],
        );
        talk.is_service_session = true;
        talk.speakers = vec![];

        let out = review_snapshot(&rooms, &[], &[keynote, talk], &HashMap::new(), &[]);
        assert!(!out.report.can_publish);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["room-overlap".to_string()]
        );
    }

    #[test]
    fn speaker_double_booking_across_rooms() {
        let rooms = vec![
            room("r1", "Main", 0, false),
            room("r2", "Workshop", 1, false),
        ];
        let items = vec![
            item(
                "a",
                "Talk A",
                true,
                &["r1"],
                false,
                Some("2026-10-21T09:00:00Z"),
                Some("2026-10-21T10:00:00Z"),
                vec![spk("s1", "Ada")],
            ),
            item(
                "b",
                "Talk B",
                true,
                &["r2"],
                false,
                Some("2026-10-21T09:30:00Z"),
                Some("2026-10-21T10:30:00Z"),
                vec![spk("s1", "Ada")],
            ),
        ];
        let out = review_snapshot(&rooms, &[], &items, &HashMap::new(), &[]);
        assert!(!out.report.can_publish);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["speaker-double-booking".to_string()]
        );
    }

    #[test]
    fn boundary_touching_ranges_do_not_conflict() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let items = vec![
            item(
                "a",
                "Talk A",
                true,
                &["r1"],
                false,
                Some("2026-10-21T09:00:00Z"),
                Some("2026-10-21T10:00:00Z"),
                vec![spk("s1", "Ada")],
            ),
            item(
                "b",
                "Talk B",
                true,
                &["r1"],
                false,
                Some("2026-10-21T10:00:00Z"),
                Some("2026-10-21T11:00:00Z"),
                vec![spk("s2", "Bob")],
            ),
        ];
        let out = review_snapshot(&rooms, &[], &items, &HashMap::new(), &[]);
        assert!(out.report.can_publish);
    }

    #[test]
    fn invalid_and_partial_ranges_block() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let mut a = item(
            "a",
            "A",
            true,
            &["r1"],
            false,
            Some("2026-10-21T10:00:00Z"),
            Some("2026-10-21T09:00:00Z"),
            vec![spk("s1", "Ada")],
        );
        a.is_service_session = true;
        a.speakers = vec![];
        let out = review_snapshot(
            &rooms,
            &[],
            &[a],
            &HashMap::new(),
            &[] as &[EventScheduleItem],
        );
        assert_eq!(
            types(&out.report.conflicts),
            vec!["invalid-range".to_string()]
        );

        let mut b = item(
            "b",
            "B",
            true,
            &["r1"],
            false,
            Some("2026-10-21T10:00:00Z"),
            None,
            vec![],
        );
        b.is_service_session = true;
        let out = review_snapshot(&rooms, &[], &[b], &HashMap::new(), &[]);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["missing-fields".to_string()]
        );
    }

    #[test]
    fn freezes_directory_speakers_and_flags_unknown() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let mut a = item(
            "a",
            "Talk",
            true,
            &["r1"],
            false,
            Some("2026-10-21T09:00:00Z"),
            Some("2026-10-21T10:00:00Z"),
            vec![spk("s1", "Stale Name")],
        );
        a.speaker_ids = vec!["s1".into()];
        let mut dir = HashMap::new();
        dir.insert(
            "s1".to_string(),
            ScheduleSpeaker {
                id: Some("s1".into()),
                name: "Fresh Name".into(),
                role: Some("DevRel".into()),
                company: None,
                avatar: None,
                social_url: None,
            },
        );
        let out = review_snapshot(&rooms, &[], &[a.clone()], &dir, &[]);
        assert!(out.report.can_publish);
        assert_eq!(out.frozen[0].speakers[0].name, "Fresh Name");

        a.speaker_ids = vec!["gone".into()];
        let out = review_snapshot(&rooms, &[], &[a], &HashMap::new(), &[]);
        assert!(!out.report.can_publish);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["unknown-speaker".to_string()]
        );
    }

    #[test]
    fn linked_format_must_exist_and_freezes_name() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let formats = vec![EventFormat {
            id: "f1".into(),
            event_slug: "e".into(),
            name: "Lightning Talk".into(),
            display_order: 0,
            archived: false,
            revision: 1,
            created_at: String::new(),
            updated_at: String::new(),
        }];
        let mut a = item(
            "a",
            "Talk",
            true,
            &["r1"],
            false,
            Some("2026-10-21T09:00:00Z"),
            Some("2026-10-21T10:00:00Z"),
            vec![spk("s1", "Ada")],
        );
        a.format_id = Some("f1".into());
        let out = review_snapshot(&rooms, &formats, &[a.clone()], &HashMap::new(), &[]);
        assert!(out.report.can_publish);
        assert_eq!(out.frozen[0].format.as_deref(), Some("Lightning Talk"));

        // Unknown or archived format blocks publish; absent format is fine.
        a.format_id = Some("nope".into());
        let out = review_snapshot(&rooms, &formats, &[a.clone()], &HashMap::new(), &[]);
        assert!(!out.report.can_publish);
        assert_eq!(
            types(&out.report.conflicts),
            vec!["unknown-format".to_string()]
        );

        let mut archived_fmt = formats.clone();
        archived_fmt[0].archived = true;
        a.format_id = Some("f1".into());
        let out = review_snapshot(&rooms, &archived_fmt, &[a.clone()], &HashMap::new(), &[]);
        assert!(!out.report.can_publish);

        a.format_id = None;
        let out = review_snapshot(&rooms, &formats, &[a], &HashMap::new(), &[]);
        assert!(out.report.can_publish);
        assert_eq!(out.frozen[0].format, None);
    }

    #[test]
    fn diff_added_updated_removed() {
        let rooms = vec![room("r1", "Main", 0, false)];
        let mut prev_a = item(
            "a",
            "Old Title",
            true,
            &["r1"],
            false,
            Some("2026-10-21T09:00:00Z"),
            Some("2026-10-21T10:00:00Z"),
            vec![spk("s1", "Ada")],
        );
        prev_a.is_active = true;
        let mut prev_c = item(
            "c",
            "Removed",
            true,
            &["r1"],
            false,
            Some("2026-10-21T11:00:00Z"),
            Some("2026-10-21T12:00:00Z"),
            vec![spk("s2", "Bob")],
        );
        prev_c.is_service_session = true;
        prev_c.speakers = vec![];

        let next_a = item(
            "a",
            "New Title",
            true,
            &["r1"],
            false,
            Some("2026-10-21T09:00:00Z"),
            Some("2026-10-21T10:00:00Z"),
            vec![spk("s1", "Ada")],
        );
        let next_b = item(
            "b",
            "Added",
            true,
            &["r1"],
            false,
            Some("2026-10-21T13:00:00Z"),
            Some("2026-10-21T14:00:00Z"),
            vec![spk("s3", "Cara")],
        );

        let out = review_snapshot(
            &rooms,
            &[],
            &[next_a, next_b],
            &HashMap::new(),
            &[prev_a, prev_c],
        );
        assert!(out.report.can_publish);
        assert_eq!(out.report.added.len(), 1);
        assert_eq!(out.report.added[0].id, "b");
        assert_eq!(out.report.updated.len(), 1);
        assert_eq!(out.report.updated[0].id, "a");
        assert_eq!(out.report.removed.len(), 1);
        assert_eq!(out.report.removed[0].id, "c");
    }
}
