use worker::*;

mod config;
mod http;
mod auth;
mod application;
mod formbricks;
mod storage;
mod validation;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> worker::Result<Response> {
    console_error_panic_hook::set_once();

    let router = Router::new();
    http::routes::register_routes(router)
        .run(req, env)
        .await
}
