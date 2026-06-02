use worker::*;

mod application;
mod auth;
mod config;
mod formbricks;
mod http;
mod storage;
mod validation;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();
    let res = http::routes::register_routes(router)
        .run(req, env.clone())
        .await;

    match res {
        Ok(response) => Ok(response),
        Err(err) => {
            console_error!("Route error: {:?}", err);

            let allowed_origins = env
                .var("ALLOWED_ORIGINS")
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "*".to_string());

            let (status, message) = match &err {
                worker::Error::RustError(msg) => {
                    if msg.starts_with("401 (") {
                        (401, msg.as_str())
                    } else if msg.starts_with("403 (") {
                        (403, msg.as_str())
                    } else if msg.starts_with("400 (") {
                        (400, msg.as_str())
                    } else if msg.starts_with("404 (") {
                        (404, msg.as_str())
                    } else if msg.starts_with("502 (") {
                        (502, msg.as_str())
                    } else {
                        (500, msg.as_str())
                    }
                }
                _ => (500, "Internal Server Error"),
            };

            let clean_message = if let Some(idx) = message.find("): ") {
                &message[idx + 3..]
            } else {
                message
            };

            let body = serde_json::json!({
                "error": {
                    "code": status,
                    "message": clean_message
                }
            });

            let response = Response::from_json(&body)?;

            let headers = Headers::new();
            let _ = headers.set("Content-Type", "application/json");
            let _ = headers.set("Access-Control-Allow-Origin", &allowed_origins);
            let _ = headers.set(
                "Access-Control-Allow-Methods",
                "GET, POST, PUT, DELETE, OPTIONS",
            );
            let _ = headers.set(
                "Access-Control-Allow-Headers",
                "Content-Type, Authorization, X-Debug-User-Email",
            );

            Ok(response.with_headers(headers).with_status(status))
        }
    }
}
