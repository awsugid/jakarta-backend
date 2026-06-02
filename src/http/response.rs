use serde::Serialize;
use worker::{Headers, Response};

/// Returns a 200 JSON response with CORS headers.
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

/// Adds CORS headers to an existing response.
pub fn with_cors(response: Response, allowed_origins: &str) -> worker::Result<Response> {
    let headers = cors_headers(allowed_origins);
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
    let _ = headers.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, DELETE, OPTIONS",
    );
    let _ = headers.set(
        "Access-Control-Allow-Headers",
        "Content-Type, Authorization, X-Debug-User-Email",
    );
    headers
}
