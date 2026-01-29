use axum::Router;

mod account;
mod info;

pub fn router() -> Router {
  Router::new()
    .nest("/account", account::router())
    .nest("/info", info::router())
}
