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
