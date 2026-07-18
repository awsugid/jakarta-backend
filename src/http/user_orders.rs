use worker::*;

use crate::auth::admin::require_user;
use crate::config::AppConfig;
use crate::http::errors::AppError;
use crate::http::response::json_success_cors;
use crate::pretix::client::PretixClient;
use crate::pretix::orders::UserPretixOrdersResponse;

/// GET /api/pretix/me/orders — sanitized order history for the signed-in user.
pub async fn handle_my_pretix_orders(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let user = require_user(&req, &config).await?;

    if config.pretix_default_organizer.trim().is_empty() {
        return Err(
            AppError::Internal("PRETIX_DEFAULT_ORGANIZER not configured".to_string()).into(),
        );
    }

    let origin = req.headers().get("Origin").ok().flatten();

    // Parse optional query params: limit (default 20, clamp 1..100), offset (default 0), status.
    let (limit, offset, status) = parse_query(&req)?;

    let client = PretixClient::new(&config);
    let page = client
        .list_orders_for_email(
            &config.pretix_default_organizer,
            None,
            &user.normalized_email(),
            limit,
            offset,
            status.as_deref(),
        )
        .await
        .map_err(|e| AppError::FormBricksError(format!("Pretix: {e}")))?;

    let body = UserPretixOrdersResponse {
        orders: page.orders,
        total: Some(page.count),
        limit,
        offset,
    };

    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

fn parse_query(req: &Request) -> Result<(u32, u32, Option<String>), AppError> {
    let url = req.url().map_err(|e| AppError::Internal(e.to_string()))?;
    let qs: std::collections::HashMap<String, String> = url.query_pairs().into_owned().collect();

    let limit = qs
        .get("limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    let offset = qs
        .get("offset")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let status = qs.get("status").map(|s| s.to_string());
    Ok((limit, offset, status))
}
