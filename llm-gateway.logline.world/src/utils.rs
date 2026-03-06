//! Utility functions for llm-gateway

use crate::{
    request_context::current_request_id,
    types::{ErrorDetail, ErrorResponse},
};
use axum::{http::StatusCode, response::Json};
use uuid::Uuid;

pub fn error_response(
    code: StatusCode,
    message: &str,
    error_type: &str,
) -> (StatusCode, Json<ErrorResponse>) {
    error_response_with_request_id(code, message, error_type, None)
}

pub fn error_response_with_request_id(
    code: StatusCode,
    message: &str,
    error_type: &str,
    request_id: Option<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    let request_id = request_id
        .or_else(current_request_id)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    (
        code,
        Json(ErrorResponse {
            request_id,
            output_schema: "https://logline.world/schemas/error-envelope.v1.schema.json",
            error: ErrorDetail {
                message: message.to_string(),
                error_type: error_type.to_string(),
                code: error_type.to_string(),
            },
        }),
    )
}
