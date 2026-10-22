use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::{json_public_cached, json_success_cors};
use crate::schedule::repository::{
    ItemMutationError, PublishError, RoomMutationError, ScheduleRepository,
};
use crate::schedule::review::{review_snapshot, ReviewOutput};
use crate::schedule::types::{
    AdminScheduleResponse, CreateFormatBody, CreateRoomBody, CreateScheduleItem,
    EventFormatsResponse, EventRoomsResponse, EventScheduleResponse, PublicationSnapshotDoc,
    PublishBody, PublishResponse, ReorderScheduleBody, ScheduleSpeaker, SyncSessionizeRequest,
    UpdateFormatBody, UpdateRoomBody, UpdateScheduleItem,
};
use crate::sessionize::client::SessionizeClient;

const MAX_EVENT_SLUG_LEN: usize = 100;
const MAX_SESSION_SPEAKERS: usize = 20;
const MAX_METADATA_VALUES: usize = 20;
const MAX_METADATA_VALUE_LEN: usize = 100;

fn internal(e: impl std::fmt::Display) -> AppError {
    AppError::Internal(e.to_string())
}

fn validate_event_slug(slug: &str) -> Result<(), AppError> {
    if slug.is_empty() || slug.len() > MAX_EVENT_SLUG_LEN {
        return Err(AppError::BadRequest("Invalid eventSlug".to_string()));
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(AppError::BadRequest("Invalid eventSlug format".to_string()));
    }
    Ok(())
}

/// Trim + bound a languages/levels array at the trust boundary. Returns the
/// normalized list (caller stores it) or a 400.
fn normalize_metadata_list(label: &str, values: Vec<String>) -> Result<Vec<String>, AppError> {
    if values.len() > MAX_METADATA_VALUES {
        return Err(AppError::BadRequest(format!(
            "{label} may contain at most {MAX_METADATA_VALUES} values"
        )));
    }
    let mut out = Vec::with_capacity(values.len());
    for v in values {
        let t = v.trim().to_string();
        if t.is_empty() || t.len() > MAX_METADATA_VALUE_LEN {
            return Err(AppError::BadRequest(format!(
                "each {label} value must be between 1 and {MAX_METADATA_VALUE_LEN} characters"
            )));
        }
        out.push(t);
    }
    Ok(out)
}

