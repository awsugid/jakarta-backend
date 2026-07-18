use serde::Serialize;
use worker::{Headers, Response};

/// Returns a 200 JSON response with wildcard CORS (for PUBLIC endpoints).
pub fn json_success(data: &impl Serialize) -> worker::Result<Response> {
    let response = Response::from_json(data)?;
    let headers = cors_headers("*");
    Ok(response.with_headers(headers))
}

/// Returns an error JSON response with the given status code and CORS headers.
#[allow(dead_code)]
pub fn json_error(status: u16, message: &str) -> worker::Result<Response> {
    let body = serde_json::json!({
        "error": {
            "code": status,
            "message": message
        }
    });
    let response = Response::from_json(&body)?;
    let headers = cors_headers("*");
    Ok(response.with_headers(headers).with_status(status))
}

/// 200 JSON for authenticated endpoints: reflects the request origin only if allowed.
pub fn json_success_cors(
    data: &impl Serialize,
    allowed_origins: &str,
    request_origin: Option<&str>,
) -> worker::Result<Response> {
    let response = Response::from_json(data)?;
    let headers = cors_headers_reflected(allowed_origins, request_origin);
    Ok(response.with_headers(headers))
}

/// Adds CORS headers to an existing response (existing wildcard/list helper, unchanged).
pub fn with_cors(response: Response, allowed_origins: &str) -> worker::Result<Response> {
    let headers = cors_headers(allowed_origins);
    Ok(response.with_headers(headers))
}

/// Adds reflected-origin CORS headers to an existing response.
#[allow(dead_code)]
pub fn with_cors_origin(
    response: Response,
    allowed_origins: &str,
    request_origin: Option<&str>,
) -> worker::Result<Response> {
    let headers = cors_headers_reflected(allowed_origins, request_origin);
    Ok(response.with_headers(headers))
}

/// Returns a 204 response for CORS preflight (OPTIONS) requests.
pub fn cors_preflight(allowed_origins: &str) -> worker::Result<Response> {
    let headers = cors_headers(allowed_origins);
    let response = Response::empty()?;
    Ok(response.with_headers(headers).with_status(204))
}

fn cors_headers(allowed_origins: &str) -> Headers {
    let headers = Headers::new();
    // Ignore errors from setting headers; in a Worker environment these are always valid.
    let _ = headers.set("Access-Control-Allow-Origin", allowed_origins);
    cors_common(&headers);
    headers
}

fn cors_headers_reflected(allowed_origins: &str, request_origin: Option<&str>) -> Headers {
    let headers = Headers::new();
    let origin = resolve_origin(allowed_origins, request_origin);
    if let Some(o) = origin {
        let _ = headers.set("Access-Control-Allow-Origin", &o);
        let _ = headers.set("Vary", "Origin");
    }
    cors_common(&headers);
    headers
}

fn cors_common(headers: &Headers) {
    let _ = headers.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, DELETE, OPTIONS",
    );
    let _ = headers.set(
        "Access-Control-Allow-Headers",
        "Content-Type, Authorization, X-Debug-User-Email",
    );
}

/// Decide which Origin value to send back. Wildcard ("*" or empty allowed_origins)
/// yields "*". Otherwise reflect request_origin only if it appears in the allow-list.
fn resolve_origin(allowed: &str, req_origin: Option<&str>) -> Option<String> {
    let trimmed = allowed.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Some("*".to_string());
    }
    let req = req_origin?;
    let req_trim = req.trim();
    if req_trim.is_empty() {
        return None;
    }
    let allowed_set: Vec<&str> = trimmed
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if allowed_set.iter().any(|a| *a == "*" || *a == req_trim) {
        Some(req_trim.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_origin;

    #[test]
    fn wildcard_when_allowed_is_star() {
        assert_eq!(
            resolve_origin("*", Some("https://evil.example")),
            Some("*".to_string())
        );
    }

    #[test]
    fn wildcard_when_allowed_empty() {
        assert_eq!(
            resolve_origin("", Some("https://evil.example")),
            Some("*".to_string())
        );
    }

    #[test]
    fn reflects_allowed_origin() {
        let list = "https://jakarta.awscommunity.id, http://localhost:4321";
        assert_eq!(
            resolve_origin(list, Some("http://localhost:4321")),
            Some("http://localhost:4321".to_string())
        );
    }

    #[test]
    fn omits_disallowed_origin() {
        let list = "https://jakarta.awscommunity.id";
        assert_eq!(resolve_origin(list, Some("https://evil.example")), None);
    }

    #[test]
    fn none_when_no_request_origin() {
        let list = "https://jakarta.awscommunity.id";
        assert_eq!(resolve_origin(list, None), None);
    }
}
