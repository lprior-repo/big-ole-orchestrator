use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use ulid::Ulid;
use tracing::Level;

use crate::middleware::auth::is_public_path;

pub async fn request_logging(
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().to_string();

    if is_public_path(&path) {
        return next.run(request).await;
    }

    let request_id = Ulid::new().to_string();
    let start = std::time::Instant::now();

    request.extensions_mut().insert(RequestId(request_id.clone()));

    let mut response = next.run(request).await;
    let duration = start.elapsed();
    let status = response.status().as_u16();

    if let (Ok(name), Ok(value)) = (
        HeaderName::try_from("x-request-id"),
        HeaderValue::from_str(&request_id),
    ) {
        response.headers_mut().insert(name, value);
    }

    let log_level = if status >= 500 {
        Level::ERROR
    } else if status >= 400 {
        Level::WARN
    } else {
        Level::INFO
    };

    match log_level {
        Level::ERROR => {
            tracing::error!(
                method = %method,
                path = %path,
                status = %status,
                request_id = %request_id,
                duration_ms = duration.as_millis() as u64,
                "request completed with server error"
            );
        }
        Level::WARN => {
            tracing::warn!(
                method = %method,
                path = %path,
                status = %status,
                request_id = %request_id,
                duration_ms = duration.as_millis() as u64,
                "request completed with client error"
            );
        }
        _ => {
            tracing::info!(
                method = %method,
                path = %path,
                status = %status,
                request_id = %request_id,
                duration_ms = duration.as_millis() as u64,
                "request completed"
            );
        }
    }

    response
}

#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl RequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn request_id_from_extensions<B>(request: &Request<B>) -> Option<String> {
    request.extensions().get::<RequestId>().map(|id| id.0.clone())
}