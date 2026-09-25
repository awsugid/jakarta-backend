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

    // Batch-fetch response counts in a single query.
    let form_ids: Vec<&str> = forms.iter().map(|f| f.id.as_str()).collect();
    let counts = repo
        .count_responses_by_form_ids(&form_ids)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let items: Vec<AdminFormSummary> = forms
        .into_iter()
        .map(|f| {
            let response_count = counts.get(&f.id).copied();
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

    let counts = repo
        .count_responses_by_form_ids(&[updated.id.as_str()])
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let summary = AdminFormSummary {
        kind: updated.kind,
        slug: updated.slug,
        title: updated.title,
        description: updated.description,
        survey_id: updated.formbricks_survey_id,
        is_active: updated.is_active,
        response_count: counts.get(&updated.id).copied(),
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
}

/// GET /api/admin/formbricks/responses?surveyId=...&limit=50&offset=0&finished=all|true|false&tag=<label>
///
/// `offset` is translated to upstream `skip` by the client. Upstream v2 has no
/// `finished` or tag filter, so any filtered view walks every page and applies
/// the filter BEFORE paging locally (filter-then-window), keeping totals exact.
/// Unfiltered views keep upstream fast paging (limit/offset passthrough).
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

    // Exact-label tag filter: membership set of response IDs carrying the tag.
    let tag_members: Option<HashSet<String>> = tag_filter.map(|want| {
        tag_rows
            .iter()
            .filter(|(_, tag)| tag == &want)
            .map(|(rid, _)| rid.clone())
            .collect()
    });

    // ponytail: any filtered view walks every upstream page (100 pages = 10k
    // responses at limit 100) because upstream cannot filter by `finished` or
    // tag. Replace with upstream filters if they are ever added to the v2 API.
    const FINISHED_FILTER_MAX_WALK_PAGES: u32 = 100;

    let (data, total) = if finished_filter.is_some() || tag_members.is_some() {
        let all = client
            .get_all_responses(&survey_id, FINISHED_FILTER_MAX_WALK_PAGES)
            .await
            .map_err(|e| AppError::FormBricksError(format!("Failed to fetch responses: {e}")))?;
        let (window, filtered_total) =
            filter_and_window(all, finished_filter, tag_members.as_ref(), offset, limit);
        (window, Some(filtered_total))
    } else {
        let list = client
            .list_responses(&survey_id, limit, offset)
            .await
            .map_err(|e| AppError::FormBricksError(format!("Failed to fetch responses: {e}")))?;
        let total = list.meta.as_ref().and_then(|m| m.total);
        (list.data, total)
    };

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

/// Filter by `finished` and/or exact tag membership BEFORE paging, then apply
/// the offset/limit window. Returns the windowed responses and the exact
/// filtered total.
/// Regression: filtering after upstream paging returned short pages and an
/// unfiltered total whenever a filter was set.
fn filter_and_window(
    responses: Vec<FormbricksResponse>,
    want_finished: Option<bool>,
    tag_members: Option<&HashSet<String>>,
    offset: u32,
    limit: u32,
) -> (Vec<FormbricksResponse>, u64) {
    let filtered: Vec<FormbricksResponse> = responses
        .into_iter()
        .filter(|r| {
            want_finished.is_none_or(|want| r.finished == want)
                && tag_members.is_none_or(|members| members.contains(&r.id))
        })
        .collect();
    let total = filtered.len() as u64;
    let start = (offset as usize).min(filtered.len());
    let end = start.saturating_add(limit as usize).min(filtered.len());
    (filtered[start..end].to_vec(), total)
}

/// Parsed query parameters for the admin responses list endpoint.
struct ListQuery {
    survey_id: String,
    limit: u32,
    offset: u32,
    finished_filter: Option<bool>,
    tag_filter: Option<String>,
}

fn parse_list_query(req: &Request) -> Result<ListQuery, AppError> {
    let url = req.url().map_err(|e| AppError::Internal(e.to_string()))?;
    let qs: HashMap<String, String> = url.query_pairs().into_owned().collect();

    let survey_id = qs
        .get("surveyId")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::BadRequest("Missing query parameter: surveyId".to_string()))?;

    let limit = qs
        .get("limit")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let offset = qs
        .get("offset")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let finished_filter = match qs.get("finished").map(|s| s.to_lowercase()) {
        Some(s) if s == "true" => Some(true),
        Some(s) if s == "false" => Some(false),
        _ => None,
    };

    // Exact-label tag filter (case-sensitive match on assigned labels).
    let tag_filter = qs.get("tag").filter(|s| !s.is_empty()).cloned();

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
        FormbricksResponse {
            id: id.to_string(),
            survey_id: "svy".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
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
        let (page, total) = filter_and_window(sample(), Some(true), None, 1, 2);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r3", "r4"]);
        assert_eq!(total, 3);
    }

    #[test]
    fn finished_filter_unfinished_page_one() {
        let (page, total) = filter_and_window(sample(), Some(false), None, 0, 50);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r2", "r5"]);
        assert_eq!(total, 2);
    }

    #[test]
    fn finished_filter_offset_beyond_end_is_empty_with_exact_total() {
        let (page, total) = filter_and_window(sample(), Some(true), None, 10, 50);
        assert!(page.is_empty());
        // Total reflects the filtered set, not the truncated page.
        assert_eq!(total, 3);
    }

    fn members(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn tag_filter_windows_over_full_set_across_pages() {
        // r2 and r5 carry the tag; window over the tagged subset, not the raw
        // upstream page. offset 1 limit 1 → second tagged response only.
        let (page, total) = filter_and_window(sample(), None, Some(&members(&["r2", "r5"])), 1, 1);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r5"]);
        assert_eq!(total, 2);
    }

    #[test]
    fn tag_filter_no_members_is_empty_with_zero_total() {
        let (page, total) = filter_and_window(sample(), None, Some(&HashSet::new()), 0, 50);
        assert!(page.is_empty());
        assert_eq!(total, 0);
    }

    #[test]
    fn combined_finished_and_tag_filter_applies_both_before_paging() {
        // r1 (finished), r3 (finished), r5 (unfinished) carry the tag.
        // finished=true ∧ tagged → [r1, r3]; offset 1 → [r3].
        let m = members(&["r1", "r3", "r5"]);
        let (page, total) = filter_and_window(sample(), Some(true), Some(&m), 1, 50);
        let ids: Vec<&str> = page.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["r3"]);
        assert_eq!(total, 2);
    }

    #[test]
    fn combined_filter_offset_beyond_end_is_empty_with_exact_total() {
        let m = members(&["r1", "r3", "r5"]);
        let (page, total) = filter_and_window(sample(), Some(true), Some(&m), 5, 50);
        assert!(page.is_empty());
        assert_eq!(total, 2);
    }
}
