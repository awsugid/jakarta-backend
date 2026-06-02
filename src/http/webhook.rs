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
use crate::storage::d1::FormRepository;
use crate::validation::email::normalize_email;
use crate::validation::linkedin::normalize_linkedin_url;

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

    // 8. Find corresponding form record by survey ID
    // We scan forms to find the one with matching formbricks_survey_id
    let forms = match repo.list_forms(None).await {
        Ok(f) => f,
        Err(e) => {
            console_log!("Failed to list forms in webhook: {}", e);
            return Response::ok("Database list error");
        }
    };

    let form = match forms
        .into_iter()
        .find(|f| f.formbricks_survey_id == payload.data.survey_id)
    {
        Some(f) => f,
        None => {
            console_log!(
                "Webhook received for unknown survey ID: {}",
                payload.data.survey_id
            );
            return Response::ok("Unknown survey");
        }
    };

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

    // 9. Scan for duplicate LinkedIn URLs in previous responses
    let client = FormbricksClient::new(&config);
    let responses = match client
        .get_all_responses(&form.formbricks_survey_id, 10)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            console_log!(
                "Failed to fetch responses from FormBricks for duplicate check: {}",
                e
            );
            return Response::ok("FormBricks responses fetch error");
        }
    };

    let mut duplicate_found = false;
    for response in &responses {
        // Exclude the current response being processed by the webhook
        if response.id == payload.data.id {
            continue;
        }

        let linkedin_answers = extract_answers_list(response, &form.linkedin_question_id);
        for url in linkedin_answers {
            if let Ok(existing_linkedin) = normalize_linkedin_url(&url) {
                if existing_linkedin == normalized_proposed_linkedin {
                    // Check if this duplicate belongs to another user
                    let email_answers = extract_answers_list(response, &form.email_question_id);
                    let is_same_user = email_answers
                        .iter()
                        .any(|e| normalize_email(e) == normalized_proposed_email);

                    if !is_same_user {
                        duplicate_found = true;
                        break;
                    }
                }
            }
        }
        if duplicate_found {
            break;
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
    }

    Response::ok("Processed")
}
