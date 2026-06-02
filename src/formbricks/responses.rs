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

/// Extract all answers for a specific question ID from a FormBricks response,
/// supporting both single string values and arrays of strings.
pub fn extract_answers_list(response: &FormbricksResponse, question_id: &str) -> Vec<String> {
    match response.data.get(question_id) {
        None => Vec::new(),
        Some(v) => {
            if v.is_null() {
                Vec::new()
            } else if let Some(s) = v.as_str() {
                vec![s.to_string()]
            } else if let Some(arr) = v.as_array() {
                arr.iter()
                    .filter_map(|val| {
                        if val.is_null() {
                            None
                        } else if let Some(s) = val.as_str() {
                            Some(s.to_string())
                        } else {
                            let s_raw = val.to_string();
                            let s_trimmed = s_raw.trim_matches('"');
                            if s_trimmed.is_empty() {
                                None
                            } else {
                                Some(s_trimmed.to_string())
                            }
                        }
                    })
                    .collect()
            } else {
                let s_raw = v.to_string();
                let s_trimmed = s_raw.trim_matches('"');
                if s_trimmed.is_empty() {
                    Vec::new()
                } else {
                    vec![s_trimmed.to_string()]
                }
            }
        }
    }
}

/// Check if a response has a non-empty answer for a question.
#[allow(dead_code)]
pub fn has_answer(response: &FormbricksResponse, question_id: &str) -> bool {
    extract_answer(response, question_id)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_extract_answers_list_single_string() {
        let mut data = HashMap::new();
        data.insert(
            "q-1".to_string(),
            serde_json::Value::String("hello".to_string()),
        );
        let resp = FormbricksResponse {
            id: "r1".to_string(),
            survey_id: "s1".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            finished: true,
            data,
            contact: None,
        };
        assert_eq!(
            extract_answers_list(&resp, "q-1"),
            vec!["hello".to_string()]
        );
    }

    #[test]
    fn test_extract_answers_list_array_strings() {
        let mut data = HashMap::new();
        data.insert(
            "q-1".to_string(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("first".to_string()),
                serde_json::Value::String("second".to_string()),
            ]),
        );
        let resp = FormbricksResponse {
            id: "r1".to_string(),
            survey_id: "s1".to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            finished: true,
            data,
            contact: None,
        };
        assert_eq!(
            extract_answers_list(&resp, "q-1"),
            vec!["first".to_string(), "second".to_string()]
        );
    }
}
