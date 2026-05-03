use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use axum::{
    body::Body,
    extract::State,
    http::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use subtle::ConstantTimeEq;

fn hash_api_key(plaintext: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(plaintext.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .unwrap_or_else(|_| {
            let salt = SaltString::from_b64("migrationsalt1234567890123456789012345678")
                .expect("static salt valid");
            argon2
                .hash_password(plaintext.as_bytes(), &salt)
                .map(|hash| hash.to_string())
                .expect("password hashing with fallback salt should not fail")
        })
}

fn constant_time_compare(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}

#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub api_keys: Arc<Vec<String>>,
    pub jwt_secret: Option<Arc<String>>,
    pub jwt_issuer: Option<String>,
    pub enabled: bool,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let api_keys = std::env::var("VO_API_KEYS")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .map(|s| hash_api_key(&s))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let jwt_secret = std::env::var("VO_JWT_SECRET")
            .ok()
            .filter(|s| !s.is_empty())
            .map(Arc::new);

        let jwt_issuer = std::env::var("VO_JWT_ISSUER")
            .ok()
            .filter(|s| !s.is_empty());

        Self {
            api_keys: Arc::new(api_keys),
            jwt_secret,
            jwt_issuer,
            enabled: true,
        }
    }

    pub fn disabled() -> Self {
        Self {
            api_keys: Arc::new(Vec::new()),
            jwt_secret: None,
            jwt_issuer: None,
            enabled: false,
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_keys: Arc::new(Vec::new()),
            jwt_secret: None,
            jwt_issuer: None,
            enabled: true,
        }
    }
}

#[derive(Debug)]
pub enum AuthError {
    MissingCredentials,
    InvalidApiKey,
    InvalidToken(String),
    ExpiredToken,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AuthError::MissingCredentials => {
                (StatusCode::UNAUTHORIZED, "missing credentials".to_string())
            }
            AuthError::InvalidApiKey => (StatusCode::UNAUTHORIZED, "invalid api key".to_string()),
            AuthError::InvalidToken(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "token expired".to_string()),
        };
        let body = serde_json::json!({
            "error": message,
            "status": status.as_u16(),
        });
        (status, axum::Json(body)).into_response()
    }
}

pub async fn auth_middleware(
    State(config): State<AuthConfig>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    if !config.enabled {
        return Ok(next.run(request).await);
    }

    let api_key = request.headers().get("X-API-Key").cloned();
    let auth_header = request.headers().get("Authorization").cloned();

    if let Some(key) = api_key {
        let key_str = key.to_str().map_err(|_| AuthError::InvalidApiKey)?;
        let argon2 = Argon2::default();
        if config.api_keys.iter().any(|stored_hash| {
            if let Ok(parsed_hash) = PasswordHash::new(stored_hash) {
                argon2
                    .verify_password(key_str.as_bytes(), &parsed_hash)
                    .is_ok()
            } else {
                false
            }
        }) {
            return Ok(next.run(request).await);
        }
        return Err(AuthError::InvalidApiKey);
    }

    if let Some(auth) = auth_header {
        let auth_str = auth.to_str().map_err(|_| AuthError::MissingCredentials)?;
        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            return validate_jwt(token, &config, request, next).await;
        }
    }

    Err(AuthError::MissingCredentials)
}

async fn validate_jwt(
    token: &str,
    config: &AuthConfig,
    request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    let secret = config
        .jwt_secret
        .as_ref()
        .ok_or_else(|| AuthError::InvalidToken("jwt not configured".to_string()))?;

    let decoding_key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let mut validation = jsonwebtoken::Validation::default();

    if let Some(ref issuer) = config.jwt_issuer {
        validation.set_issuer(&[issuer.as_str()]);
    }

    match jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation) {
        Ok(_) => Ok(next.run(request).await),
        Err(e) => match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => Err(AuthError::ExpiredToken),
            _ => Err(AuthError::InvalidToken(e.to_string())),
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_config_default_has_auth_enabled() {
        let config = AuthConfig::default();
        assert!(config.enabled);
        assert!(config.api_keys.is_empty());
        assert!(config.jwt_secret.is_none());
    }

    #[test]
    fn auth_config_disabled_is_disabled() {
        let config = AuthConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn auth_config_from_env_parses_and_hashes_api_keys() {
        std::env::set_var("VO_API_KEYS", "key1, key2, key3");
        let config = AuthConfig::from_env();
        assert_eq!(config.api_keys.len(), 3);
        let argon2 = Argon2::default();
        for stored_hash in config.api_keys.iter() {
            let parsed_hash = PasswordHash::new(stored_hash).expect("valid argon2 hash");
            assert!(
                argon2.verify_password(b"key1", &parsed_hash).is_ok()
                    || argon2.verify_password(b"key2", &parsed_hash).is_ok()
                    || argon2.verify_password(b"key3", &parsed_hash).is_ok()
            );
        }
        assert!(config.enabled);
        std::env::remove_var("VO_API_KEYS");
    }

    #[test]
    fn hash_api_key_produces_valid_argon2_hash() {
        let hash = hash_api_key("test_key");
        let parsed_hash = PasswordHash::new(&hash).expect("valid argon2 hash");
        let argon2 = Argon2::default();
        assert!(argon2.verify_password(b"test_key", &parsed_hash).is_ok());
        assert!(argon2.verify_password(b"wrong_key", &parsed_hash).is_err());
    }

    #[test]
    fn claims_deserialize_minimal() {
        let json = r#"{"sub":"user-1","exp":9999999999,"iat":1000000000}"#;
        let claims: Claims = serde_json::from_str(json).unwrap();
        assert_eq!(claims.sub, "user-1");
        assert!(claims.iss.is_none());
    }

    #[test]
    fn auth_error_into_response_status_codes() {
        let resp = AuthError::MissingCredentials.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = AuthError::InvalidApiKey.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = AuthError::ExpiredToken.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = AuthError::InvalidToken("bad".to_string()).into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
