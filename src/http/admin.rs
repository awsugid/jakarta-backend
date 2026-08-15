use std::collections::HashMap;
use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::formbricks::client::FormbricksClient;
use crate::formbricks::responses::extract_answers_list;
use crate::formbricks::types::{FormbricksResponse, FormbricksSurvey};
use crate::http::errors::AppError;
use crate::http::response::json_success_cors;
use crate::storage::d1::FormRepository;

#[derive(serde::Serialize)]
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

#[derive(serde::Serialize)]
struct AdminFormSummary {
    kind: String,
    slug: String,
    title: String,
    description: Option<String>,
    survey_id: String,
    is_active: bool,
    response_count: Option<u64>,
}

/// GET /api/admin/forms — list all active application_forms (all kinds).
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
        .list_forms(None)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let items: Vec<AdminFormSummary> = forms
        .into_iter()
        .map(|f| AdminFormSummary {
            kind: f.kind,
            slug: f.slug,
            title: f.title,
            description: f.description,
            survey_id: f.formbricks_survey_id,
            is_active: f.is_active,
            response_count: None,
        })
        .collect();

    let resp = json_success_cors(&items, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

#[derive(serde::Serialize)]
struct AdminFormbricksResponseSummary {
    id: String,
    survey_id: String,
    submitted_at: Option<String>,
    updated_at: Option<String>,
    finished: bool,
    respondent_email: Option<String>,
    respondent_name: Option<String>,
    preview_answers: serde_json::Value,
}

#[derive(serde::Serialize)]
struct AdminFormbricksResponseList {
    items: Vec<AdminFormbricksResponseSummary>,
    total: Option<u64>,
    limit: u32,
    offset: u32,
}

/// GET /api/admin/formbricks/responses?surveyId=...&limit=50&offset=0&finished=all|true|false
pub async fn handle_admin_responses(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;
    let origin = req.headers().get("Origin").ok().flatten();

    let (survey_id, limit, offset, finished_filter) = parse_list_query(&req)?;

    let client = FormbricksClient::new(&config);

    // Fetch schema once for label mapping.
    let survey = client
        .get_survey(&survey_id)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch survey: {e}")))?;
    let qmap = build_question_map(&survey);

    let list = client
        .list_responses(&survey_id, limit, offset)
        .await
        .map_err(|e| AppError::FormBricksError(format!("Failed to fetch responses: {e}")))?;

    let total = list.meta.as_ref().and_then(|m| m.total);

    let mut items: Vec<AdminFormbricksResponseSummary> = Vec::new();
    for resp in list.data {
        if let Some(want_finished) = finished_filter {
            if resp.finished != want_finished {
                continue;
            }
        }
        items.push(summarize_response(&resp, &survey, &qmap));
    }

    let body = AdminFormbricksResponseList {
        items,
        total,
        limit,
        offset,
    };
    let resp = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(resp)
}

#[derive(serde::Serialize)]
struct AdminAnswerItem {
    question_id: String,
    label: String,
    #[serde(rename = "type")]
    type_field: String,
    value: serde_json::Value,
}

#[derive(serde::Serialize)]
struct AdminResponseMetadata {
    contact_id: Option<String>,
}

#[derive(serde::Serialize)]
struct AdminFormbricksResponseDetail {
    id: String,
    survey_id: String,
    submitted_at: Option<String>,
    updated_at: Option<String>,
    finished: bool,
    answers: Vec<AdminAnswerItem>,
    metadata: AdminResponseMetadata,
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

    let body = AdminFormbricksResponseDetail {
        id: resp.id.clone(),
        survey_id: resp.survey_id.clone(),
        submitted_at,
        updated_at: Some(resp.updated_at.clone()),
        finished: resp.finished,
        answers,
        metadata: AdminResponseMetadata { contact_id },
    };

    let response = json_success_cors(&body, &config.allowed_origins, origin.as_deref())?;
    Ok(response)
}

// --- helpers ---

fn parse_list_query(req: &Request) -> Result<(String, u32, u32, Option<bool>), AppError> {
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

    Ok((survey_id, limit, offset, finished_filter))
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
