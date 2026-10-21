use aide::{
  OperationOutput,
  generate::GenContext,
  openapi::{Operation, Response},
};
use axum::{Json, response::IntoResponse};
use centaurus::error::ErrorReport;
use http::StatusCode;
use schemars::JsonSchema;
use sea_orm::DbErr;
use serde::Serialize;

#[derive(Serialize, JsonSchema, Debug, PartialEq, Eq)]
pub struct FieldError {
  /// Dotted path of the offending request field, e.g. `allocations.1.port`
  /// or `variables.SERVER_JARFILE`.
  pub field: String,
  pub message: String,
}

/// Body of a 400 (validation) or 409 (allocation in use) response.
#[derive(Serialize, JsonSchema)]
pub struct FieldErrors {
  pub errors: Vec<FieldError>,
}

pub(super) fn field_err(field: impl Into<String>, message: impl Into<String>) -> FieldError {
  FieldError {
    field: field.into(),
    message: message.into(),
  }
}

/// A plain centaurus error (status only) or field errors with a JSON body.
#[derive(Debug)]
pub enum ServerError {
  Report(ErrorReport),
  Fields(StatusCode, Vec<FieldError>),
}

impl From<ErrorReport> for ServerError {
  fn from(e: ErrorReport) -> Self {
    Self::Report(e)
  }
}

impl From<DbErr> for ServerError {
  fn from(e: DbErr) -> Self {
    Self::Report(e.into())
  }
}

impl IntoResponse for ServerError {
  fn into_response(self) -> axum::response::Response {
    match self {
      Self::Report(e) => e.into_response(),
      Self::Fields(status, errors) => (status, Json(FieldErrors { errors })).into_response(),
    }
  }
}

impl OperationOutput for ServerError {
  type Inner = FieldErrors;

  fn inferred_responses(
    ctx: &mut GenContext,
    operation: &mut Operation,
  ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
    let mut res = Vec::new();
    if let Some(body) = Json::<FieldErrors>::operation_response(ctx, operation) {
      for code in [400, 409] {
        res.push((Some(aide::openapi::StatusCode::Code(code)), body.clone()));
      }
    }
    res.extend(ErrorReport::inferred_responses(ctx, operation));
    res
  }
}

pub(super) type FieldResult<T> = std::result::Result<T, ServerError>;

pub(super) fn fail_if(status: StatusCode, errors: Vec<FieldError>) -> FieldResult<()> {
  if errors.is_empty() {
    Ok(())
  } else {
    Err(ServerError::Fields(status, errors))
  }
}
#[cfg(test)]
mod test {
  use serde_json::json;

  use super::*;

  #[test]
  fn field_errors_serialize_with_status() {
    let res = ServerError::Fields(
      StatusCode::CONFLICT,
      vec![field_err("allocations.0", "taken")],
    )
    .into_response();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let res = ServerError::from(centaurus::anyhow!(FORBIDDEN, "no")).into_response();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = ServerError::from(DbErr::Custom("x".into())).into_response();
    assert!(res.status().is_server_error());
    assert!(fail_if(StatusCode::BAD_REQUEST, vec![]).is_ok());
    assert!(matches!(
      fail_if(StatusCode::BAD_REQUEST, vec![field_err("a", "b")]),
      Err(ServerError::Fields(StatusCode::BAD_REQUEST, _))
    ));
    assert_eq!(
      serde_json::to_value(FieldErrors {
        errors: vec![field_err("name", "Name is required.")]
      })
      .unwrap(),
      json!({ "errors": [{ "field": "name", "message": "Name is required." }] })
    );
  }
}
