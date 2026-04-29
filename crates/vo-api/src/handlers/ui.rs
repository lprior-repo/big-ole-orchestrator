//! WASM UI asset serving for the Dioxus frontend.
//!
//! Serves the compiled Dioxus WASM binary and its HTML shell from
//! `crates/vo-api/assets/`. Assets are embedded at compile time via
//! `rust-embed`, preserving the single-binary constraint from ADR-007.
//!
//! To build the frontend:
//! ```sh
//! cd crates/vo-frontend && dx build --release
//! cp dist/* ../vo-api/assets/
//! ```
//!
//! Routes:
//! - `GET /wtf/ui` — HTML shell that boots the Dioxus WASM module
//! - `GET /wtf/ui/*path` — Static assets (.wasm, .js, .css)

use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "assets/"]
struct UiAssets;

pub async fn wtf_ui() -> impl IntoResponse {
    match UiAssets::get("index.html") {
        Some(content) => {
            let body = content.data;
            Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(body))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(
                "UI assets not built. Run: cd crates/vo-frontend && dx build",
            ))
            .unwrap(),
    }
}

pub async fn wtf_ui_asset(Path(path): Path<String>) -> impl IntoResponse {
    match UiAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("not found"))
            .unwrap(),
    }
}
