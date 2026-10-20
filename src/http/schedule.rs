use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::json_success_cors;
use crate::schedule::repository::ScheduleRepository;
use crate::schedule::types::{
    CreateScheduleItem, EventScheduleResponse, ReorderScheduleBody, SyncSessionizeRequest,
    UpdateScheduleItem,
};
use crate::sessionize::client::SessionizeClient;

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

/// GET /api/events/:eventSlug/schedule — public active schedule list
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

    let schedule = repo
        .list_public_schedule(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = EventScheduleResponse {
        event_slug,
        schedule: &schedule,
    };

    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/admin/events/:eventSlug/schedule — admin listing of all schedule items
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

    let schedule = repo
        .list_admin_schedule(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = EventScheduleResponse {
        event_slug,
        schedule: &schedule,
    };

    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/schedule — create schedule item manually
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

    let input = req
        .json::<CreateScheduleItem>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".to_string()).into());
    }

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let item = repo
        .create_schedule_item(event_slug, input)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    json_success_cors(&item, &config.allowed_origins, origin.as_deref())
}

/// PUT /api/admin/events/:eventSlug/schedule/:itemId — update schedule item
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

    let input = req
        .json::<UpdateScheduleItem>()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid request body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let updated = repo
        .update_schedule_item(event_slug, item_id, input)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Schedule item {item_id} not found")))?;

    json_success_cors(&updated, &config.allowed_origins, origin.as_deref())
}

/// DELETE /api/admin/events/:eventSlug/schedule/:itemId — delete schedule item
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

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let deleted = repo
        .delete_schedule_item(event_slug, item_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = serde_json::json!({ "success": deleted });
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
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let schedule = repo
        .list_admin_schedule(event_slug)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resp_body = EventScheduleResponse {
        event_slug,
        schedule: &schedule,
    };

    json_success_cors(&resp_body, &config.allowed_origins, origin.as_deref())
}

/// POST /api/admin/events/:eventSlug/schedule/sync-sessionize — sync sessions from Sessionize into schedule
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
        .unwrap_or(SyncSessionizeRequest {
            session_ids: None,
            overwrite: Some(true),
            fresh: Some(false),
        });

    let client = SessionizeClient::default();
    let sessionize_data = client
        .get_data(body.fresh.unwrap_or(false))
        .await
        .map_err(|e| AppError::Internal(format!("Failed to fetch from Sessionize: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = ScheduleRepository::new(db);

    let result = repo
        .sync_from_sessionize(
            event_slug,
            sessionize_data.sessions,
            body.session_ids,
            body.overwrite.unwrap_or(true),
        )
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    json_success_cors(&result, &config.allowed_origins, origin.as_deref())
}
