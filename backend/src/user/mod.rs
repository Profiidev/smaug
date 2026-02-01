use axum::Router;

use crate::rate_limit::RateLimiter;

mod account;
mod info;
mod management;

pub fn router(rate_limiter: &mut RateLimiter) -> Router {
  Router::new()
    .nest("/account", account::router(rate_limiter))
    .nest("/info", info::router())
    .nest("/management", management::router())
}
