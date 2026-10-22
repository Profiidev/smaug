//! `ErrorBody` responses (`docs/API.md` §2.2) for the live routes.

use axum::{
  Json,
  response::{IntoResponse, Response},
};
use centaurus::error::ErrorReport;
use http::StatusCode;
use shared::msg::{ErrorBody, ErrorCode};

#[derive(Debug)]
pub struct ApiError {
  pub body: ErrorBody,
  pub status: StatusCode,
}

pub type ApiResult<T> = Result<T, ApiError>;

impl ApiError {
  pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      status: status(code),
      body: ErrorBody {
        code,
        message: message.into(),
      },
    }
  }

  /// `410 gone`, which has no `ErrorCode` of its own.
  pub fn gone(message: impl Into<String>) -> Self {
    Self {
      status: StatusCode::GONE,
      ..Self::new(ErrorCode::NotFound, message)
    }
  }

  pub fn internal(err: impl std::fmt::Display) -> Self {
    Self::new(ErrorCode::Internal, err.to_string())
  }
}

fn status(code: ErrorCode) -> StatusCode {
  match code {
    ErrorCode::BadRequest => StatusCode::BAD_REQUEST,
    ErrorCode::Denied => StatusCode::FORBIDDEN,
    ErrorCode::NotFound => StatusCode::NOT_FOUND,
    ErrorCode::Conflict | ErrorCode::Busy => StatusCode::CONFLICT,
    ErrorCode::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
    ErrorCode::Unsupported => StatusCode::NOT_IMPLEMENTED,
    ErrorCode::Quota => StatusCode::INSUFFICIENT_STORAGE,
    ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
  }
}

impl std::fmt::Display for ApiError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.body.message)
  }
}

impl IntoResponse for ApiError {
  fn into_response(self) -> Response {
    if self.body.code == ErrorCode::Internal {
      tracing::error!("{}", self.body.message);
    }
    (self.status, Json(self.body)).into_response()
  }
}

impl From<ErrorReport> for ApiError {
  fn from(err: ErrorReport) -> Self {
    let code = match err.status {
      StatusCode::BAD_REQUEST => ErrorCode::BadRequest,
      StatusCode::FORBIDDEN => ErrorCode::Denied,
      StatusCode::NOT_FOUND => ErrorCode::NotFound,
      StatusCode::CONFLICT => ErrorCode::Conflict,
      StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::TooLarge,
      _ => ErrorCode::Internal,
    };
    Self::new(code, format!("{:#}", err.error))
  }
}

impl From<reqwest::Error> for ApiError {
  fn from(err: reqwest::Error) -> Self {
    Self::internal(format!("Failed to reach agent: {err}"))
  }
}
