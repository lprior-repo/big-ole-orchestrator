use axum::{
    extract::{Extension, Request},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::types::{ApiError, V3StartRequest};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct WebhookState {
    pub secret: String,
}

impl WebhookState {
    #[must_use]
    pub fn new(secret: String) -> Self {
        Self { secret }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WebhookError {
    #[error("missing signature header")]
    MissingSignature,
    #[error("invalid signature format")]
    InvalidSignatureFormat,
    #[error("signature verification failed")]
    SignatureVerificationFailed,
    #[error("invalid request body")]
    InvalidBody,
    #[error("body read error")]
    BodyReadError,
}

impl IntoResponse for WebhookError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_code, message) = match &self {
            WebhookError::MissingSignature => (
                StatusCode::UNAUTHORIZED,
                "missing_signature",
                self.to_string(),
            ),
            WebhookError::InvalidSignatureFormat => (
                StatusCode::UNAUTHORIZED,
                "invalid_signature_format",
                self.to_string(),
            ),
            WebhookError::SignatureVerificationFailed => (
                StatusCode::UNAUTHORIZED,
                "signature_verification_failed",
                self.to_string(),
            ),
            WebhookError::InvalidBody => (
                StatusCode::BAD_REQUEST,
                "invalid_body",
                self.to_string(),
            ),
            WebhookError::BodyReadError => (
                StatusCode::BAD_REQUEST,
                "body_read_error",
                self.to_string(),
            ),
        };
        (status, Json(ApiError::new(error_code, message))).into_response()
    }
}

impl From<serde_json::Error> for WebhookError {
    fn from(_: serde_json::Error) -> Self {
        WebhookError::InvalidBody
    }
}

pub fn verify_hmac_signature(
    secret: &str,
    body: &[u8],
    signature_header: &str,
) -> Result<(), WebhookError> {
    let expected_signature = signature_header.trim_start_matches("sha256=");

    let expected_bytes =
        hex::decode(expected_signature).map_err(|_| WebhookError::InvalidSignatureFormat)?;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).map_err(|_| {
            WebhookError::SignatureVerificationFailed
        })?;
    mac.update(body);

    mac.verify_slice(&expected_bytes)
        .map_err(|_| WebhookError::SignatureVerificationFailed)?;

    Ok(())
}

pub async fn webhook_handler(
    Extension(state): Extension<WebhookState>,
    headers: HeaderMap,
    request: Request,
) -> Result<impl IntoResponse, WebhookError> {
    let signature = headers
        .get("X-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or(WebhookError::MissingSignature)?;

    let body_bytes = axum::body::to_bytes(request.into_body(), 64 * 1024)
        .await
        .map_err(|_| WebhookError::BodyReadError)?;

    verify_hmac_signature(&state.secret, &body_bytes, signature)?;

    let _request: V3StartRequest = serde_json::from_slice(&body_bytes)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"status": "received"})),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "whs-test-secret-123";

    fn compute_signature(secret: &str, body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        format!("sha256={}", hex::encode(result.into_bytes()))
    }

    #[test]
    fn test_valid_signature() {
        let body = br#"{"namespace":"test","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let signature = compute_signature(TEST_SECRET, body);

        let result = verify_hmac_signature(TEST_SECRET, body, &signature);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_signature_wrong_secret() {
        let body = br#"{"namespace":"test","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let signature = compute_signature("wrong-secret", body);

        let result = verify_hmac_signature(TEST_SECRET, body, &signature);
        assert!(matches!(
            result,
            Err(WebhookError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn test_invalid_signature_tampered_body() {
        let body = br#"{"namespace":"test","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let signature = compute_signature(TEST_SECRET, body);

        let tampered_body =
            br#"{"namespace":"hacked","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let result = verify_hmac_signature(TEST_SECRET, tampered_body, &signature);
        assert!(matches!(
            result,
            Err(WebhookError::SignatureVerificationFailed)
        ));
    }

    #[test]
    fn test_invalid_signature_format_not_hex() {
        let body = br#"{"namespace":"test","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let signature = "sha256=not-valid-hex!!!";

        let result = verify_hmac_signature(TEST_SECRET, body, signature);
        assert!(matches!(result, Err(WebhookError::InvalidSignatureFormat)));
    }

    #[test]
    fn test_invalid_signature_missing_prefix() {
        let body = br#"{"namespace":"test","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let mut mac = HmacSha256::new_from_slice(TEST_SECRET.as_bytes()).unwrap();
        mac.update(body);
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());

        let result = verify_hmac_signature(TEST_SECRET, body, &signature);
        assert!(matches!(result, Err(WebhookError::InvalidSignatureFormat)));
    }

    #[test]
    fn test_missing_signature_header_value() {
        let body = br#"{"namespace":"test","workflow_type":"test","paradigm":"fsm","input":{}}"#;
        let signature = "";

        let result = verify_hmac_signature(TEST_SECRET, body, signature);
        assert!(matches!(result, Err(WebhookError::InvalidSignatureFormat)));
    }
}