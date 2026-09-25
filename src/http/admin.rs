use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::formbricks::client::FormbricksClient;
use crate::formbricks::responses::extract_answers_list;
use crate::formbricks::types::{FormbricksResponse, FormbricksSurvey};
use crate::http::errors::AppError;
use crate::http::response::json_success_cors;
use crate::storage::d1::{FormRepository, ResponseTagRepository};
use crate::validation::tags::normalize_tags;

/// Safety cap for the live Formbricks walks backing admin response counts
/// (100 pages × limit 100 = 10k responses per survey).
const FORMS_COUNT_MAX_WALK_PAGES: u32 = 100;

#[derive(Serialize)]
struct AdminMe {
    email: String,
    name: Option<String>,
    picture: Option<String>,
    is_admin: bool,
}

/// GET /api/admin/me — identity of the authenticated admin.
pub async fn handle_admin_me(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    let user = require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();
    let body = AdminMe {
        email: user.email,
        name: user.name,
        picture: user.picture,
        is_admin: true,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

#[derive(Serialize)]
struct AdminFormSummary {
    kind: String,
    slug: String,
    title: String,
    description: Option<String>,
    survey_id: String,
    is_active: bool,
    response_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUpdateFormStatusInput {
    #[serde(alias = "is_active")]
    pub is_active: bool,
}

/// GET /api/admin/forms — list all application_forms (all kinds, active and inactive).
///
/// `response_count` reflects ALL live Formbricks responses for the form's
/// survey (finished + in-progress), NOT the partial D1 index. A per-form
/// upstream failure yields `null` — never a misleading zero. Shared survey IDs
/// are counted once.
pub async fn handle_admin_forms(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = FormRepository::new(db);

    let forms = repo
        .list_all_forms(None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // ponytail: sequential per-distinct-survey walks; N forms × pages of
    // upstream fetches per request. Fine at ~13 forms with 1–2 pages each;
    // if surveys grow past the Workers subrequest budget, cache counts in D1
    // with a short TTL instead.
    let client = FormbricksClient::new(&config);
    let mut count_cache: HashMap<String, Option<u64>> = HashMap::new();
    for f in &forms {
        if !count_cache.contains_key(&f.formbricks_survey_id) {
            let count = match client
                .get_all_responses(&f.formbricks_survey_id, FORMS_COUNT_MAX_WALK_PAGES)
                .await
            {
                Ok(all) => Some(all.len() as u64),
                // Log identifiers only — no PII in logs.
                Err(e) => {
                    console_log!(
                        "response count fetch failed: kind={} slug={} survey={}: {}",
                        f.kind,
                        f.slug,
                        f.formbricks_survey_id,
                        e
                    );
                    None
                }
            };
            count_cache.insert(f.formbricks_survey_id.clone(), count);
        }
    }

    let items: Vec<AdminFormSummary> = forms
        .into_iter()
        .map(|f| {
            let response_count = count_cache.get(&f.formbricks_survey_id).copied().flatten();
            AdminFormSummary {
                kind: f.kind,
                slug: f.slug,
                title: f.title,
                description: f.description,
                survey_id: f.formbricks_survey_id,
                is_active: f.is_active,
                response_count,
            }
        })
        .collect();

    let resp = json_success_cors(&items, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// PUT /api/admin/forms/:kind/:slug — update form active status (open/closed) (admin-only).
pub async fn handle_admin_update_form_status(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let kind = ctx
        .param("kind")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: kind".to_string()))?;
    let slug = ctx
        .param("slug")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: slug".to_string()))?;

    let input: AdminUpdateFormStatusInput = req
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {e}")))?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = FormRepository::new(db);

    let updated = repo
        .update_form_active(kind, slug, input.is_active)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Form {}/{} not found", kind, slug)))?;

    // The toggle already mutated state: a count failure must NOT fail the
    // request. Fall back to null and log identifiers only (no PII).
    let client = FormbricksClient::new(&config);
    let response_count = match client
        .get_all_responses(&updated.formbricks_survey_id, FORMS_COUNT_MAX_WALK_PAGES)
        .await
    {
        Ok(all) => Some(all.len() as u64),
        Err(e) => {
            console_log!(
                "response count fetch failed after toggle: kind={} slug={} survey={}: {}",
                updated.kind,
                updated.slug,
                updated.formbricks_survey_id,
                e
            );
            None
        }
    };

    let summary = AdminFormSummary {
        kind: updated.kind,
        slug: updated.slug,
        title: updated.title,
        description: updated.description,
        survey_id: updated.formbricks_survey_id,
        is_active: updated.is_active,
        response_count,
    };

    let resp = json_success_cors(&summary, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

#[derive(Serialize)]
struct AdminFormbricksResponseSummary {
    id: String,
    survey_id: String,
    submitted_at: Option<String>,
    updated_at: Option<String>,
    finished: bool,
    respondent_email: Option<String>,
    respondent_name: Option<String>,
    preview_answers: serde_json::Value,
    tags: Vec<String>,
}

#[derive(Serialize)]
struct AdminFormbricksResponseList {
    items: Vec<AdminFormbricksResponseSummary>,
    total: Option<u64>,
    limit: u32,
    offset: u32,
    stats: AdminFormbricksStats,
}

/// Stats over the FULL filtered set (before windowing), matching the active
/// survey + finished + tag filters.
#[derive(Serialize, Debug, PartialEq)]
struct AdminFormbricksStats {
    total: u64,
    finished: u64,
    in_progress: u64,
    latest_submission: Option<String>,
}

/// GET /api/admin/formbricks/responses?surveyId=...&limit=50&offset=0&finished=all|true|false&tag=<label>&tag=<label>&untagged=true
///
/// Tag filtering: repeated `tag` params OR together (any selected label
/// matches); `untagged=true` selects responses with no tags (mutually
/// exclusive with `tag`); neither present disables tag filtering (backward
/// compatible).
///
/// Walks every upstream page once, applies the finished/tag filter, computes
/// stats over the full filtered set, then windows offset/limit locally — so
/// `total` and `stats` are exact across ALL pages while `items` is just the
/// requested window.
pub async fn handle_admin_responses(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let ListQuery {
        survey_id,
        limit,
        offset,
        finished_filter,
        tag_filter,
    } = parse_list_query(&req)?;

    let client = FormbricksClient::new(&config);

    // Fetch schema once for label mapping.
    let survey = client
        .get_survey(&survey_id)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch survey: {e}")))?;
    let qmap = build_question_map(&survey);

    // Tags live in D1, keyed per survey (independent of the response index:
    // unfinished/unindexed responses can carry tags too).
    let tag_repo =
        ResponseTagRepository::new(db_opt.ok_or_else(|| {
            AppError::Internal("D1 database binding DB not available".to_string())
        })?);
    let tag_rows = tag_repo
        .list_survey_tag_map(&survey_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let mut tag_map: HashMap<String, Vec<String>> = HashMap::new();
    for (rid, tag) in &tag_rows {
        tag_map.entry(rid.clone()).or_default().push(tag.clone());
    }

    // Exact-label tag filtering (OR over repeated tag params, or the untagged
    // complement) needs the fetched set, so walk upstream first.
    // ponytail: every view walks every upstream page (100 pages = 10k
    // responses at limit 100) because stats must cover the full filtered set
    // and upstream cannot filter by `finished` or tag. Replace with upstream
    // filters if they are ever added to the v2 API.
    const LIST_MAX_WALK_PAGES: u32 = 100;

    let all = client
        .get_all_responses(&survey_id, LIST_MAX_WALK_PAGES)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch responses: {e}")))?;
    let tag_members = allowed_tag_members(&tag_filter, &tag_rows, &all);
    let (data, stats) =
        filter_and_window(all, finished_filter, tag_members.as_ref(), offset, limit);
    let total = Some(stats.total);

    let items: Vec<AdminFormbricksResponseSummary> = data
        .iter()
        .map(|resp| {
            let mut item = summarize_response(resp, &survey, &qmap);
            item.tags = tag_map.get(&resp.id).cloned().unwrap_or_default();
            item
        })
        .collect();

    let body = AdminFormbricksResponseList {
        items,
        total,
        limit,
        offset,
        stats,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

#[derive(Serialize)]
struct AdminAnswerItem {
    question_id: String,
    label: String,
    #[serde(rename = "type")]
    type_field: String,
    value: serde_json::Value,
}

#[derive(Serialize)]
struct AdminResponseMetadata {
    contact_id: Option<String>,
}

#[derive(Serialize)]
struct AdminFormbricksResponseDetail {
    id: String,
    survey_id: String,
    submitted_at: Option<String>,
    updated_at: Option<String>,
    finished: bool,
    answers: Vec<AdminAnswerItem>,
    metadata: AdminResponseMetadata,
    tags: Vec<String>,
}

/// GET /api/admin/formbricks/responses/:responseId?surveyId=...
pub async fn handle_admin_response_detail(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let response_id = ctx
        .param("responseId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: responseId".to_string()))?;

    let survey_id = req
        .url()
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "surveyId")
                .map(|(_, v)| v.to_string())
        })
        .ok_or_else(|| AppError::BadRequest("Missing query parameter: surveyId".to_string()))?;

    let client = FormbricksClient::new(&config);

    let survey = client
        .get_survey(&survey_id)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch survey: {e}")))?;
    let qmap = build_question_map(&survey);

    let resp = client
        .get_response(response_id)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch response: {e}")))?;

    let submitted_at = if resp.finished {
        Some(resp.created_at.clone())
    } else {
        None
    };

    let answers = build_answer_items(&resp, &qmap);
    let contact_id = resp.contact.as_ref().map(|c| c.id.clone());

    // Tags live in D1, independent of the response index.
    let tag_repo =
        ResponseTagRepository::new(db_opt.ok_or_else(|| {
            AppError::Internal("D1 database binding DB not available".to_string())
        })?);
    let tags = tag_repo
        .get_response_tags(&survey_id, response_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = AdminFormbricksResponseDetail {
        id: resp.id.clone(),
        survey_id: resp.survey_id.clone(),
        submitted_at,
        updated_at: Some(resp.updated_at.clone()),
        finished: resp.finished,
        answers,
        metadata: AdminResponseMetadata { contact_id },
        tags,
    };

    let response = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(response)
}

#[derive(Deserialize)]
pub struct ResponseTagsInput {
    tags: Vec<String>,
}

#[derive(Serialize)]
struct ResponseTagsBody {
    tags: Vec<String>,
}

/// GET /api/admin/formbricks/tags?surveyId=...
/// Distinct assigned tag labels for a survey (derived catalog: labels vanish
/// when no assignment references them).
pub async fn handle_admin_survey_tags(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let survey_id = required_query(&req, "surveyId")?;

    let repo =
        ResponseTagRepository::new(db_opt.ok_or_else(|| {
            AppError::Internal("D1 database binding DB not available".to_string())
        })?);
    let tags = repo
        .list_survey_tags(&survey_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let resp = json_success_cors(&tags, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

/// PUT /api/admin/formbricks/responses/:responseId/tags?surveyId=...
/// Body {"tags":["..."]} → replaces the response's tag set atomically.
/// The response is verified to exist and belong to the survey via Formbricks
/// itself (NOT the D1 response index — unfinished/unindexed responses are
/// taggable too).
pub async fn handle_admin_replace_response_tags(
    mut req: Request,
    ctx: RouteContext<()>,
) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let response_id = ctx
        .param("responseId")
        .ok_or_else(|| AppError::BadRequest("Missing path parameter: responseId".to_string()))?
        .clone();
    let survey_id = required_query(&req, "surveyId")?;

    let input: ResponseTagsInput = req
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid JSON body: {e}")))?;

    // Verify via Formbricks that the response exists and belongs to the survey.
    let client = FormbricksClient::new(&config);
    match client.get_response(&response_id).await {
        Ok(resp) => {
            if resp.survey_id != survey_id {
                return Err(AppError::NotFound(format!(
                    "Response {response_id} does not belong to survey {survey_id}"
                ))
                .into());
            }
        }
        // ponytail: 404 sniffed from the upstream error string; a typed status
        // from the client would remove this. Add if the client ever grows one.
        Err(e) if e.contains("status 404") => {
            return Err(AppError::NotFound(format!("Response {response_id} not found")).into());
        }
        Err(e) => {
            return Err(AppError::FormBricksError(format!("Failed to fetch response: {e}")).into());
        }
    }

    let repo =
        ResponseTagRepository::new(db_opt.ok_or_else(|| {
            AppError::Internal("D1 database binding DB not available".to_string())
        })?);

    // Canonicalize against labels already assigned in this survey so case
    // variants of existing labels reuse the established spelling.
    let existing = repo
        .list_survey_tags(&survey_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let tags = normalize_tags(&input.tags, &existing)
        .map_err(|e| AppError::BadRequest(format!("Invalid tags: {e}")))?;

    // Atomic: D1 batch runs delete + inserts as one transaction.
    repo.replace_response_tags(&survey_id, &response_id, &tags)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = ResponseTagsBody { tags };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

// --- helpers ---

/// Non-empty required query parameter.
fn required_query(req: &Request, name: &str) -> Result<String, AppError> {
    req.url()
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.to_string())
        })
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::BadRequest(format!("Missing query parameter: {name}")))
}

/// Filter by `finished` and/or exact tag membership, compute stats over the
/// FULL filtered set, then apply the offset/limit window. Returns the windowed
/// responses and the exact cross-page stats (whose `total` is the filtered
/// total).
/// Regression: filtering after upstream paging returned short pages and an
/// unfiltered total whenever a filter was set.
fn filter_and_window(
    responses: Vec<FormbricksResponse>,
    want_finished: Option<bool>,
    tag_members: Option<&HashSet<String>>,
    offset: u32,
    limit: u32,
) -> (Vec<FormbricksResponse>, AdminFormbricksStats) {
    let filtered: Vec<FormbricksResponse> = responses
        .into_iter()
        .filter(|r| {
            want_finished.is_none_or(|want| r.finished == want)
                && tag_members.is_none_or(|members| members.contains(&r.id))
        })
        .collect();

    // `latest_submission` uses the same field mapping as the list rows:
    // submitted_at (= createdAt) for finished, updated_at otherwise. The max
    // is lexicographic: upstream timestamps share one canonical ISO-8601 UTC
    // form, so string order equals chronological order.
    // ponytail: parse real RFC3339 (js_sys::Date or a date crate) if upstream
    // ever emits mixed formats/timezones.
    let mut finished_count = 0u64;
    let mut latest: Option<&str> = None;
    for r in &filtered {
        if r.finished {
            finished_count += 1;
        }
        let ts = if r.finished {
            r.created_at.as_str()
        } else {
            r.updated_at.as_str()
        };
        if latest.is_none_or(|cur| ts > cur) {
            latest = Some(ts);
        }
    }
    let stats = AdminFormbricksStats {
        total: filtered.len() as u64,
        finished: finished_count,
        in_progress: filtered.len() as u64 - finished_count,
        latest_submission: latest.map(str::to_string),
    };

    let start = (offset as usize).min(filtered.len());
    let end = start.saturating_add(limit as usize).min(filtered.len());
    (filtered[start..end].to_vec(), stats)
}

// --- tag filtering ---

/// Parsed tag-filter intent from repeated `tag` params + the `untagged` flag.
/// No params at all keeps the pre-filter behavior (everything) for backward
/// compatibility.
#[derive(Debug, Clone, PartialEq)]
enum TagFilter {
    /// No tag/untagged params: no tag filtering.
    None,
    /// Repeated `tag=<label>`: OR — a response matches if it carries ANY label.
    AnyOf(Vec<String>),
    /// `untagged=true`: only responses with zero assigned tags.
    Untagged,
}

/// Parse raw query values (all `tag` occurrences, the `untagged` value if
/// present) into a [`TagFilter`]. Rejects the contradictory combination.
fn parse_tag_filter(tags: Vec<String>, untagged: Option<String>) -> Result<TagFilter, AppError> {
    let tags: Vec<String> = tags.into_iter().filter(|s| !s.is_empty()).collect();
    let untagged = untagged.as_deref().map(|v| v.eq_ignore_ascii_case("true")) == Some(true);
    match (tags.is_empty(), untagged) {
        (true, false) => Ok(TagFilter::None),
        (true, true) => Ok(TagFilter::Untagged),
        (false, true) => Err(AppError::BadRequest(
            "untagged=true cannot be combined with tag filters".to_string(),
        )),
        (false, false) => Ok(TagFilter::AnyOf(tags)),
    }
}

/// Resolve a [`TagFilter`] into the allowed response-ID set, given the
/// survey's (response_id, tag) assignments and the fetched responses.
/// `None` = no filtering.
fn allowed_tag_members(
    filter: &TagFilter,
    tag_rows: &[(String, String)],
    fetched: &[FormbricksResponse],
) -> Option<HashSet<String>> {
    match filter {
        TagFilter::None => None,
        TagFilter::AnyOf(labels) => Some(
            tag_rows
                .iter()
                .filter(|(_, tag)| labels.contains(tag))
                .map(|(rid, _)| rid.clone())
                .collect(),
        ),
        TagFilter::Untagged => {
            // Complement within the fetched set: responses with no assignment row.
            let tagged: HashSet<&str> = tag_rows.iter().map(|(rid, _)| rid.as_str()).collect();
            Some(
                fetched
                    .iter()
                    .map(|r| r.id.clone())
                    .filter(|rid| !tagged.contains(rid.as_str()))
                    .collect(),
            )
        }
    }
}

/// Parsed query parameters for the admin responses list endpoint.
struct ListQuery {
    survey_id: String,
    limit: u32,
    offset: u32,
    finished_filter: Option<bool>,
    tag_filter: TagFilter,
}

fn parse_list_query(req: &Request) -> Result<ListQuery, AppError> {
    let url = req.url().map_err(|e| AppError::Internal(e.to_string()))?;
    let pairs: Vec<(String, String)> = url.query_pairs().into_owned().collect();

    let survey_id = pairs
        .iter()
        .find(|(k, _)| k == "surveyId")
        .map(|(_, v)| v.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("Missing query parameter: surveyId".to_string()))?;

    let limit = pairs
        .iter()
        .find(|(k, _)| k == "limit")
        .and_then(|(_, v)| v.parse::<u32>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let offset = pairs
        .iter()
        .find(|(k, _)| k == "offset")
        .and_then(|(_, v)| v.parse::<u32>().ok())
        .unwrap_or(0);

    let finished_filter = pairs
        .iter()
        .find(|(k, _)| k == "finished")
        .map(|(_, v)| v.to_lowercase())
        .and_then(|s| match s.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });

    let tags: Vec<String> = pairs
        .iter()
        .filter(|(k, _)| k == "tag")
        .map(|(_, v)| v.clone())
        .collect();
    let untagged = pairs
        .iter()
        .find(|(k, _)| k == "untagged")
        .map(|(_, v)| v.clone());
    let tag_filter = parse_tag_filter(tags, untagged)?;

    Ok(ListQuery {
        survey_id,
        limit,
        offset,
        finished_filter,
        tag_filter,
    })
}

/// Build ordered list of (question_id, label, type) preserving survey definition order.
fn build_question_map(survey: &FormbricksSurvey) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for q in &survey.questions {
        let label = q.headline_text();
        if !label.is_empty() {
            out.push((q.id.clone(), label, q.question_type.clone()));
        }
    }
    for block in &survey.blocks {
        for el in &block.elements {
            let label = el.headline_text();
            if !label.is_empty() {
                out.push((el.id.clone(), label, el.question_type.clone()));
            }
        }
    }
    out
}

fn build_answer_items(
    resp: &FormbricksResponse,
    qmap: &[(String, String, String)],
) -> Vec<AdminAnswerItem> {
    let mut answers = Vec::new();
    for (qid, label, qtype) in qmap {
        if let Some(val) = resp.data.get(qid) {
            if val.is_null() {
                continue;
            }
            answers.push(AdminAnswerItem {
                question_id: qid.clone(),
                label: label.clone(),
                type_field: qtype.clone(),
                value: val.clone(),
            });
        }
    }
    answers
}

fn summarize_response(
    resp: &FormbricksResponse,
    survey: &FormbricksSurvey,
    qmap: &[(String, String, String)],
) -> AdminFormbricksResponseSummary {
    let (email, name) = extract_respondent(resp, qmap);

    // preview_answers: first 3 questions with non-empty answers.
    let mut preview = serde_json::Map::new();
    for (qid, label, _) in qmap {
        if preview.len() >= 3 {
            break;
        }
        let vals = extract_answers_list(resp, qid);
        if vals.is_empty() {
            continue;
        }
        let joined = vals.join(", ");
        let short: String = joined.chars().take(80).collect();
        preview.insert(label.clone(), serde_json::Value::String(short));
    }

    let _ = survey; // survey kept in signature for future schema-aware rendering.

    let submitted_at = if resp.finished {
        Some(resp.created_at.clone())
    } else {
        None
    };

    AdminFormbricksResponseSummary {
        id: resp.id.clone(),
        survey_id: resp.survey_id.clone(),
        submitted_at,
        updated_at: Some(resp.updated_at.clone()),
        finished: resp.finished,
        respondent_email: email,
        respondent_name: name,
        preview_answers: serde_json::Value::Object(preview),
        tags: Vec::new(),
    }
}

/// Extract respondent email/name from contact.attributes if present,
/// otherwise scan data values whose question label contains email/name keywords.
fn extract_respondent(
    resp: &FormbricksResponse,
    qmap: &[(String, String, String)],
) -> (Option<String>, Option<String>) {
    let mut email = None;
    let mut name = None;

    if let Some(contact) = &resp.contact {
        if let Some(v) = contact.attributes.get("email") {
            email = v.as_str().map(|s| s.to_string());
        }
        if let Some(v) = contact.attributes.get("name") {
            name = v.as_str().map(|s| s.to_string());
        }
    }

    if email.is_none() || name.is_none() {
        for (qid, label, _) in qmap {
            let lower = label.to_lowercase();
            let vals = extract_answers_list(resp, qid);
            if vals.is_empty() {
                continue;
            }
            let first = vals.first().cloned().unwrap_or_default();
            if email.is_none() && lower.contains("email") {
                email = Some(first.clone());
            } else if name.is_none() && lower.contains("name") {
                name = Some(first);
            }
            if email.is_some() && name.is_some() {
                break;
            }
        }
    }

    (email, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(id: &str, finished: bool) -> FormbricksResponse {
        resp_at(id, finished, "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z")
    }

    fn resp_at(id: &str, finished: bool, created: &str, updated: &str) -> FormbricksResponse {
        FormbricksResponse {
            id: id.to_string(),
            survey_id: "svy".to_string(),
            created_at: created.to_string(),
            updated_at: updated.to_string(),
            finished,
            data: HashMap::new(),
            contact: None,
        }
    }

    fn sample() -> Vec<FormbricksResponse> {
        vec![
            resp("r1", true),
            resp("r2", false),
            resp("r3", true),
            resp("r4", true),
            resp("r5", false),
        ]
    }

    #[test]
    fn finished_filter_applies_before_paging() {
        // Finished set is [r1, r3, r4]; offset 1 limit 2 must window that set,
        // not the raw upstream list. Old code filtered after upstream paging,
        // so offset 1 skipped r2 (unfinished) instead of r1 (finished).
        let (page, stats) = filter_and_window(sample(), Some(true), None, 1, 2);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r3", "r4"]);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn finished_filter_unfinished_page_one() {
        let (page, stats) = filter_and_window(sample(), Some(false), None, 0, 50);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r2", "r5"]);
        assert_eq!(stats.total, 2);
    }

    #[test]
    fn finished_filter_offset_beyond_end_is_empty_with_exact_total() {
        let (page, stats) = filter_and_window(sample(), Some(true), None, 10, 50);
        assert!(page.is_empty());
        // Total reflects the filtered set, not the truncated page.
        assert_eq!(stats.total, 3);
    }

    fn members(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tag_filter_windows_over_full_set_across_pages() {
        // r2 and r5 carry the tag; window over the tagged subset, not the raw
        // upstream page. offset 1 limit 1 → second tagged response only.
        let (page, stats) = filter_and_window(sample(), None, Some(&members(&["r2", "r5"])), 1, 1);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r5"]);
        assert_eq!(stats.total, 2);
    }

    #[test]
    fn tag_filter_no_members_is_empty_with_zero_total() {
        let (page, stats) = filter_and_window(sample(), None, Some(&HashSet::new()), 0, 50);
        assert!(page.is_empty());
        assert_eq!(stats.total, 0);
    }

    #[test]
    fn combined_finished_and_tag_filter_applies_both_before_paging() {
        // r1 (finished), r3 (finished), r5 (unfinished) carry the tag.
        // finished=true ∧ tagged → [r1, r3]; offset 1 → [r3].
        let m = members(&["r1", "r3", "r5"]);
        let (page, stats) = filter_and_window(sample(), Some(true), Some(&m), 1, 50);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r3"]);
        assert_eq!(stats.total, 2);
    }

    #[test]
    fn combined_filter_offset_beyond_end_is_empty_with_exact_total() {
        let m = members(&["r1", "r3", "r5"]);
        let (page, stats) = filter_and_window(sample(), Some(true), Some(&m), 5, 50);
        assert!(page.is_empty());
        assert_eq!(stats.total, 2);
    }

    // --- parse_tag_filter ---

    fn owned(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tag_filter_no_params_is_none() {
        assert_eq!(parse_tag_filter(vec![], None).unwrap(), TagFilter::None);
    }

    #[test]
    fn tag_filter_untagged_false_or_absent_value_is_none() {
        assert_eq!(
            parse_tag_filter(vec![], Some("false".to_string())).unwrap(),
            TagFilter::None
        );
        assert_eq!(
            parse_tag_filter(vec![], Some("0".to_string())).unwrap(),
            TagFilter::None
        );
    }

    #[test]
    fn tag_filter_single_tag_backward_compatible() {
        assert_eq!(
            parse_tag_filter(owned(&["Shortlisted"]), None).unwrap(),
            TagFilter::AnyOf(owned(&["Shortlisted"]))
        );
    }

    #[test]
    fn tag_filter_repeated_tags_parse_as_or_list() {
        assert_eq!(
            parse_tag_filter(owned(&["A", "B"]), None).unwrap(),
            TagFilter::AnyOf(owned(&["A", "B"]))
        );
    }

    #[test]
    fn tag_filter_empty_tag_values_are_dropped() {
        assert_eq!(
            parse_tag_filter(owned(&[""]), None).unwrap(),
            TagFilter::None
        );
        // An empty `tag=` occurrence is not a tag, so it is not a conflict
        // with untagged either.
        assert_eq!(
            parse_tag_filter(owned(&[""]), Some("true".to_string())).unwrap(),
            TagFilter::Untagged
        );
    }

    #[test]
    fn tag_filter_untagged_true_case_insensitive() {
        assert_eq!(
            parse_tag_filter(vec![], Some("TRUE".to_string())).unwrap(),
            TagFilter::Untagged
        );
    }

    #[test]
    fn tag_filter_rejects_tag_plus_untagged() {
        assert!(parse_tag_filter(owned(&["A"]), Some("true".to_string())).is_err());
    }

    // --- allowed_tag_members ---

    fn tag_rows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(r, t)| (r.to_string(), t.to_string()))
            .collect()
    }

    #[test]
    fn allowed_members_none_is_none() {
        assert!(allowed_tag_members(&TagFilter::None, &[], &sample()).is_none());
    }

    #[test]
    fn allowed_members_any_of_unions_labels() {
        let rows = tag_rows(&[("r1", "A"), ("r2", "A"), ("r3", "B"), ("r4", "C")]);
        let m =
            allowed_tag_members(&TagFilter::AnyOf(owned(&["A", "B"])), &rows, &sample()).unwrap();
        assert_eq!(m, members(&["r1", "r2", "r3"]));
    }

    #[test]
    fn allowed_members_exact_label_match() {
        // Case-sensitive exact labels, same as the single-tag behavior.
        let rows = tag_rows(&[("r2", "Shortlisted"), ("r5", "shortlisted")]);
        let m = allowed_tag_members(&TagFilter::AnyOf(owned(&["Shortlisted"])), &rows, &sample())
            .unwrap();
        assert_eq!(m, members(&["r2"]));
    }

    #[test]
    fn allowed_members_untagged_complements_survey_rows() {
        let rows = tag_rows(&[("r2", "X")]);
        let m = allowed_tag_members(&TagFilter::Untagged, &rows, &sample()).unwrap();
        assert_eq!(m, members(&["r1", "r3", "r4", "r5"]));
    }

    #[test]
    fn allowed_members_untagged_ignores_stale_rows() {
        // Tag rows for responses no longer fetched upstream must not leak in.
        let rows = tag_rows(&[("ghost", "X")]);
        let m = allowed_tag_members(&TagFilter::Untagged, &rows, &sample()).unwrap();
        assert_eq!(m, members(&["r1", "r2", "r3", "r4", "r5"]));
    }

    // --- multi-tag / untagged end-to-end over filter_and_window ---

    #[test]
    fn multi_tag_or_filter_applies_before_window() {
        let rows = tag_rows(&[("r1", "A"), ("r3", "B"), ("r5", "A")]);
        let m =
            allowed_tag_members(&TagFilter::AnyOf(owned(&["A", "B"])), &rows, &sample()).unwrap();
        let (page, stats) = filter_and_window(sample(), None, Some(&m), 1, 2);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r3", "r5"]);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn untagged_filter_applies_before_window() {
        let rows = tag_rows(&[("r2", "X"), ("r4", "Y")]);
        let m = allowed_tag_members(&TagFilter::Untagged, &rows, &sample()).unwrap();
        let (page, stats) = filter_and_window(sample(), None, Some(&m), 0, 50);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r1", "r3", "r5"]);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn untagged_combines_with_finished_filter() {
        // Untagged = r1, r3, r5; finished among those = r1, r3.
        let rows = tag_rows(&[("r2", "X"), ("r4", "Y")]);
        let m = allowed_tag_members(&TagFilter::Untagged, &rows, &sample()).unwrap();
        let (page, stats) = filter_and_window(sample(), Some(true), Some(&m), 0, 50);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r1", "r3"]);
        assert_eq!(stats.total, 2);
    }

    /// 120 responses: even i finished (createdAt varies), odd i unfinished with
    /// updatedAt one day later so every unfinished timestamp beats every
    /// finished one. Latest overall = updatedAt of s119 (index 119 > page 1).
    fn big_set() -> Vec<FormbricksResponse> {
        (0..120)
            .map(|i| {
                let created = format!("2026-03-01T{:02}:{:02}:00Z", i / 60, i % 60);
                let updated = if i % 2 == 0 {
                    created.clone()
                } else {
                    format!("2026-03-02T{:02}:{:02}:00Z", i / 60, i % 60)
                };
                resp_at(&format!("s{i:03}"), i % 2 == 0, &created, &updated)
            })
            .collect()
    }

    #[test]
    fn stats_span_all_pages_not_window() {
        let (p1, s1) = filter_and_window(big_set(), None, None, 0, 50);
        let (p2, s2) = filter_and_window(big_set(), None, None, 50, 50);
        let (p3, s3) = filter_and_window(big_set(), None, None, 10_000, 50);
        assert_eq!(p1.len(), 50);
        assert_eq!(p2.len(), 50);
        assert!(p3.is_empty());
        assert_eq!(
            s1,
            AdminFormbricksStats {
                total: 120,
                finished: 60,
                in_progress: 60,
                latest_submission: Some("2026-03-02T01:59:00Z".to_string()),
            }
        );
        // Stats are identical on page 2 and on an empty window past the end.
        assert_eq!(s1, s2);
        assert_eq!(s1, s3);
    }

    #[test]
    fn stats_follow_finished_filter() {
        let (page, stats) = filter_and_window(big_set(), Some(true), None, 0, 10);
        assert_eq!(page.len(), 10);
        // Latest among finished = createdAt of s118; in_progress is 0 because
        // the filter itself excludes unfinished responses.
        assert_eq!(
            stats,
            AdminFormbricksStats {
                total: 60,
                finished: 60,
                in_progress: 0,
                latest_submission: Some("2026-03-01T01:58:00Z".to_string()),
            }
        );
    }

    #[test]
    fn stats_follow_tag_filter_with_unfinished_latest() {
        // s001/s003 unfinished -> latest = updatedAt of s003.
        let m = members(&["s001", "s003"]);
        let (_, stats) = filter_and_window(big_set(), None, Some(&m), 0, 50);
        assert_eq!(
            stats,
            AdminFormbricksStats {
                total: 2,
                finished: 0,
                in_progress: 2,
                latest_submission: Some("2026-03-02T00:03:00Z".to_string()),
            }
        );
    }

    #[test]
    fn stats_empty_set_is_zero_with_null_latest() {
        let (page, stats) = filter_and_window(vec![], None, None, 0, 50);
        assert!(page.is_empty());
        assert_eq!(
            stats,
            AdminFormbricksStats {
                total: 0,
                finished: 0,
                in_progress: 0,
                latest_submission: None,
            }
        );
    }
}
