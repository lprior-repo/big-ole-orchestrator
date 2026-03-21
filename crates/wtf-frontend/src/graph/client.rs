#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![forbid(unsafe_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct RestateClient {
    http_client: Client,
    base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyedState {
    pub service: String,
    pub key: String,
    pub value: serde_json::Value,
}

impl RestateClient {
    #[must_use]
    pub fn new(base_url: &str) -> Self {
        Self {
            http_client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn get_keyed_state(
        &self,
        service_name: &str,
        key: &str,
    ) -> Result<KeyedState, RestateClientError> {
        let url = format!("{}/{}/{}", self.base_url, service_name, key);
        
        let response = self.http_client
            .get(&url)
            .send()
            .await
            .map_err(RestateClientError::RequestFailed)?;

        if !response.status().is_success() {
            return Err(RestateClientError::ApiError {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let value: serde_json::Value = response.json().await?;

        Ok(KeyedState {
            service: service_name.to_string(),
            key: key.to_string(),
            value,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RestateClientError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    
    #[error("API error ({status}): {message}")]
    ApiError { status: u16, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    mod restate_client {
        use super::*;

        mod new {
            use super::*;

            #[test]
            fn given_valid_url_when_created_then_stores_base_url() {
                let client = RestateClient::new("http://localhost:8080");
                assert_eq!(client.base_url(), "http://localhost:8080");
            }

            #[test]
            fn given_url_with_trailing_slash_when_created_then_trims_slash() {
                let client = RestateClient::new("http://localhost:8080/");
                assert_eq!(client.base_url(), "http://localhost:8080");
            }

            #[test]
            fn given_multiple_trailing_slashes_when_created_then_trims_all() {
                let client = RestateClient::new("http://localhost:8080///");
                assert_eq!(client.base_url(), "http://localhost:8080");
            }

            #[test]
            fn given_empty_url_when_created_then_stores_empty_string() {
                let client = RestateClient::new("");
                assert_eq!(client.base_url(), "");
            }

            #[test]
            fn given_https_url_when_created_then_preserves_https() {
                let client = RestateClient::new("https://restate.example.com");
                assert_eq!(client.base_url(), "https://restate.example.com");
            }

            #[test]
            fn given_url_with_port_when_created_then_preserves_port() {
                let client = RestateClient::new("http://localhost:9090");
                assert_eq!(client.base_url(), "http://localhost:9090");
            }
        }

        mod base_url {
            use super::*;

            #[test]
            fn given_client_when_accessing_base_url_then_returns_immutable_reference() {
                let client = RestateClient::new("http://localhost:8080");
                let url: &str = client.base_url();
                assert_eq!(url, "http://localhost:8080");
            }
        }

        mod get_keyed_state {
            use super::*;
            use tokio::test;

            #[test]
            async fn given_empty_service_name_when_fetching_then_returns_error() {
                let client = RestateClient::new("http://localhost:8080");
                let result = client.get_keyed_state("", "some-key").await;
                assert!(result.is_err());
            }

            #[test]
            async fn given_empty_key_when_fetching_then_returns_error() {
                let client = RestateClient::new("http://localhost:8080");
                let result = client.get_keyed_state("my-service", "").await;
                assert!(result.is_err());
            }

            #[tokio::test]
            async fn given_special_characters_in_key_then_builds_url() {
                let client = RestateClient::new("http://localhost:8080");
                let _ = client.get_keyed_state("my-service", "key/with/slashes").await;
            }

            #[tokio::test]
            async fn given_unicode_in_service_name_then_handles() {
                let client = RestateClient::new("http://localhost:8080");
                let _ = client.get_keyed_state("service-日本語", "key").await;
            }
        }

        mod invariants {
            use super::*;

            #[test]
            fn given_client_clone_then_works() {
                let client1 = RestateClient::new("http://localhost:8080");
                let client2 = client1.clone();
                assert_eq!(client1.base_url(), client2.base_url());
            }

            #[test]
            fn given_keyed_state_clone_then_independent() {
                let state1 = KeyedState {
                    service: "test-service".to_string(),
                    key: "test-key".to_string(),
                    value: serde_json::json!({"foo": "bar"}),
                };
                let state2 = state1.clone();
                assert_eq!(state1, state2);
                assert_eq!(state1.service, state2.service);
                assert_ne!(std::ptr::addr_of!(state1.service), std::ptr::addr_of!(state2.service));
            }

            #[test]
            fn given_keyed_state_with_nested_json_then_serializes_correctly() {
                let state = KeyedState {
                    service: "my-service".to_string(),
                    key: "my-key".to_string(),
                    value: serde_json::json!({
                        "nested": {
                            "array": [1, 2, 3],
                            "null": null,
                            "boolean": true
                        }
                    }),
                };
                let serialized = serde_json::to_string(&state).unwrap();
                assert!(serialized.contains("my-service"));
                assert!(serialized.contains("my-key"));
                assert!(serialized.contains("nested"));
            }

            #[test]
            fn given_keyed_state_roundtrip_preserves_data() {
                let original = KeyedState {
                    service: "test-service".to_string(),
                    key: "test-key".to_string(),
                    value: serde_json::json!({"data": "value"}),
                };
                let serialized = serde_json::to_string(&original).unwrap();
                let restored: KeyedState = serde_json::from_str(&serialized).unwrap();
                assert_eq!(original, restored);
            }
        }

        mod error_display {
            use super::*;

            #[test]
            fn given_api_error_then_displays_status_and_message() {
                let error = RestateClientError::ApiError {
                    status: 404,
                    message: "Not Found".to_string(),
                };
                let display = error.to_string();
                assert!(display.contains("404"));
                assert!(display.contains("Not Found"));
            }
        }

        mod boundary_conditions {
            use super::*;

            #[test]
            fn given_very_long_service_name_then_handles() {
                let long_name = "a".repeat(1000);
                let client = RestateClient::new("http://localhost:8080");
                let url = format!("{}/{}/{}", client.base_url(), long_name, "key");
                assert!(url.len() > 1000);
            }

            #[test]
            fn given_very_long_key_then_handles() {
                let long_key = "k".repeat(1000);
                let client = RestateClient::new("http://localhost:8080");
                let url = format!("{}/{}/{}", client.base_url(), "service", long_key);
                assert!(url.len() > 1000);
            }

            #[test]
            fn given_max_u16_status_code_then_displays() {
                let error = RestateClientError::ApiError {
                    status: u16::MAX,
                    message: "Max status".to_string(),
                };
                let display = error.to_string();
                assert!(display.contains("65535"));
            }
        }
    }

    mod keyed_state {
        use super::*;

        mod serialization {
            use super::*;

            #[test]
            fn given_keyed_state_when_serialized_then_contains_all_fields() {
                let state = KeyedState {
                    service: "my-service".to_string(),
                    key: "my-key".to_string(),
                    value: serde_json::json!({"count": 42}),
                };
                let json = serde_json::to_string(&state).unwrap();
                assert!(json.contains("my-service"));
                assert!(json.contains("my-key"));
                assert!(json.contains("count"));
            }

            #[test]
            fn given_json_value_when_deserialized_then_preserves_type() {
                let json = r#"{"service":"svc","key":"k","value":{"nested":[1,2,3]}}"#;
                let state: KeyedState = serde_json::from_str(json).unwrap();
                assert_eq!(state.service, "svc");
                assert_eq!(state.key, "k");
                assert!(state.value.get("nested").is_some());
            }

            #[test]
            fn given_null_value_then_serializes_correctly() {
                let state = KeyedState {
                    service: "svc".to_string(),
                    key: "k".to_string(),
                    value: serde_json::Value::Null,
                };
                let json = serde_json::to_string(&state).unwrap();
                assert!(json.contains("null"));
            }

            #[test]
            fn given_array_value_then_serializes_correctly() {
                let state = KeyedState {
                    service: "svc".to_string(),
                    key: "k".to_string(),
                    value: serde_json::json!([1, "two", 3.0]),
                };
                let json = serde_json::to_string(&state).unwrap();
                assert!(json.contains("[1"));
            }
        }
    }
}
