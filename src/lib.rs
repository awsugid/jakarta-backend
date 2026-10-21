use worker::*;

mod application;
mod auth;
mod config;
mod formbricks;
mod http;
mod links;
mod pretix;
mod schedule;
mod sessionize;
mod speakers;
mod sponsors;
mod statistics;
mod storage;
mod validation;

#[event(scheduled)]
async fn scheduled(_event: ScheduledEvent, env: Env, ctx: ScheduleContext) {
    ctx.wait_until(async move {
        if let Err(error) = refresh_sessionize(env).await {
            console_error!("Daily Sessionize import failed: {}", error);
            panic!("Daily Sessionize import failed: {}", error);
        }
    });
}

// ponytail: one Sessionize event; add per-event source settings when another event needs daily ingestion.
async fn refresh_sessionize(env: Env) -> worker::Result<()> {
    let event_slug = env.var("SESSIONIZE_EVENT_SLUG")?.to_string();
    if event_slug.is_empty()
        || event_slug.len() > 100
        || !event_slug
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(worker::Error::RustError(
            "Invalid SESSIONIZE_EVENT_SLUG".into(),
        ));
    }
    let db = env.d1("DB")?;
    let payload = sessionize::client::SessionizeClient::default()
        .get_data(&db, true)
        .await
        .map_err(worker::Error::RustError)?;
    console_log!(
        "Sessionize snapshot refreshed: event={} speakers={} sessions={}",
        event_slug,
        payload.speakers.len(),
        payload.sessions.len()
    );
    let speakers = speakers::repository::SpeakerRepository::new(env.d1("DB")?)
        .sync_from_sessionize(&event_slug, payload.speakers, None, false)
        .await?;
    let schedule = schedule::repository::ScheduleRepository::new(db)
        .sync_from_sessionize(&event_slug, payload.sessions, None, false)
        .await?;
    console_log!(
        "Sessionize insert-only import completed: event={} speakers={} sessions={}",
        event_slug,
        speakers.imported,
        schedule.imported
    );
    Ok(())
}

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
                    } else if msg.starts_with("409 (") {
                        (409, msg.as_str())
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
