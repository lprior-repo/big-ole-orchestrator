use std::sync::Arc;

use axum::{
    body::Body,
    extract::Extension,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

use vo_storage::api_key_partition::{ApiKeyStore, ApiKeyStoreError};

#[derive(Clone)]
pub struct ApiKeyState {
    pub api_key_store: Arc<dyn ApiKeyStore>,
}

pub async fn api_key_auth(
    Extension(state): Extension<ApiKeyState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let api_key = extract_api_key(&request);

    let api_key = match api_key {
        Some(key) => key,
        None => {
            return Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Body::from(
                    r#"{"error":"missing_api_key","message":"API key is required. Provide it via X-API-Key header or api_key query parameter."}"#,
                ))
                .unwrap()
        }
    };

    match state.api_key_store.validate_key(&api_key) {
        Ok(()) => next.run(request).await,
        Err(e) => {
            let (status, error_code, message) = match e {
                ApiKeyStoreError::NotFound => (
                    StatusCode::UNAUTHORIZED,
                    "invalid_api_key",
                    "The provided API key is invalid.",
                ),
                ApiKeyStoreError::Revoked => (
                    StatusCode::UNAUTHORIZED,
                    "revoked_api_key",
                    "The provided API key has been revoked.",
                ),
                ApiKeyStoreError::Expired => (
                    StatusCode::UNAUTHORIZED,
                    "expired_api_key",
                    "The provided API key has expired.",
                ),
            };

            let body = serde_json::json!({
                "error": error_code,
                "message": message
            });

            Response::builder()
                .status(status)
                .body(Body::from(body.to_string()))
                .unwrap()
        }
    }
}

fn extract_api_key(request: &Request<Body>) -> Option<String> {
    let uri = request.uri();

    if let Some(query) = uri.query() {
        for param in query.split('&') {
            if let Some(key) = param.strip_prefix("api_key=") {
                if !key.is_empty() {
                    return Some(decode_uri_component(key));
                }
            }
        }
    }

    let headers = request.headers();
    if let Some(key) = headers.get("X-API-Key") {
        if let Ok(key_str) = key.to_str() {
            if !key_str.is_empty() {
                return Some(key_str.to_string());
            }
        }
    }

    None
}

fn decode_uri_component(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if hex.len() == 2 {
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    result.push(byte as char);
                    continue;
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else {
            result.push(c);
        }
    }

    result
}

pub fn is_public_path(path: &str) -> bool {
    path == "/health" || path == "/openapi.json" || path.starts_with("/wtf/")
}