fn validate_speakers(speakers: &[ScheduleSpeaker]) -> Result<(), AppError> {
    if speakers.len() > MAX_SESSION_SPEAKERS {
        return Err(AppError::BadRequest(format!(
            "speakers may contain at most {MAX_SESSION_SPEAKERS} entries"
        )));
    }
    for s in speakers {
        let name = s.name.trim();
        if name.is_empty() || name.len() > 100 {
            return Err(AppError::BadRequest(
                "each speaker name must be between 1 and 100 characters".to_string(),
            ));
        }
        for (label, url) in [
            ("speaker avatar", &s.avatar),
            ("speaker socialUrl", &s.social_url),
        ] {
            if let Some(u) = url.as_deref() {
                let u = u.trim();
                if !(u.is_empty() || u.starts_with("http://") || u.starts_with("https://")) {
                    return Err(AppError::BadRequest(format!(
                        "{label} must be an http:// or https:// URL"
                    )));
                }
                if u.len() > 500 {
                    return Err(AppError::BadRequest(format!(
                        "{label} must be at most 500 characters"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn item_mutation_err(item_id: &str, e: ItemMutationError) -> AppError {
    match e {
        ItemMutationError::NotFound => {
            AppError::NotFound(format!("Schedule item {item_id} not found"))
        }
        ItemMutationError::StaleRevision => {
            AppError::Conflict("stale revision; refetch the schedule and retry".to_string())
        }
    }
}

fn room_mutation_err(label: &str, id: &str, e: RoomMutationError) -> AppError {
    match e {
        RoomMutationError::NotFound => AppError::NotFound(format!("{label} {id} not found")),
        RoomMutationError::StaleRevision => {
            AppError::Conflict("stale revision; refetch the list and retry".to_string())
        }
        RoomMutationError::DuplicateName => {
            AppError::Conflict(format!("{label} name already exists for this event"))
        }
        RoomMutationError::Occupied(items) => AppError::Conflict(format!(
            "cannot archive {label} {id}: still assigned to {} item(s): {}",
            items.len(),
            items
                .iter()
                .map(|(_, t)| format!("\"{t}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Validate explicit room ids (must exist and be active).
fn validate_room_ids(
    rooms: &[crate::schedule::types::EventRoom],
    rids: &[String],
) -> Result<(), AppError> {
    for rid in rids {
        match rooms.iter().find(|r| &r.id == rid) {
            Some(r) if !r.archived => {}
            Some(_) => {
                return Err(AppError::BadRequest(format!("room {rid} is archived")));
            }
            None => return Err(AppError::BadRequest(format!("unknown roomId: {rid}"))),
        }
    }
    Ok(())
}

/// Resolve create-input references: speakerIds -> speakers, room name -> id,
/// format existence. Mutates `input` so the repository stores resolved values.
async fn resolve_create_input(
    repo: &ScheduleRepository,
    event_slug: &str,
    input: &mut CreateScheduleItem,
) -> Result<(), AppError> {
    let rooms = repo.list_rooms(event_slug).await.map_err(internal)?;
    if input.all_rooms == Some(true) {
        input.room_ids = Some(Vec::new());
    } else if let Some(ref rids) = input.room_ids {
        validate_room_ids(&rooms, rids)?;
    } else if let Some(ref name) = input.room {
        let n = name.trim();
        if !n.is_empty() {
            match rooms.iter().find(|r| r.name.eq_ignore_ascii_case(n)) {
                Some(r) if !r.archived => input.room_ids = Some(vec![r.id.clone()]),
                Some(_) => {
                    return Err(AppError::BadRequest(format!("room \"{n}\" is archived")));
                }
                None => {
                    return Err(AppError::BadRequest(format!(
                        "unknown room \"{n}\"; create it first via /rooms"
                    )));
                }
            }
        }
    }
    if let Some(ref fid) = input.format_id {
        if !fid.is_empty() {
            let formats = repo.list_formats(event_slug).await.map_err(internal)?;
            match formats.iter().find(|f| &f.id == fid) {
                Some(f) if !f.archived => {}
                Some(_) => {
                    return Err(AppError::BadRequest(format!("format {fid} is archived")));
                }
                None => {
                    return Err(AppError::BadRequest(format!("unknown formatId: {fid}")));
                }
            }
        }
    }
    if let Some(ref ids) = input.speaker_ids {
        let dir = repo
            .list_directory_speakers(event_slug)
            .await
            .map_err(internal)?;
        let mut speakers = Vec::new();
        for id in ids {
            let mut sp = dir
                .get(id)
                .cloned()
                .ok_or_else(|| AppError::BadRequest(format!("unknown speakerId: {id}")))?;
            sp.id = Some(id.clone());
            speakers.push(sp);
        }
        validate_speakers(&speakers)?;
        input.speakers = Some(speakers);
    }
    Ok(())
}

/// Resolve update-input references (only fields explicitly provided).
async fn resolve_update_input(
    repo: &ScheduleRepository,
    event_slug: &str,
    input: &mut UpdateScheduleItem,
) -> Result<(), AppError> {
    let rooms = repo.list_rooms(event_slug).await.map_err(internal)?;
    if input.all_rooms == Some(true) {
        input.room_ids = Some(Vec::new());
    } else if let Some(ref rids) = input.room_ids {
        validate_room_ids(&rooms, rids)?;
    } else if let Some(ref name) = input.room {
        let n = name.trim();
        if !n.is_empty() {
            match rooms.iter().find(|r| r.name.eq_ignore_ascii_case(n)) {
                Some(r) if !r.archived => input.room_ids = Some(vec![r.id.clone()]),
                Some(_) => {
                    return Err(AppError::BadRequest(format!("room \"{n}\" is archived")));
                }
                None => {
                    return Err(AppError::BadRequest(format!(
                        "unknown room \"{n}\"; create it first via /rooms"
                    )));
                }
            }
        }
    }
    if let Some(ref fid) = input.format_id {
        if !fid.is_empty() {
            let formats = repo.list_formats(event_slug).await.map_err(internal)?;
            match formats.iter().find(|f| &f.id == fid) {
                Some(f) if !f.archived => {}
                Some(_) => {
                    return Err(AppError::BadRequest(format!("format {fid} is archived")));
                }
                None => {
                    return Err(AppError::BadRequest(format!("unknown formatId: {fid}")));
                }
            }
        }
    }
    if let Some(ref ids) = input.speaker_ids {
        let dir = repo
            .list_directory_speakers(event_slug)
            .await
            .map_err(internal)?;
        let mut speakers = Vec::new();
        for id in ids {
            let mut sp = dir
                .get(id)
                .cloned()
                .ok_or_else(|| AppError::BadRequest(format!("unknown speakerId: {id}")))?;
            sp.id = Some(id.clone());
            speakers.push(sp);
        }
        validate_speakers(&speakers)?;
        input.speakers = Some(speakers);
    } else if let Some(ref speakers) = input.speakers {
        validate_speakers(speakers)?;
    }
    Ok(())
}

/// Shared review/publish assembly. The workspace revision is read BEFORE the
/// data so any mutation landing after this point bumps the revision and fails
/// the publish guard — a token can never certify data newer than itself.
async fn assemble_review(
    repo: &ScheduleRepository,
    event_slug: &str,
) -> Result<(ReviewOutput, i64, Option<String>), AppError> {
    let revision = repo
        .workspace_revision(event_slug)
        .await
        .map_err(internal)?;
    let rooms = repo.list_rooms(event_slug).await.map_err(internal)?;
    let formats = repo.list_formats(event_slug).await.map_err(internal)?;
    let items = repo
        .list_admin_schedule(event_slug)
        .await
        .map_err(internal)?;
    let directory = repo
        .list_directory_speakers(event_slug)
        .await
        .map_err(internal)?;
    let publication = repo.get_publication(event_slug).await.map_err(internal)?;
    let previous = publication
        .as_ref()
        .and_then(|p| serde_json::from_str::<PublicationSnapshotDoc>(&p.snapshot_json).ok())
        .map(|d| d.schedule)
        .unwrap_or_default();
    let out = review_snapshot(&rooms, &formats, &items, &directory, &previous);
    Ok((out, revision, publication.map(|p| p.published_at)))
}

/// Sort frozen items the way the legacy public listing ordered them
/// (startsAt ASC with NULLs first, then displayOrder).
fn sort_public(items: &mut [crate::schedule::types::EventScheduleItem]) {
    items.sort_by(|a, b| {
        a.starts_at
            .as_deref()
            .unwrap_or("")
            .cmp(b.starts_at.as_deref().unwrap_or(""))
            .then(a.display_order.cmp(&b.display_order))
    });
}

/// GET /api/events/:eventSlug/schedule — public, serves the published snapshot
/// only. Drafts are never visible here. Response shape unchanged.
pub async fn handle_public_schedule(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let origin = req.headers().get("Origin").ok().flatten();
    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let mut schedule = match repo.get_publication(event_slug).await.map_err(internal)? {
        Some(p) => {
            serde_json::from_str::<PublicationSnapshotDoc>(&p.snapshot_json)
                .map_err(|e| AppError::Internal(format!("corrupt publication snapshot: {e}")))?
                .schedule
        }
        None => Vec::new(),
    };
    sort_public(&mut schedule);

    let body = EventScheduleResponse {
        event_slug,
        schedule: &schedule,
    };
    json_public_cached(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/admin/events/:eventSlug/schedule — admin listing of all items
/// (drafts + included), the workspace revision (review/publish token) and the
/// last publication time.
pub async fn handle_admin_list_schedule(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    // Revision first: a mutation landing after this read bumps the revision,
    // so the revision handed to the editor never certifies newer data.
    let revision = repo
        .workspace_revision(event_slug)
        .await
        .map_err(internal)?;
    let schedule = repo
        .list_admin_schedule(event_slug)
        .await
        .map_err(internal)?;
    let published_at = repo
        .get_publication(event_slug)
        .await
        .map_err(internal)?
        .map(|p| p.published_at);

    let body = AdminScheduleResponse {
        event_slug,
        schedule: &schedule,
        revision,
        published_at,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/schedule — create schedule item (draft or
/// included). Never public until an explicit publish.
pub async fn handle_admin_create_schedule(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let mut input = req
        .json::<CreateScheduleItem>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".to_string()).into());
    }
    if let Some(ref speakers) = input.speakers {
        validate_speakers(speakers)?;
    }
    input.languages = match input.languages.take() {
        Some(v) => Some(normalize_metadata_list("languages", v)?),
        None => None,
    };
    input.levels = match input.levels.take() {
        Some(v) => Some(normalize_metadata_list("levels", v)?),
        None => None,
    };

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    resolve_create_input(&repo, event_slug, &mut input).await?;

    let item = repo
        .create_schedule_item(event_slug, input)
        .await
        .map_err(internal)?;

    json_success_cors(&item, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/schedule/:itemId — update with revision CAS.
pub async fn handle_admin_update_schedule(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let item_id = ctx
        .param("itemId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: itemId".to_string()))?;

    let mut input = req
        .json::<UpdateScheduleItem>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    if let Some(ref title) = input.title {
        if title.trim().is_empty() {
            return Err(AppError::BadRequest("title cannot be empty".to_string()).into());
        }
    }
    input.languages = match input.languages.take() {
        Some(v) => Some(normalize_metadata_list("languages", v)?),
        None => None,
    };
    input.levels = match input.levels.take() {
        Some(v) => Some(normalize_metadata_list("levels", v)?),
        None => None,
    };

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    resolve_update_input(&repo, event_slug, &mut input).await?;

    let updated = repo
        .update_schedule_item(event_slug, item_id, input)
        .await
        .map_err(internal)?
        .map_err(|e| item_mutation_err(item_id, e))?;

    json_success_cors(&updated, &config.allowed_origins, origin.as_deref())
}

/// DELETE /api/admin/events/:eventSlug/schedule/:itemId?revision=N
pub async fn handle_admin_delete_schedule(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let item_id = ctx
        .param("itemId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: itemId".to_string()))?;

    let revision = req
        .url()
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "revision")
                .and_then(|(_, v)| v.parse::<i32>().ok())
        })
        .ok_or_else(|| {
            AppError::BadRequest("missing or invalid ?revision=N query parameter".to_string())
        })?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    repo.delete_schedule_item(event_slug, item_id, revision)
        .await
        .map_err(internal)?
        .map_err(|e| item_mutation_err(item_id, e))?;

    let body = serde_json::json!({ "success": true });
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/schedule/order — batch reorder schedule items
pub async fn handle_admin_reorder_schedule(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let body = req
        .json::<ReorderScheduleBody>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    repo.reorder_schedule_items(event_slug, body.items)
        .await
        .map_err(internal)?;

    let schedule = repo
        .list_admin_schedule(event_slug)
        .await
        .map_err(internal)?;
    let revision = repo
        .workspace_revision(event_slug)
        .await
        .map_err(internal)?;

    let resp_body = AdminScheduleResponse {
        event_slug,
        schedule: &schedule,
        revision,
        published_at: None,
    };

    json_success_cors(&resp_body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/schedule/sync-sessionize — imports land
/// as unscheduled inactive drafts (upstream timing/rooms ignored).
pub async fn handle_admin_sync_sessionize(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let body = req
        .json::<SyncSessionizeRequest>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let client = SessionizeClient::default();
    let sessionize_data = client
        .get_data(&db, body.fresh.unwrap_or(false))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch from Sessionize: {e}")))?;

    let selected_speaker_ids = body.session_ids.as_ref().map(|ids| {
        sessionize_data
            .sessions
            .iter()
            .filter(|session| ids.contains(&session.id))
            .flat_map(|session| session.speakers.iter().map(|speaker| speaker.id.clone()))
            .collect()
    });
    crate::speakers::repository::SpeakerRepository::new(ctx.d1("DB")?)
        .sync_from_sessionize(
            event_slug,
            sessionize_data.speakers,
            selected_speaker_ids,
            false,
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let result = repo
        .sync_from_sessionize(
            event_slug,
            sessionize_data.sessions,
            body.session_ids,
            body.overwrite.unwrap_or(false),
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    json_success_cors(&result, &config.allowed_origins, origin.as_deref())
}

/// GET /api/admin/events/:eventSlug/schedule/review — dry-run of publish:
/// inclusions, exclusions, conflicts and diff vs the published snapshot.
pub async fn handle_admin_review_schedule(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let (out, revision, published_at) = assemble_review(&repo, event_slug).await?;

    let mut body = serde_json::to_value(&out.report)
        .map_err(|e| AppError::Internal(format!("serialize review: {e}")))?;
    let obj = body
        .as_object_mut()
        .ok_or_else(|| AppError::Internal("review body must be an object".to_string()))?;
    obj.insert("revision".into(), serde_json::json!(revision));
    match published_at {
        Some(p) => obj.insert("publishedAt".into(), serde_json::json!(p)),
        None => obj.remove("publishedAt"),
    };

    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/schedule/publish — atomically snapshot
/// the validated workspace. Body revision must equal the workspace revision
/// the admin reviewed; a mismatch (or any concurrent mutation) fails the
/// guarded D1 batch and the whole publish rolls back.
pub async fn handle_admin_publish_schedule(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let body = req
        .json::<PublishBody>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let (out, _revision, _) = assemble_review(&repo, event_slug).await?;
    if !out.report.conflicts.is_empty() {
        return Err(AppError::Conflict(format!(
            "publish blocked by {} conflict(s); fetch review for details",
            out.report.conflicts.len()
        ))
        .into());
    }

    let mut frozen = out.frozen;
    sort_public(&mut frozen);
    let snapshot = PublicationSnapshotDoc { schedule: frozen };
    let snapshot_json =
        serde_json::to_string(&snapshot).map_err(|e| AppError::Internal(e.to_string()))?;

    let item_count = snapshot.schedule.len();
    let published_at = repo
        .publish_snapshot(event_slug, &snapshot_json, body.revision)
        .await
        .map_err(internal)?
        .map_err(|e| match e {
            PublishError::StaleRevision => AppError::Conflict(
                "stale revision; the workspace changed since review — refetch and retry"
                    .to_string(),
            ),
            PublishError::Other(m) => AppError::Internal(format!("publish failed: {m}")),
        })?;

    let body = PublishResponse {
        published_at,
        item_count,
        revision: body.revision + 1,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/admin/events/:eventSlug/rooms — list rooms (archived included).
pub async fn handle_admin_list_rooms(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    let rooms = repo.list_rooms(event_slug).await.map_err(internal)?;

    let body = EventRoomsResponse {
        event_slug,
        rooms: &rooms,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/rooms — create a room.
pub async fn handle_admin_create_room(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let body = req
        .json::<CreateRoomBody>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::BadRequest(
            "room name must be between 1 and 100 characters".to_string(),
        )
        .into());
    }

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    let room = repo
        .create_room(event_slug, &body)
        .await
        .map_err(internal)?
        .map_err(|e| room_mutation_err("room", "-", e))?;

    json_success_cors(&room, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/rooms/:roomId — rename / reorder / archive
/// with revision CAS. Archive is rejected while items still use the room.
pub async fn handle_admin_update_room(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;
    let room_id = ctx
        .param("roomId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: roomId".to_string()))?;

    let body = req
        .json::<UpdateRoomBody>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    let room = repo
        .update_room(event_slug, room_id, &body)
        .await
        .map_err(internal)?
        .map_err(|e| room_mutation_err("room", room_id, e))?;

    json_success_cors(&room, &config.allowed_origins, origin.as_deref())
}

/// DELETE /api/admin/events/:eventSlug/rooms/:roomId?revision=N — permanent
/// deletion with revision CAS. Rejected (409, item titles included) while any
/// draft item still references the room explicitly or via all_rooms (all_rooms
/// items implicitly reference every active room). Published snapshots are
/// frozen rows and never change; schedule rows are never deleted.
pub async fn handle_admin_delete_room(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;
    let room_id = ctx
        .param("roomId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: roomId".to_string()))?;

    let revision = req
        .url()
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "revision")
                .and_then(|(_, v)| v.parse::<i32>().ok())
        })
        .ok_or_else(|| {
            AppError::BadRequest("missing or invalid ?revision=N query parameter".to_string())
        })?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    repo.delete_room(event_slug, room_id, revision)
        .await
        .map_err(internal)?
        .map_err(|e| match e {
            RoomMutationError::Occupied(items) => AppError::Conflict(format!(
                "cannot permanently delete room {room_id}: still referenced by {} item(s): {}",
                items.len(),
                items
                    .iter()
                    .map(|(_, t)| format!("\"{t}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
            other => room_mutation_err("room", room_id, other),
        })?;

    let body = serde_json::json!({ "success": true });
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/admin/events/:eventSlug/formats — list talk formats (archived included).
pub async fn handle_admin_list_formats(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    let formats = repo.list_formats(event_slug).await.map_err(internal)?;

    let body = EventFormatsResponse {
        event_slug,
        formats: &formats,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/formats — create a talk format.
pub async fn handle_admin_create_format(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let body = req
        .json::<CreateFormatBody>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(AppError::BadRequest(
            "format name must be between 1 and 100 characters".to_string(),
        )
        .into());
    }

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    let format = repo
        .create_format(event_slug, &body)
        .await
        .map_err(internal)?
        .map_err(|e| room_mutation_err("format", "-", e))?;

    json_success_cors(&format, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/formats/:formatId — rename / reorder /
/// archive with revision CAS. Rename keeps the id, so drafts linking the
/// format update automatically and published snapshots stay frozen.
pub async fn handle_admin_update_format(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;
    let format_id = ctx
        .param("formatId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: formatId".to_string()))?;

    let body = req
        .json::<UpdateFormatBody>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);
    let format = repo
        .update_format(event_slug, format_id, &body)
        .await
        .map_err(internal)?
        .map_err(|e| room_mutation_err("format", format_id, e))?;

    json_success_cors(&format, &config.allowed_origins, origin.as_deref())
}
