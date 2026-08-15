use std::collections::HashSet;

use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::{json_success, json_success_cors, with_cors};
use crate::links::repository::LinkRepository;
use crate::links::types::{LinkCreate, LinkItem, LinkPage, LinkUpdate, PageUpdate, ReorderRequest};

const ICON_ALLOWLIST: &[&str] = &[
    "link",
    "github",
    "linkedin",
    "twitter",
    "instagram",
    "youtube",
    "globe",
    "mail",
    "calendar",
    "map-pin",
    "users",
    "external-link",
];
const BACKGROUND_ALLOWLIST: &[&str] = &["dark", "gradient", "mesh"];
const BUTTON_STYLE_ALLOWLIST: &[&str] = &["solid", "outline", "soft"];

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LinksResponse<'a> {
    page: &'a LinkPage,
    items: &'a [LinkItem],
}

/// GET /api/links — public community link page (enabled items only).
pub async fn handle_public_links(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let _origin = req.headers().get("Origin").ok().flatten();

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let page = repo
        .get_page()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let items = repo
        .get_items(true)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = LinksResponse {
        page: &page,
        items: &items,
    };
    let resp = json_success(&body)?;
    with_cors(resp, &config.allowed_origins)
}

/// GET /api/admin/links — admin view of all items.
pub async fn handle_admin_links(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let page = repo
        .get_page()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let items = repo
        .get_items(false)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = LinksResponse {
        page: &page,
        items: &items,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// PUT /api/admin/links/page — update singleton page profile.
pub async fn handle_admin_update_page(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let bytes = req.bytes().await?;
    let input: PageUpdate = serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_page_update(&input)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let updated = repo
        .update_page(&input)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resp = json_success_cors(&updated, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// POST /api/admin/links/items — create a link.
pub async fn handle_admin_create_link(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let bytes = req.bytes().await?;
    let input: LinkCreate = serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_link_create(&input)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let created = repo
        .create_link(&input)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resp = json_success_cors(&created, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// PUT /api/admin/links/items/:linkId — update a link.
pub async fn handle_admin_update_link(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let id = ctx
        .param("linkId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: linkId".to_string()))?;

    let bytes = req.bytes().await?;
    let input: LinkUpdate = serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_link_update(&input)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let updated = repo
        .update_link(id, &input)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Link {id} not found")))?;

    let resp = json_success_cors(&updated, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// DELETE /api/admin/links/items/:linkId — delete a link (204 No Content).
pub async fn handle_admin_delete_link(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let _origin = req.headers().get("Origin").ok().flatten();

    let id = ctx
        .param("linkId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: linkId".to_string()))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let deleted = repo
        .delete_link(id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if !deleted {
        return Err(AppError::NotFound(format!("Link {id} not found")).into());
    }

    let resp = Response::empty()?.with_status(204);
    with_cors(resp, &config.allowed_origins)
}

/// PUT /api/admin/links/order — reassign display_order based on id sequence.
pub async fn handle_admin_reorder(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let bytes = req.bytes().await?;
    let input: ReorderRequest = serde_json::from_slice(&bytes).map_err(AppError::from)?;
    validate_reorder(&input.ids)?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = LinkRepository::new(db);

    let items = repo
        .reorder(&input.ids)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let page = repo
        .get_page()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = LinksResponse {
        page: &page,
        items: &items,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

// --- validation helpers ---

fn validate_url(s: &str) -> Result<(), AppError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Err(AppError::BadRequest("url must not be empty".to_string()));
    }
    if trimmed.chars().count() > 2048 {
        return Err(AppError::BadRequest(
            "url must be <= 2048 chars".to_string(),
        ));
    }
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(AppError::BadRequest(
            "url must use http or https scheme".to_string(),
        ));
    }
    Ok(())
}

fn validate_title(s: &str) -> Result<(), AppError> {
    let len = s.trim().chars().count();
    if !(1..=100).contains(&len) {
        return Err(AppError::BadRequest(
            "title must be 1..=100 chars".to_string(),
        ));
    }
    Ok(())
}

fn validate_bio(opt: &Option<String>) -> Result<(), AppError> {
    if let Some(s) = opt {
        if s.chars().count() > 300 {
            return Err(AppError::BadRequest(
                "bio must be 0..=300 chars".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_label(s: &str) -> Result<(), AppError> {
    let len = s.trim().chars().count();
    if !(1..=80).contains(&len) {
        return Err(AppError::BadRequest(
            "label must be 1..=80 chars".to_string(),
        ));
    }
    Ok(())
}

fn validate_icon(opt: &Option<String>) -> Result<(), AppError> {
    if let Some(s) = opt {
        if !ICON_ALLOWLIST.contains(&s.as_str()) {
            return Err(AppError::BadRequest("icon is not in allowlist".to_string()));
        }
    }
    Ok(())
}

fn validate_page_update(input: &PageUpdate) -> Result<(), AppError> {
    validate_title(&input.title)?;
    validate_bio(&input.bio)?;
    if let Some(url) = &input.avatar_url {
        validate_url(url)?;
    }
    if !BACKGROUND_ALLOWLIST.contains(&input.background.as_str()) {
        return Err(AppError::BadRequest(
            "background must be one of dark|gradient|mesh".to_string(),
        ));
    }
    if !BUTTON_STYLE_ALLOWLIST.contains(&input.button_style.as_str()) {
        return Err(AppError::BadRequest(
            "buttonStyle must be one of solid|outline|soft".to_string(),
        ));
    }
    Ok(())
}

fn validate_link_create(input: &LinkCreate) -> Result<(), AppError> {
    validate_label(&input.label)?;
    validate_url(&input.url)?;
    validate_icon(&input.icon)?;
    Ok(())
}

fn validate_link_update(input: &LinkUpdate) -> Result<(), AppError> {
    validate_label(&input.label)?;
    validate_url(&input.url)?;
    validate_icon(&input.icon)?;
    Ok(())
}

fn validate_reorder(ids: &[String]) -> Result<(), AppError> {
    if ids.is_empty() {
        return Err(AppError::BadRequest("ids must not be empty".to_string()));
    }
    if ids.len() > 200 {
        return Err(AppError::BadRequest("ids count must be <= 200".to_string()));
    }
    let seen: HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    if seen.len() != ids.len() {
        return Err(AppError::BadRequest("ids must be unique".to_string()));
    }
    Ok(())
}
