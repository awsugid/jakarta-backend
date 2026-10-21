use serde::Serialize;
use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::json_success_cors;
use crate::sessionize::client::{SessionizeClient, NO_SNAPSHOT_ERROR};
use crate::sessionize::types::{
    EnrichedSession, EnrichedSpeaker, SessionizeMetadata, SessionizeQueryFilter,
};

fn sessionize_cache_err(e: String) -> AppError {
    if e.starts_with(NO_SNAPSHOT_ERROR) {
        AppError::NotFound(e)
    } else {
        AppError::Internal(e)
    }
}

async fn load_sessionize_data(
    req: &Request,
    ctx: &RouteContext<()>,
    filter: &SessionizeQueryFilter,
) -> Result<crate::sessionize::types::SessionizeDataPayload, AppError> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Upstream fetches are admin-only; public reads are served from the D1 snapshot.
    if filter.fresh {
        require_admin(req, &config, Some(&db)).await?;
    }

    let client = SessionizeClient::default();
    client
        .get_data(&db, filter.fresh)
        .await
        .map_err(sessionize_cache_err)
}

fn parse_query_filter(req: &Request) -> SessionizeQueryFilter {
    let mut filter = SessionizeQueryFilter::default();
    if let Ok(url) = req.url() {
        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "q" => filter.q = Some(v.to_string()),
                "room" => filter.room = Some(v.to_string()),
                "jobTitle" | "job_title" => filter.job_title = Some(v.to_string()),
                "company" => filter.company = Some(v.to_string()),
                "fresh" => filter.fresh = v == "true" || v == "1",
                _ => {}
            }
        }
    }
    filter
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeSpeakersResponse {
    pub speakers: Vec<EnrichedSpeaker>,
    pub metadata: SessionizeMetadata,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeSessionsResponse {
    pub sessions: Vec<EnrichedSession>,
    pub metadata: SessionizeMetadata,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionizeDataResponse {
    pub speakers: Vec<EnrichedSpeaker>,
    pub sessions: Vec<EnrichedSession>,
    pub metadata: SessionizeMetadata,
}

/// GET /api/sessionize/speakers
pub async fn handle_get_sessionize_speakers(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let origin = req.headers().get("Origin").ok().flatten();

    let filter = parse_query_filter(&req);
    let payload = load_sessionize_data(&req, &ctx, &filter).await?;

    let filtered = SessionizeClient::apply_filters(payload, &filter);

    let body = SessionizeSpeakersResponse {
        speakers: filtered.speakers,
        metadata: filtered.metadata,
    };

    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/sessionize/sessions
pub async fn handle_get_sessionize_sessions(
    req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let origin = req.headers().get("Origin").ok().flatten();

    let filter = parse_query_filter(&req);
    let payload = load_sessionize_data(&req, &ctx, &filter).await?;

    let filtered = SessionizeClient::apply_filters(payload, &filter);

    let body = SessionizeSessionsResponse {
        sessions: filtered.sessions,
        metadata: filtered.metadata,
    };

    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}

/// GET /api/sessionize/data — full enriched data (speakers + sessions + metadata)
pub async fn handle_get_sessionize_data(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let origin = req.headers().get("Origin").ok().flatten();

    let filter = parse_query_filter(&req);
    let payload = load_sessionize_data(&req, &ctx, &filter).await?;

    let filtered = SessionizeClient::apply_filters(payload, &filter);

    let body = SessionizeDataResponse {
        speakers: filtered.speakers,
        sessions: filtered.sessions,
        metadata: filtered.metadata,
    };

    json_success_cors(&body, &config.allowed_origins, origin.as_deref())
}
