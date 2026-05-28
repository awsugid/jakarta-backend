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
    http::routes::register_routes(router).run(req, env).await
}
