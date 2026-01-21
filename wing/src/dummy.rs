use axum::{Extension, Router, extract::FromRequestParts, routing::get};
use centaurus::{error::Result, router_extension};

pub fn router() -> Router {
  Router::new().route("/test", get(test))
}

router_extension!(
  async fn dummy(self) -> Self {
    self.layer(Extension(TestState::default()))
  }
);

async fn test(test: TestState) -> Result<String> {
  Ok(test.test)
}

#[derive(Clone, FromRequestParts)]
#[from_request(via(Extension))]
struct TestState {
  test: String,
}

impl Default for TestState {
  fn default() -> Self {
    Self {
      test: "test".to_string(),
    }
  }
}
