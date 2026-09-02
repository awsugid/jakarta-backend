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
///
/// Note: worker 0.8 has no middleware API. Admin auth is enforced per-handler
/// via `require_admin`. Keep every `/api/admin/*` handler calling it.
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
        // GET /api/events/:siteSlug/pretix-stats — Pretix aggregate stats (public)
        // ---------------------------------------------------------------
        .get_async(
            "/api/events/:siteSlug/pretix-stats",
            |req, ctx| async move {
                let config =
                    AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;

                let site_slug = ctx
                    .param("siteSlug")
                    .ok_or_else(|| AppError::BadRequest("Missing siteSlug".to_string()))?;

                let url = req.url().map_err(|e| AppError::Internal(e.to_string()))?;
                let qs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();

                let org = qs
                    .get("organizer_slug")
                    .ok_or_else(|| AppError::BadRequest("Missing organizer_slug".to_string()))?;
                let evt = qs
                    .get("event_slug")
                    .ok_or_else(|| AppError::BadRequest("Missing event_slug".to_string()))?;
                let sub = qs.get("subevent_id");
                let items: Option<Vec<u64>> = qs.get("item_ids").and_then(|s| {
                    let v: Vec<u64> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
                    if v.is_empty() {
                        None
                    } else {
                        Some(v)
                    }
                });

                let client = crate::pretix::client::PretixClient::new(&config);

                // Auto-discover first check-in list when not provided.
                let list = match qs.get("checkin_list_id") {
                    Some(l) if !l.trim().is_empty() => l.clone(),
                    _ => client.get_first_checkin_list_id(org, evt).await.map_err(|e| {
                        AppError::Internal(format!("Pretix checkinlist lookup: {e}"))
                    })?,
                };

                let registered = client
                    .get_position_count(
                        org,
                        evt,
                        &list,
                        false,
                        sub.map(|s| s.as_str()),
                        items.as_deref(),
                    )
                    .await
                    .map_err(|e| AppError::Internal(format!("Pretix: {}", e)))?;

                let checked_in = client
                    .get_position_count(
                        org,
                        evt,
                        &list,
                        true,
                        sub.map(|s| s.as_str()),
                        items.as_deref(),
                    )
                    .await
                    .map_err(|e| AppError::Internal(format!("Pretix: {}", e)))?;

                let rate = if registered > 0 {
                    Some(checked_in as f64 / registered as f64)
                } else {
                    None
                };

                let now = (js_sys::Date::now() / 1000.0) as u64;

                let result = serde_json::json!({
                    "site_slug": site_slug,
                    "pretix": {
                        "organizer_slug": org,
                        "event_slug": evt,
                        "checkin_list_id": list,
                        "subevent_id": sub,
                    },
                    "registered_count": registered,
                    "checked_in_count": checked_in,
                    "attendance_rate": rate,
                    "last_refreshed_at": now.to_string(),
                    "stale": false,
                });

                let resp = json_success(&result)?;
                with_cors(resp, &config.allowed_origins)
            },
        )
        // ---------------------------------------------------------------
        // Admin — Re-ingest application_response_index from Formbricks
        // ---------------------------------------------------------------
        .post_async("/api/admin/reingest", |req, ctx| async move {
            crate::http::reingest::handle_reingest(req, ctx).await
        })
        // ---------------------------------------------------------------
        // Admin — identity & dashboard (admin-only, origin-reflected CORS)
        // ---------------------------------------------------------------
        .get_async("/api/admin/me", |req, ctx| async move {
            crate::http::admin::handle_admin_me(req, ctx).await
        })
        .get_async("/api/admin/forms", |req, ctx| async move {
            crate::http::admin::handle_admin_forms(req, ctx).await
        })
        .get_async("/api/admin/formbricks/responses", |req, ctx| async move {
            crate::http::admin::handle_admin_responses(req, ctx).await
        })
        .get_async(
            "/api/admin/formbricks/responses/:responseId",
            |req, ctx| async move {
                crate::http::admin::handle_admin_response_detail(req, ctx).await
            },
        )
        // ---------------------------------------------------------------
        // GET /api/pretix/me/orders — authenticated user order history
        // ---------------------------------------------------------------
        .get_async("/api/pretix/me/orders", |req, ctx| async move {
            crate::http::user_orders::handle_my_pretix_orders(req, ctx).await
        })
        // ---------------------------------------------------------------
        // GET /api/community/statistics — merged community stats (public, cached 1h)
        // ---------------------------------------------------------------
        .get_async("/api/community/statistics", |_req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;

            // Cloudflare Cache API — 1h TTL, cache-first.
            // Versioned key so code/data changes invalidate.
            let cache_key = "https://jakarta-backend.local/api/community/statistics?v=8";
            let cache = worker::Cache::default();
            if let Some(cached) = cache.get(cache_key, true).await.ok().flatten() {
                return with_cors(cached, &config.allowed_origins);
            }

            let stats =
                crate::statistics::service::get_community_statistics(&config, &db).await?;
            let mut resp = json_success(&stats)?;

            // Cache API requires a Response with Cache-Control header.
            let mut cached_resp = resp.cloned()?;
            let cache_headers = cached_resp.headers_mut();
            cache_headers.set("Cache-Control", "public, max-age=3600")?;
            cache.put(cache_key, cached_resp).await.ok();

            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // Public — Linktree community page
        // ---------------------------------------------------------------
        .get_async("/api/links", |req, ctx| async move {
            crate::http::links::handle_public_links(req, ctx).await
        })
        // ---------------------------------------------------------------
        // Admin — Linktree management
        // ---------------------------------------------------------------
        .get_async("/api/admin/links", |req, ctx| async move {
            crate::http::links::handle_admin_links(req, ctx).await
        })
        .put_async("/api/admin/links/page", |req, ctx| async move {
            crate::http::links::handle_admin_update_page(req, ctx).await
        })
        .post_async("/api/admin/links/items", |req, ctx| async move {
            crate::http::links::handle_admin_create_link(req, ctx).await
        })
        .put_async("/api/admin/links/items/:linkId", |req, ctx| async move {
            crate::http::links::handle_admin_update_link(req, ctx).await
        })
        .delete_async("/api/admin/links/items/:linkId", |req, ctx| async move {
            crate::http::links::handle_admin_delete_link(req, ctx).await
        })
        .put_async("/api/admin/links/order", |req, ctx| async move {
            crate::http::links::handle_admin_reorder(req, ctx).await
        })
        // ---------------------------------------------------------------
        // GET /api/events/:eventSlug/sponsor-packages — sponsor packages (public)
        // ---------------------------------------------------------------
        .get_async(
            "/api/events/:eventSlug/sponsor-packages",
            |req, ctx| async move {
                crate::http::sponsors::handle_public_sponsor_packages(req, ctx).await
            },
        )
        // ---------------------------------------------------------------
        // PUT /api/admin/events/:eventSlug/sponsor-packages — batch price/unlock
        // update (admin-only, origin-reflected CORS)
        // ---------------------------------------------------------------
        .put_async(
            "/api/admin/events/:eventSlug/sponsor-packages",
            |req, ctx| async move {
                crate::http::sponsors::handle_admin_update_sponsor_packages(req, ctx).await
            },
        )
        // ---------------------------------------------------------------
        // POST /api/admin/events/:eventSlug/sponsor-packages — create a
        // sponsor package (admin-only, origin-reflected CORS)
        // ---------------------------------------------------------------
        .post_async(
            "/api/admin/events/:eventSlug/sponsor-packages",
            |req, ctx| async move {
                crate::http::sponsors::handle_admin_create_sponsor_package(req, ctx).await
            },
        )
        // ---------------------------------------------------------------
        // POST /api/admin/events/:eventSlug/sponsor-groups — create sponsor
        // package group (admin-only, origin-reflected CORS)
        // ---------------------------------------------------------------
        .post_async(
            "/api/admin/events/:eventSlug/sponsor-groups",
            |req, ctx| async move {
                crate::http::sponsors::handle_admin_create_sponsor_group(req, ctx).await
            },
        )
        // ---------------------------------------------------------------
        // CORS preflight for all /api/* routes
        // ---------------------------------------------------------------
        .options("/api/*rest", |_req, ctx| {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            cors_preflight(&config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // POST /api/profiles/lookup — batch profile lookup by email (public)
        // ---------------------------------------------------------------
        .post_async("/api/profiles/lookup", |req, ctx| async move {
            crate::http::profiles::handle_profiles_lookup(req, ctx).await
        })
        // ---------------------------------------------------------------
        // GET /api/profiles/me — the caller's own profile (authenticated)
        // ---------------------------------------------------------------
        .get_async("/api/profiles/me", |req, ctx| async move {
            crate::http::profiles::handle_profiles_me_get(req, ctx).await
        })
        // ---------------------------------------------------------------
        // PUT /api/profiles/me — replace the caller's editable profile (authenticated)
        // ---------------------------------------------------------------
        .put_async("/api/profiles/me", |req, ctx| async move {
            crate::http::profiles::handle_profiles_me_put(req, ctx).await
        })
        // ---------------------------------------------------------------
        // POST /api/profiles/me/avatar — upload avatar image to Cloudflare R2 (authenticated)
        // ---------------------------------------------------------------
        .post_async("/api/profiles/me/avatar", |req, ctx| async move {
            crate::http::avatars::handle_profiles_me_avatar_post(req, ctx).await
        })
        // ---------------------------------------------------------------
        // DELETE /api/profiles/me/avatar — reset avatar image (authenticated)
        // ---------------------------------------------------------------
        .delete_async("/api/profiles/me/avatar", |req, ctx| async move {
            crate::http::avatars::handle_profiles_me_avatar_delete(req, ctx).await
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
            let authed = extract_user(&req, &config, Some(&db)).await.is_ok();
            let repo = FormRepository::new(db);

            let kind = req.url().ok().and_then(|url| {
                url.query_pairs()
                    .find(|(k, _)| k == "kind")
                    .map(|(_, v)| v.to_string())
            });

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
            let authed = extract_user(&req, &config, Some(&db)).await.is_ok();
            let repo = FormRepository::new(db);

            let kind = ctx
                .param("kind")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: kind".to_string()))?;

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
            let authed = extract_user(&req, &config, Some(&db)).await.is_ok();
            let repo = FormRepository::new(db);

            let kind = ctx
                .param("kind")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: kind".to_string()))?;
            let slug = ctx
                .param("slug")
                .ok_or_else(|| AppError::BadRequest("Missing path parameter: slug".to_string()))?;

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
        // GET /api/forms/:kind/:slug/schema — get question labels for a form
        // ---------------------------------------------------------------
        .get_async("/api/forms/:kind/:slug/schema", |_req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
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

            // Get the form to retrieve survey_id
            let form = repo
                .get_form(kind, slug)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?
                .ok_or_else(|| AppError::NotFound(format!("Form {}/{} not found", kind, slug)))?;

            // Fetch survey definition from FormBricks
            let survey = client
                .get_survey(&form.formbricks_survey_id)
                .await
                .map_err(|e| AppError::FormBricksError(format!("Failed to fetch survey: {}", e)))?;

            // Build a map of question ID -> { label, type }.
            // Support both legacy `questions[]` and newer `blocks[].elements[]`.
            let mut question_labels = std::collections::HashMap::new();

            for question in &survey.questions {
                let label = question.headline_text();
                if !label.is_empty() {
                    question_labels.insert(
                        question.id.clone(),
                        serde_json::json!({ "label": label, "type": question.question_type }),
                    );
                }
            }

            for block in &survey.blocks {
                for element in &block.elements {
                    let label = element.headline_text();
                    if !label.is_empty() {
                        question_labels.insert(
                            element.id.clone(),
                            serde_json::json!({ "label": label, "type": element.question_type }),
                        );
                    }
                }
            }

            let result = serde_json::json!({
                "questions": question_labels,
            });

            let resp = json_success(&result)?;
            with_cors(resp, &config.allowed_origins)
        })
        // ---------------------------------------------------------------
        // GET /api/applications/summary — list all user's existing applications
        // Registered BEFORE /:kind/:slug to avoid "summary" matching as :kind.
        // ---------------------------------------------------------------
        .get_async("/api/applications/summary", |req, ctx| async move {
            let config =
                AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let user = extract_user(&req, &config, Some(&db)).await?;
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
                let db = ctx
                    .d1("DB")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let user = extract_user(&req, &config, Some(&db)).await?;
                let repo = FormRepository::new(db);
                let client = FormbricksClient::new(&config);

                let kind = ctx.param("kind").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: kind".to_string())
                })?;
                let slug = ctx.param("slug").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: slug".to_string())
                })?;

                let result = service::get_user_response(&repo, &client, &user, kind, slug).await?;
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
            let db = ctx
                .d1("DB")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let user = extract_user(&req, &config, Some(&db)).await?;
            let repo = FormRepository::new(db);
            let _client = FormbricksClient::new(&config);

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
                discover_user_application(&repo, &form, &user, form.editable_until.as_deref())
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
                let db = ctx
                    .d1("DB")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let user = extract_user(&req, &config, Some(&db)).await?;
                let repo = FormRepository::new(db);
                let _client = FormbricksClient::new(&config);

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

                let result = validate_application(&repo, &form, &user, linkedin_url).await?;
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
                let db = ctx
                    .d1("DB")
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                let user = extract_user(&req, &config, Some(&db)).await?;
                let repo = FormRepository::new(db);

                let kind = ctx.param("kind").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: kind".to_string())
                })?;
                let slug = ctx.param("slug").ok_or_else(|| {
                    AppError::BadRequest("Missing path parameter: slug".to_string())
                })?;

                let form = repo
                    .get_form(kind, slug)
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
                        &repo,
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

                    // Pre-fill the email with the user's Google email
                    let mut prefill_data = std::collections::HashMap::new();
                    prefill_data.insert(
                        form.email_question_id.clone(),
                        serde_json::Value::String(user.email.clone()),
                    );
                    let prefilled_url = service::build_prefilled_url(&public_url, &prefill_data);

                    serde_json::json!({
                        "url": prefilled_url,
                        "editable": policy::is_form_editable(&form),
                    })
                };

                let resp = json_success(&result)?;
                with_cors(resp, &config.allowed_origins)
            },
        )
}
