use worker::*;

use crate::auth::admin::require_admin;
use crate::config::AppConfig;
use crate::formbricks::client::FormbricksClient;
use crate::formbricks::responses::extract_answers_list;
use crate::http::errors::AppError;
use crate::storage::d1::{ApplicationResponseIndex, FormRepository};
use crate::validation::email::normalize_email;
use crate::validation::linkedin::normalize_linkedin_url;

#[derive(serde::Serialize)]
struct ReingestResult {
    success: bool,
    total_forms: usize,
    total_responses_processed: usize,
    total_responses_indexed: usize,
    total_responses_skipped: usize,
    errors: Vec<String>,
}

/// Re-ingest all responses from Formbricks into the application_response_index table.
///
/// Admin-only. Fetches all active forms, retrieves all responses from Formbricks,
/// and populates/updates the D1 index.
pub async fn handle_reingest(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let config = AppConfig::from_env(&ctx.env).map_err(|e| AppError::Internal(e.to_string()))?;
    let db_opt = ctx.d1("DB").ok();
    require_admin(&req, &config, db_opt.as_ref()).await?;

    let db = ctx
        .d1("DB")
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let repo = FormRepository::new(db);
    let client = FormbricksClient::new(&config);

    console_log!("Starting re-ingestion of application_response_index...");

    // Fetch all active forms
    let forms = repo
        .list_forms(None)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list forms: {}", e)))?;

    let total_forms = forms.len();
    console_log!("Found {} active forms to process", total_forms);

    let mut total_responses_processed = 0;
    let mut total_responses_indexed = 0;
    let mut total_responses_skipped = 0;
    let mut errors: Vec<String> = Vec::new();

    // Process each form
    for form in forms {
        console_log!(
            "Processing form: {} (kind: {}, slug: {})",
            form.title,
            form.kind,
            form.slug
        );

        // Fetch all responses for this form's survey
        let responses = match client
            .get_all_responses(&form.formbricks_survey_id, 50)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let error_msg = format!(
                    "Failed to fetch responses for form {} (survey {}): {}",
                    form.id, form.formbricks_survey_id, e
                );
                console_log!("{}", error_msg);
                errors.push(error_msg);
                continue;
            }
        };

        console_log!(
            "Fetched {} responses for form {}",
            responses.len(),
            form.title
        );

        // Process each response
        for response in responses {
            total_responses_processed += 1;

            // Only process finished responses
            if !response.finished {
                total_responses_skipped += 1;
                continue;
            }

            // Extract email and LinkedIn
            let email_answers = extract_answers_list(&response, &form.email_question_id);
            let linkedin_answers = extract_answers_list(&response, &form.linkedin_question_id);

            let email = email_answers
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let linkedin = linkedin_answers
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default();

            // Skip if missing email or LinkedIn
            if email.is_empty() || linkedin.is_empty() {
                console_log!(
                    "Skipping response {} - missing email or LinkedIn",
                    response.id
                );
                total_responses_skipped += 1;
                continue;
            }

            // Normalize
            let normalized_email = normalize_email(&email);
            let normalized_linkedin = match normalize_linkedin_url(&linkedin) {
                Ok(l) => l,
                Err(e) => {
                    console_log!(
                        "Skipping response {} - invalid LinkedIn URL: {}",
                        response.id,
                        e
                    );
                    total_responses_skipped += 1;
                    continue;
                }
            };

            // Create index entry
            let index = ApplicationResponseIndex {
                id: format!("idx_{}", response.id),
                form_id: form.id.clone(),
                formbricks_survey_id: form.formbricks_survey_id.clone(),
                formbricks_response_id: response.id.clone(),
                normalized_email: normalized_email.clone(),
                normalized_linkedin_url: Some(normalized_linkedin.clone()),
                finished: response.finished,
                status: "active".to_string(),
                submitted_at: Some(response.created_at.clone()),
                created_at: String::new(),
                updated_at: String::new(),
            };

            // Upsert into index
            match repo.upsert_active_response_index(&index).await {
                Ok(_) => {
                    total_responses_indexed += 1;
                }
                Err(e) => {
                    let error_msg =
                        format!("Failed to upsert index for response {}: {}", response.id, e);
                    console_log!("{}", error_msg);
                    errors.push(error_msg);
                }
            }
        }
    }

    console_log!("Re-ingestion complete!");
    console_log!("Total forms processed: {}", total_forms);
    console_log!("Total responses processed: {}", total_responses_processed);
    console_log!("Total responses indexed: {}", total_responses_indexed);
    console_log!("Total responses skipped: {}", total_responses_skipped);
    console_log!("Total errors: {}", errors.len());

    let result = ReingestResult {
        success: true,
        total_forms,
        total_responses_processed,
        total_responses_indexed,
        total_responses_skipped,
        errors,
    };

    Response::from_json(&result)
}
