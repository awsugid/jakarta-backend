use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::collections::HashMap;
use worker::*;

use crate::config::AppConfig;
use crate::formbricks::client::FormbricksClient;
use crate::formbricks::responses::extract_answers_list;
use crate::storage::d1::{ApplicationResponseIndex, FormRepository, ResponseTagRepository};
use crate::validation::email::normalize_email;
use crate::validation::linkedin::normalize_linkedin_url;
use crate::validation::tags::normalize_tags;

type HmacSha256 = Hmac<Sha256>;

/// Payload structure for a FormBricks webhook event.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebhookPayload {
    event: String,
    data: WebhookResponseData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebhookResponseData {
    id: String,
    survey_id: String,
    finished: bool,
    data: HashMap<String, serde_json::Value>,
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
}

/// Verify signature and process the FormBricks webhook.
pub async fn handle_webhook(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = match AppConfig::from_env(&ctx.env) {
        Ok(c) => c,
        Err(e) => return Response::error(format!("Config error: {}", e), 500),
    };

    // 1. Get signature headers
    let headers = req.headers();
    let webhook_id = match headers.get("webhook-id")? {
        Some(id) => id,
        None => return Response::error("Missing webhook-id header", 401),
    };
    let webhook_timestamp = match headers.get("webhook-timestamp")? {
        Some(t) => t,
        None => return Response::error("Missing webhook-timestamp header", 401),
    };
    let webhook_signature = match headers.get("webhook-signature")? {
        Some(s) => s,
        None => return Response::error("Missing webhook-signature header", 401),
    };

    // 2. Verify timestamp tolerance (5 minutes = 300 seconds)
    let timestamp: i64 = match webhook_timestamp.parse() {
        Ok(t) => t,
        Err(_) => return Response::error("Invalid webhook-timestamp header", 401),
    };
    let now = (js_sys::Date::now() / 1000.0) as i64;
    if (now - timestamp).abs() > 300 {
        return Response::error("Webhook timestamp outside tolerance window", 401);
    }

    // 3. Read raw request body
    let raw_body = req.text().await?;

    // 4. Verify signature
    let secret = &config.formbricks_webhook_secret;
    if secret.is_empty() {
        // If webhook secret is not configured, deny requests as a security precaution
        return Response::error("Webhook signing secret not configured", 401);
    }

    let secret_base64 = secret.strip_prefix("whsec_").unwrap_or(secret);
    let secret_bytes = match BASE64.decode(secret_base64) {
        Ok(b) => b,
        Err(_) => return Response::error("Invalid webhook signing secret format", 401),
    };

    let signed_content = format!("{}.{}.{}", webhook_id, webhook_timestamp, raw_body);
    let mut mac = match HmacSha256::new_from_slice(&secret_bytes) {
        Ok(m) => m,
        Err(_) => return Response::error("Internal error initializing signature check", 500),
    };
    mac.update(signed_content.as_bytes());
    let expected_tag = mac.finalize().into_bytes();
    let expected_signature = BASE64.encode(expected_tag);

    // Header signature format is "v1,signature_base64"
    let signature_parts: Vec<&str> = webhook_signature.split(',').collect();
    if signature_parts.len() < 2 || signature_parts[0] != "v1" {
        return Response::error("Invalid webhook signature format", 401);
    }
    let received_signature = signature_parts[1];

    // Constant-time or robust string comparison
    if expected_signature != received_signature {
        return Response::error("Invalid webhook signature", 401);
    }

    // 5. Parse payload
    let payload: WebhookPayload = match serde_json::from_str(&raw_body) {
        Ok(p) => p,
        Err(e) => {
            console_log!("Failed to parse webhook JSON body: {}", e);
            return Response::ok("Invalid payload format"); // return 200 to stop retry storms
        }
    };

    // 6. Process only responseFinished with finished: true
    if payload.event != "responseFinished" || !payload.data.finished {
        return Response::ok("Event ignored");
    }

    // 7. Extract database repository
    let db = match ctx.d1("DB") {
        Ok(d) => d,
        Err(e) => {
            console_log!("D1 database connection failed in webhook: {}", e);
            return Response::ok("Database error");
        }
    };
    let repo = FormRepository::new(db);

    // 8. Find corresponding form record by survey ID (any is_active state:
    // Talent Pool volunteer forms are inactive but must still be indexed).
    let form = match repo.get_form_by_survey_id(&payload.data.survey_id).await {
        Ok(f) => f,
        Err(e) => {
            console_log!("Failed to look up form by survey ID in webhook: {}", e);
            return Response::ok("Database lookup error");
        }
    };
    let form = match form {
        Some(f) => f,
        None => {
            console_log!(
                "Webhook received for unknown survey ID: {}",
                payload.data.survey_id
            );
            return Response::ok("Unknown survey");
        }
    };

    // Inactive non-volunteer forms stay fully closed: ignore their submissions
    // (matches the pre-Talent-Pool behavior where inactive forms were
    // unreachable through the active-only form scan).
    if !form.is_active && form.kind != "volunteer" {
        console_log!(
            "Webhook ignored for inactive non-volunteer form kind={} slug={}",
            form.kind,
            form.slug
        );
        return Response::ok("Ignored inactive form");
    }

    // Create a temporary mock response to use our robust extract_answers_list helper
    let mock_response = crate::formbricks::types::FormbricksResponse {
        id: payload.data.id.clone(),
        survey_id: payload.data.survey_id.clone(),
        created_at: String::new(),
        updated_at: String::new(),
        finished: payload.data.finished,
        data: payload.data.data.clone(),
        contact: None,
    };

    let proposed_linkedin_answers =
        extract_answers_list(&mock_response, &form.linkedin_question_id);
    let proposed_email_answers = extract_answers_list(&mock_response, &form.email_question_id);

    let proposed_linkedin = proposed_linkedin_answers
        .first()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let proposed_email = proposed_email_answers
        .first()
        .map(|s| s.to_string())
        .unwrap_or_default();

    if proposed_linkedin.is_empty() || proposed_email.is_empty() {
        console_log!("Webhook missing email or LinkedIn response answers");
        return Response::ok("Missing response fields");
    }

    let normalized_proposed_linkedin = match normalize_linkedin_url(&proposed_linkedin) {
        Ok(l) => l,
        Err(e) => {
            console_log!("Failed to normalize proposed LinkedIn URL: {}", e);
            return Response::ok("Invalid LinkedIn URL");
        }
    };
    let normalized_proposed_email = normalize_email(&proposed_email);

    // 9. Check D1 index for duplicate LinkedIn URLs (Volunteers only)
    let client = FormbricksClient::new(&config);
    let is_volunteer = form.kind == "volunteer";
    let mut duplicate_found = false;

    if is_volunteer {
        let existing_linkedin_index = match repo
            .get_index_by_form_linkedin(&form.id, &normalized_proposed_linkedin)
            .await
        {
            Ok(idx) => idx,
            Err(e) => {
                console_log!("Failed to query duplicate LinkedIn from D1: {}", e);
                return Response::ok("Database error");
            }
        };

        if let Some(existing) = existing_linkedin_index {
            // Exclude the current response being processed (in case of retry)
            if existing.formbricks_response_id != payload.data.id
                && existing.normalized_email != normalized_proposed_email
            {
                duplicate_found = true;
            }
        }
    }

    if duplicate_found {
        console_log!(
            "Duplicate LinkedIn URL detected for response ID: {}, survey ID: {}. Deleting response.",
            payload.data.id,
            form.formbricks_survey_id
        );

        if let Err(e) = client.delete_response(&payload.data.id).await {
            console_log!("Failed to delete duplicate FormBricks response: {}", e);
        } else {
            console_log!(
                "Successfully deleted duplicate FormBricks response {}",
                payload.data.id
            );
        }

        // We can optionally add it to the index as duplicate_deleted, or just skip it
        // We will skip inserting it to the active index.
    } else {
        // No duplicate from another user, upsert into index
        let new_index = ApplicationResponseIndex {
            id: format!("idx_{}", payload.data.id),
            form_id: form.id.clone(),
            formbricks_survey_id: form.formbricks_survey_id.clone(),
            formbricks_response_id: payload.data.id.clone(),
            normalized_email: normalized_proposed_email.clone(),
            normalized_linkedin_url: Some(normalized_proposed_linkedin.clone()),
            finished: payload.data.finished,
            status: "active".to_string(),
            submitted_at: Some(payload.data.created_at.clone().unwrap_or_default()), // WebhookResponseData missing created_at, wait let's check
            created_at: String::new(),
            updated_at: String::new(),
        };

        if let Err(e) = repo.upsert_active_response_index(&new_index).await {
            console_log!("Failed to upsert active response index: {}", e);
            return Response::ok("Database error");
        }

        // Talent Pool: a volunteer form that is inactive at submission time
        // means the applicant joined the pool. The tag is required bookkeeping:
        // a failure must fail the delivery (5xx) so Formbricks retries. The
        // index upsert above is idempotent, so retries are safe — they re-index
        // and re-tag the same response without duplicating anything. Manual
        // tags are never touched (additive insert only), and old responses are
        // never reclassified (only responseFinished is handled).
        if form.kind == "volunteer" && !form.is_active {
            let tagged = match ctx.d1("DB") {
                Ok(d) => {
                    tag_talent_pool(
                        &ResponseTagRepository::new(d),
                        &form.formbricks_survey_id,
                        &payload.data.id,
                    )
                    .await
                }
                Err(e) => Err(format!("D1 binding failed: {e}")),
            };
            if let Err(reason) = tagged {
                console_log!(
                    "Talent Pool tag failed for response {} survey {}: {} — failing delivery for retry",
                    payload.data.id,
                    form.formbricks_survey_id,
                    reason
                );
                return Response::error("Talent Pool tag failed", 500);
            }
        }

        // Delete old responses in the index for the same form + email (enforce 1-per-user for volunteers)
        if is_volunteer {
            if let Err(e) = repo
                .delete_old_response_index(&form.id, &normalized_proposed_email, &payload.data.id)
                .await
            {
                console_log!("Failed to delete old response index (edit cleanup): {}", e);
            }
        }
    }

    Response::ok("Processed")
}

/// Canonical label for talent-pool submissions recorded by the webhook.
const TALENT_POOL_TAG: &str = "Talent Pool";

/// Add the Talent Pool tag to a response without touching its other tags.
/// Reuses the survey's established spelling of the label when one exists
/// (same canonicalization as the admin tags endpoint).
/// Returns Err(reason) when the tag could not be recorded — the caller must
/// then fail the delivery so Formbricks retries.
async fn tag_talent_pool(
    repo: &ResponseTagRepository,
    survey_id: &str,
    response_id: &str,
) -> Result<(), String> {
    let existing = repo
        .list_survey_tags(survey_id)
        .await
        .map_err(|e| format!("survey tag list failed: {e}"))?;
    let mut labels = normalize_tags(&[TALENT_POOL_TAG.to_string()], &existing)
        .map_err(|e| format!("tag normalization failed: {e}"))?;
    let label = labels.remove(0);
    repo.add_response_tag_if_missing(survey_id, response_id, &label)
        .await
        .map_err(|e| format!("tag insert failed: {e}"))?;
    Ok(())
}
