use super::types::FormbricksResponse;

/// Extract the answer for a specific question ID from a FormBricks response.
///
/// Returns `None` if the question was not answered or the value is null.
pub fn extract_answer(response: &FormbricksResponse, question_id: &str) -> Option<String> {
    response.data.get(question_id).and_then(|v| {
        if v.is_null() {
            return None;
        }
        if let Some(s) = v.as_str() {
            return Some(s.to_string());
        }
        Some(v.to_string())
    })
}

/// Check if a response has a non-empty answer for a question.
pub fn has_answer(response: &FormbricksResponse, question_id: &str) -> bool {
    extract_answer(response, question_id)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}
