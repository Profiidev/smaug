//! Coverage for the `GET /settings/general` site-url endpoint (`settings.rs`).

mod common;

use common::TestServer;
use reqwest::StatusCode;
use serde_json::Value;

#[tokio::test]
async fn general_settings_returns_the_configured_site_url() {
  let (server, _) = TestServer::start_with_admin().await;

  let resp = server.get("/settings/general").await;
  assert_eq!(resp.status(), StatusCode::OK);
  let body: Value = resp.json().await.unwrap();
  // common::prepare_env sets SITE_URL=http://localhost/
  assert_eq!(body["site_url"], "http://localhost/");
}

#[tokio::test]
async fn general_settings_requires_authentication() {
  let server = TestServer::start().await;

  let resp = server.get("/settings/general").await;
  assert!(!resp.status().is_success());
}
