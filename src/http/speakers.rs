use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::{json_success_cors, with_cors};
use crate::speakers::repository::{
    CreateSpeakerError, DeleteSpeakerError, ReorderSpeakersError, SpeakerRepository,
    UpdateSpeakerError,
};
use crate::speakers::types::{
    EventSpeakerCreate, EventSpeakerReorderBody, EventSpeakerUpdate, EventSpeakersResponse,
};

const MAX_EVENT_SLUG_LEN: usize = 100;

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

/// GET /api/events/:eventSlug/speakers — public active speakers list for carousels.
pub async fn handle_public_speakers(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SpeakerRepository::new(db);

    let speakers = repo
        .list_public_speakers(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = EventSpeakersResponse {
        event_slug,
        speakers: &speakers,
    };
    let resp = Response::from_json(&body)?;
    with_cors(resp, &config.allowed_origins)
}

/// GET /api/admin/events/:eventSlug/speakers — admin listing of all speakers (including inactive).
pub async fn handle_admin_list_speakers(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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
    let repo = SpeakerRepository::new(db);

    let speakers = repo
        .list_speakers(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = EventSpeakersResponse {
        event_slug,
        speakers: &speakers,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/admin/speakers — admin listing of all speakers across all events.
pub async fn handle_admin_list_all_speakers(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SpeakerRepository::new(db);

    let speakers = repo
        .list_all_speakers()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = serde_json::json!({
        "speakers": speakers,
    });
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/speakers — create a new speaker.
pub async fn handle_admin_create_speaker(
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

    let input: EventSpeakerCreate = req
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {e}")))?;

    if input.name.trim().is_empty() || input.name.trim().len() > 100 {
        return Err(AppError::BadRequest(
            "Speaker name must be between 1 and 100 characters".to_string(),
        )
        .into());
    }
    if input.role.trim().is_empty() || input.role.trim().len() > 100 {
        return Err(AppError::BadRequest(
            "Speaker role must be between 1 and 100 characters".to_string(),
        )
        .into());
    }
    if input.photo_url.trim().is_empty() || input.photo_url.trim().len() > 500 {
        return Err(AppError::BadRequest(
            "Photo URL must be between 1 and 500 characters".to_string(),
        )
        .into());
    }
    if input.talk_title.trim().is_empty() || input.talk_title.trim().len() > 200 {
        return Err(AppError::BadRequest(
            "Talk title must be between 1 and 200 characters".to_string(),
        )
        .into());
    }

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SpeakerRepository::new(db);

    let created = repo
        .create_speaker(event_slug, input)
        .await
        .map_err(|e| match e {
            CreateSpeakerError::DuplicateName => AppError::Conflict(
                "A speaker with this name already exists for this event".to_string(),
            ),
            CreateSpeakerError::SpeakerLimit => {
                AppError::Conflict("Maximum speaker limit reached for this event".to_string())
            }
            CreateSpeakerError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    json_success_cors(&created, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/speakers/:speakerId — update an existing speaker.
pub async fn handle_admin_update_speaker(
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
    let speaker_id = ctx
        .param("speakerId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: speakerId".to_string()))?;
    validate_event_slug(event_slug)?;

    let input: EventSpeakerUpdate = req
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {e}")))?;

    if input.name.trim().is_empty() || input.name.trim().len() > 100 {
        return Err(AppError::BadRequest(
            "Speaker name must be between 1 and 100 characters".to_string(),
        )
        .into());
    }
    if input.role.trim().is_empty() || input.role.trim().len() > 100 {
        return Err(AppError::BadRequest(
            "Speaker role must be between 1 and 100 characters".to_string(),
        )
        .into());
    }
    if input.photo_url.trim().is_empty() || input.photo_url.trim().len() > 500 {
        return Err(AppError::BadRequest(
            "Photo URL must be between 1 and 500 characters".to_string(),
        )
        .into());
    }
    if input.talk_title.trim().is_empty() || input.talk_title.trim().len() > 200 {
        return Err(AppError::BadRequest(
            "Talk title must be between 1 and 200 characters".to_string(),
        )
        .into());
    }

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SpeakerRepository::new(db);

    let updated = repo
        .update_speaker(event_slug, speaker_id, input)
        .await
        .map_err(|e| match e {
            UpdateSpeakerError::NotFound => AppError::NotFound(format!(
                "Speaker '{speaker_id}' not found for event {event_slug}"
            )),
            UpdateSpeakerError::DuplicateName => AppError::Conflict(
                "Another speaker with this name already exists for this event".to_string(),
            ),
            UpdateSpeakerError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    json_success_cors(&updated, &config.allowed_origins, origin.as_deref())
}

/// DELETE /api/admin/events/:eventSlug/speakers/:speakerId — delete a speaker.
pub async fn handle_admin_delete_speaker(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    let speaker_id = ctx
        .param("speakerId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: speakerId".to_string()))?;
    validate_event_slug(event_slug)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SpeakerRepository::new(db);

    repo.delete_speaker(event_slug, speaker_id)
        .await
        .map_err(|e| match e {
            DeleteSpeakerError::NotFound => AppError::NotFound(format!(
                "Speaker '{speaker_id}' not found for event {event_slug}"
            )),
            DeleteSpeakerError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    let remaining = repo
        .list_speakers(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = EventSpeakersResponse {
        event_slug,
        speakers: &remaining,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/speakers/order — reorder speakers.
pub async fn handle_admin_reorder_speakers(
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

    let input: EventSpeakerReorderBody = req
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = SpeakerRepository::new(db);

    let speakers = repo
        .reorder_speakers(event_slug, &input.items)
        .await
        .map_err(|e| match e {
            ReorderSpeakersError::InvalidList => {
                AppError::BadRequest("Invalid speaker order list".to_string())
            }
            ReorderSpeakersError::Db(e) => AppError::Internal(e.to_string()),
        })?;

    let body = EventSpeakersResponse {
        event_slug,
        speakers: &speakers,
    };
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/speakers/upload — Upload a speaker photo or topic banner (admin-only).
pub async fn handle_admin_upload_speaker_asset(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    use sha2::{Digest, Sha256};

    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let event_slug = ctx
        .param("eventSlug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: eventSlug".to_string()))?;
    validate_event_slug(event_slug)?;

    let asset_type = req
        .url()
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(k, _)| k == "type")
                .map(|(_, v)| v.to_string())
        })
        .unwrap_or_else(|| "photo".to_string());

    let folder = if asset_type == "topic" {
        format!("{event_slug}/topics")
    } else {
        format!("{event_slug}/speakers")
    };

    let bytes = req
        .bytes()
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read request body: {e}")))?;

    let format = crate::http::sponsors::detect_mockup_format(&bytes)?;

    let timestamp = (js_sys::Date::now() / 1000.0) as u64;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = hasher.finalize();
    let hash_hex: String = hash.iter().map(|b| format!("{:02x}", b)).collect();
    let hash_prefix = &hash_hex[..12];

    let key = format!("{folder}/{hash_prefix}_{timestamp}.{}", format.ext);

    let bucket = ctx.bucket("AVATAR_BUCKET")?;
    bucket
        .put(&key, bytes)
        .http_metadata(worker::HttpMetadata {
            content_type: Some(format.mime.to_string()),
            ..Default::default()
        })
        .execute()
        .await?;

    let public_url = format!(
        "{}/{}",
        config.avatar_public_base_url.trim_end_matches('/'),
        key
    );

    console_log!("speaker asset uploaded: event={event_slug} key={key} url={public_url}");

    let body = serde_json::json!({
        "url": public_url,
        "key": key
    });
    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}
