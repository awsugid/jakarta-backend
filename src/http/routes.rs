use worker::*;

use crate::application::discovery::{discover_user_application, validate_application};
use crate::application::policy;
use crate::application::service;
use crate::auth::google::extract_user;
use crate::config::AppConfig;
use crate::formbricks::client::FormbricksClient;
use crate::http::errors::AppError;
use crate::http::response::{cors_preflight, json_success, with_cors};
use crate::storage::d1::FormRepository;

/// Wire all HTTP routes onto the worker Router.
pub fn register_routes(router: Router<'_, ()>) -> Router<'_, ()> {
    router
        // ---------------------------------------------------------------
        // Health
        // ---------------------------------------------------------------
        .get("/health", |_req, _ctx| Response::ok("OK"))
        // ---------------------------------------------------------------
        // Webhook — FormBricks responseFinished webhook
        // ---------------------------------------------------------------
        .post_async("/api/webhook/formbricks", |req, ctx| async move {
            crate::http::webhook::handle_webhook(req, ctx).await
        })
        // ---------------------------------------------------------------
        // CORS preflight for all /api/* routes
        // ---------------------------------------------------------------
        .options("/api/*rest", |_req, ctx| {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            cors_preflight(&config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // GET /api/forms — list all active forms (optionally filter by ?kind=)
        // ---------------------------------------------------------------
        .get_async("/api/forms", |req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let repo = FormRepository::new(db);

            let kind = req.url().ok().and_then(|url| {
                url.query_pairs()
                    .find(|(k, _)| k == "kind")
                    .map(|(_, v)| v.to_string())
            });

            let authed = extract_user(&req, &config).await.is_ok();
            let resp = if authed {
                let forms = service::list_forms_authed(&repo, kind.as_deref()).await?;
                json_success(&forms)?
            } else {
                let forms = service::list_forms(&repo, kind.as_deref()).await?;
                json_success(&forms)?
            };
            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // GET /api/forms/:kind — list forms filtered by kind
        // ---------------------------------------------------------------
        .get_async("/api/forms/:kind", |req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let repo = FormRepository::new(db);

            let kind = ctx
                .param("kind")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: kind".to_string()))?;

            let authed = extract_user(&req, &config).await.is_ok();
            let resp = if authed {
                let forms = service::list_forms_authed(&repo, Some(kind.as_str())).await?;
                json_success(&forms)?
            } else {
                let forms = service::list_forms(&repo, Some(kind.as_str())).await?;
                json_success(&forms)?
            };
            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // GET /api/forms/:kind/:slug — get a single form with status
        // ---------------------------------------------------------------
        .get_async("/api/forms/:kind/:slug", |req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let repo = FormRepository::new(db);

            let kind = ctx
                .param("kind")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: kind".to_string()))?;
            let slug = ctx
                .param("slug")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: slug".to_string()))?;

            let authed = extract_user(&req, &config).await.is_ok();
            let resp = if authed {
                let form_status = service::get_form_status_authed(&repo, kind, slug).await?;
                json_success(&form_status)?
            } else {
                let form_status = service::get_form_status(&repo, kind, slug).await?;
                json_success(&form_status)?
            };
            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // GET /api/applications/summary — list all user's existing applications
        // Registered BEFORE /:kind/:slug to avoid "summary" matching as :kind.
        // ---------------------------------------------------------------
        .get_async("/api/applications/summary", |req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let user = extract_user(&req, &config).await?;

            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let repo = FormRepository::new(db);
            let client = FormbricksClient::new(&config);

            let summaries = service::list_user_applications(&repo, &client, &user).await?;
            let resp = json_success(&summaries)?;
            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // GET /api/applications/:kind/:slug/response — get user's full response
        // ---------------------------------------------------------------
        .get_async(
            "/api/applications/:kind/:slug/response",
            |req, ctx| async move {
                let config =
                    AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
                let user = extract_user(&req, &config).await?;

                let db = ctx
                    .d1("DB")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let repo = FormRepository::new(db);
                let client = FormbricksClient::new(&config);

                let kind = ctx.param("kind").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: kind".to_string())
                })?;
                let slug = ctx.param("slug").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: slug".to_string())
                })?;

                let result =
                    service::get_user_response(&repo, &client, &user, &kind, &slug).await?;
                let resp = json_success(&result)?;
                with_cors(resp, &config.allowed_origins)
            },
        )
        // ---------------------------------------------------------------
        // GET /api/applications/:kind/:slug — discover user's existing application
        // ---------------------------------------------------------------
        .get_async("/api/applications/:kind/:slug", |req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let user = extract_user(&req, &config).await?;

            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let repo = FormRepository::new(db);
            let client = FormbricksClient::new(&config);

            let kind = ctx
                .param("kind")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: kind".to_string()))?;
            let slug = ctx
                .param("slug")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: slug".to_string()))?;

            let form = repo
                .get_form(kind, slug)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .ok_or_else(|| AppError::NotFound(format!("Form {}/{} not found", kind, slug)))?;

            let result =
                discover_user_application(&client, &form, &user, form.editable_until.as_deref())
                    .await?;

            let resp = json_success(&result)?;
            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // POST /api/applications/:kind/:slug/validate — validate a proposed submission
        // ---------------------------------------------------------------
        .post_async(
            "/api/applications/:kind/:slug/validate",
            |mut req, ctx| async move {
                let config =
                    AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
                let user = extract_user(&req, &config).await?;

                let db = ctx
                    .d1("DB")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let repo = FormRepository::new(db);
                let client = FormbricksClient::new(&config);

                let kind = ctx.param("kind").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: kind".to_string())
                })?;
                let slug = ctx.param("slug").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: slug".to_string())
                })?;

                // Parse JSON body: { "linkedin_url": "..." }
                let body: serde_json::Value = req
                    .json()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {}", e)))?;
                let linkedin_url = body["linkedin_url"].as_str().ok_or_else(|| {
                    AppError::BadRequest("Missing or invalid 'linkedin_url' field".to_string())
                })?;

                let form = repo
                    .get_form(kind, slug)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?
                    .ok_or_else(|| {
                        AppError::NotFound(format!("Form {}/{} not found", kind, slug))
                    })?;

                let result = validate_application(&client, &form, &user, linkedin_url).await?;
                let resp = json_success(&result)?;
                with_cors(resp, &config.allowed_origins)
            },
        )
        // ---------------------------------------------------------------
        // POST /api/applications/:kind/:slug/link — get the FormBricks form link
        // ---------------------------------------------------------------
        .post_async(
            "/api/applications/:kind/:slug/link",
            |req, ctx| async move {
                let config =
                    AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
                let user = extract_user(&req, &config).await?;

                let db = ctx
                    .d1("DB")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let repo = FormRepository::new(db);

                let kind = ctx.param("kind").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: kind".to_string())
                })?;
                let slug = ctx.param("slug").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: slug".to_string())
                })?;

                let form = repo
                    .get_form(&kind, &slug)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?
                    .ok_or_else(|| {
                        AppError::NotFound(format!("Form {}/{} not found", kind, slug))
                    })?;

                let public_url = form.formbricks_public_url.clone().ok_or_else(|| {
                    AppError::Internal("Form has no public URL configured.".to_string())
                })?;

                // Check mode query parameter
                let mode = req.url().ok().and_then(|url| {
                    url.query_pairs()
                        .find(|(k, _)| k == "mode")
                        .map(|(_, v)| v.to_string())
                });

                let result = if mode.as_deref() == Some("edit") {
                    if !policy::is_form_editable(&form) {
                        return Err(AppError::Forbidden(
                            "This form is currently closed and no longer editable.".to_string(),
                        )
                        .into());
                    }

                    let client = FormbricksClient::new(&config);
                    let discovery = crate::application::discovery::discover_user_application(
                        &client,
                        &form,
                        &user,
                        form.editable_until.as_deref(),
                    )
                    .await?;

                    let response_id = discovery.response_id.ok_or_else(|| {
                        AppError::NotFound("No existing response found for this user".to_string())
                    })?;

                    let fb_response = client.get_response(&response_id).await.map_err(|e| {
                        AppError::FormBricksError(format!("Failed to fetch response: {}", e))
                    })?;

                    let prefilled_url =
                        service::build_prefilled_url(&public_url, &fb_response.data);

                    serde_json::json!({
                        "url": prefilled_url,
                        "editable": true,
                        "response_id": response_id,
                    })
                } else {
                    // Check that the form is open
                    let is_open = policy::is_form_open(&form);
                    if !is_open {
                        return Err(AppError::Forbidden(
                            "This form is currently closed and no longer editable.".to_string(),
                        )
                        .into());
                    }

                    serde_json::json!({
                        "url": public_url,
                        "editable": policy::is_form_editable(&form),
                    })
                };

                let resp = json_success(&result)?;
                with_cors(resp, &config.allowed_origins)
            },
        )
}
