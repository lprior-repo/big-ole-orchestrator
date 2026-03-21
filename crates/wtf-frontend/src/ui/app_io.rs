#[cfg(target_arch = "wasm32")]
use oya_frontend::graph::Workflow;

#[cfg(target_arch = "wasm32")]
use chrono;

#[cfg(target_arch = "wasm32")]
pub fn canvas_rect_size() -> Option<(f32, f32)> {
    use web_sys::window;

    let document = window().and_then(|win| win.document())?;
    let element = document.query_selector("main").ok().flatten()?;
    let rect = element.get_bounding_client_rect();
    #[allow(clippy::cast_possible_truncation)]
    let width = rect.width() as f32;
    #[allow(clippy::cast_possible_truncation)]
    let height = rect.height() as f32;
    Some((width, height))
}

#[cfg(target_arch = "wasm32")]
pub fn canvas_origin() -> Option<(f32, f32)> {
    use web_sys::window;

    let document = window().and_then(|win| win.document())?;
    let element = document.query_selector("main").ok().flatten()?;
    let rect = element.get_bounding_client_rect();
    #[allow(clippy::cast_possible_truncation)]
    let left = rect.left() as f32;
    #[allow(clippy::cast_possible_truncation)]
    let top = rect.top() as f32;
    Some((left, top))
}

#[cfg(not(target_arch = "wasm32"))]
pub const fn canvas_rect_size() -> Option<(f32, f32)> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub const fn canvas_origin() -> Option<(f32, f32)> {
    None
}

#[cfg(target_arch = "wasm32")]
pub fn download_workflow_json(name: &str, workflow: &Workflow) {
    use js_sys::Array;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{window, Blob, HtmlAnchorElement, Url};

    let json = match serde_json::to_string_pretty(workflow) {
        Ok(value) => value,
        Err(_) => return,
    };

    let chunks = Array::new();
    chunks.push(&JsValue::from_str(&json));

    let blob = match Blob::new_with_str_sequence(&chunks) {
        Ok(value) => value,
        Err(_) => return,
    };

    let url = match Url::create_object_url_with_blob(&blob) {
        Ok(value) => value,
        Err(_) => return,
    };

    let document = match window().and_then(|win| win.document()) {
        Some(value) => value,
        None => return,
    };

    let element = match document.create_element("a") {
        Ok(value) => value,
        Err(_) => {
            let _ = Url::revoke_object_url(&url);
            return;
        }
    };

    let anchor = match element.dyn_into::<HtmlAnchorElement>() {
        Ok(value) => value,
        Err(_) => {
            let _ = Url::revoke_object_url(&url);
            return;
        }
    };

    let filename = format!(
        "{}.json",
        name.trim()
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>()
    );

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();
    let _ = Url::revoke_object_url(&url);
}

#[cfg(target_arch = "wasm32")]
pub fn export_restate_history<T: serde::Serialize>(invocations: &[T]) {
    use js_sys::Array;
    use wasm_bindgen::{JsCast, JsValue};
    use web_sys::{window, Blob, HtmlAnchorElement, Url};

    let json = match serde_json::to_string_pretty(invocations) {
        Ok(value) => value,
        Err(_) => return,
    };

    let chunks = Array::new();
    chunks.push(&JsValue::from_str(&json));

    let blob = match Blob::new_with_str_sequence(&chunks) {
        Ok(value) => value,
        Err(_) => return,
    };

    let url = match Url::create_object_url_with_blob(&blob) {
        Ok(value) => value,
        Err(_) => return,
    };

    let document = match window().and_then(|win| win.document()) {
        Some(value) => value,
        None => return,
    };

    let element = match document.create_element("a") {
        Ok(value) => value,
        Err(_) => {
            let _ = Url::revoke_object_url(&url);
            return;
        }
    };

    let anchor = match element.dyn_into::<HtmlAnchorElement>() {
        Ok(value) => value,
        Err(_) => {
            let _ = Url::revoke_object_url(&url);
            return;
        }
    };

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let filename = format!("restate-history-{}.json", timestamp);

    anchor.set_href(&url);
    anchor.set_download(&filename);
    anchor.click();
    let _ = Url::revoke_object_url(&url);
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{canvas_origin, canvas_rect_size};

    #[test]
    fn given_non_wasm_target_when_reading_canvas_rect_size_then_none_is_returned() {
        assert_eq!(canvas_rect_size(), None);
    }

    #[test]
    fn given_non_wasm_target_when_reading_canvas_origin_then_none_is_returned() {
        assert_eq!(canvas_origin(), None);
    }

    mod export_restate_history {
        use super::super::*;

        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct MockInvocation {
            id: String,
            service: String,
            status: String,
        }

        #[test]
        fn given_empty_invocations_when_serializing_then_valid_json_array_is_produced() {
            let invocations: Vec<MockInvocation> = vec![];
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            assert_eq!(json, "[]");
        }

        #[test]
        fn given_single_invocation_when_serializing_then_valid_json_is_produced() {
            let invocations = vec![MockInvocation {
                id: "inv-123".to_string(),
                service: "UserService".to_string(),
                status: "completed".to_string(),
            }];
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            assert!(json.contains("inv-123"));
            assert!(json.contains("UserService"));
            assert!(json.contains("completed"));
        }

        #[test]
        fn given_multiple_invocations_when_serializing_then_all_are_preserved() {
            let invocations = vec![
                MockInvocation {
                    id: "inv-1".to_string(),
                    service: "ServiceA".to_string(),
                    status: "running".to_string(),
                },
                MockInvocation {
                    id: "inv-2".to_string(),
                    service: "ServiceB".to_string(),
                    status: "completed".to_string(),
                },
                MockInvocation {
                    id: "inv-3".to_string(),
                    service: "ServiceC".to_string(),
                    status: "failed".to_string(),
                },
            ];
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            assert!(json.contains("inv-1"));
            assert!(json.contains("inv-2"));
            assert!(json.contains("inv-3"));
        }

        #[test]
        fn given_invocation_with_nested_fields_when_serializing_then_nested_is_preserved() {
            #[derive(serde::Serialize, serde::Deserialize, Debug)]
            struct NestedInput {
                name: String,
                value: i32,
            }

            #[derive(serde::Serialize, serde::Deserialize, Debug)]
            struct InvocationWithNested {
                id: String,
                input: NestedInput,
            }

            let invocations = vec![InvocationWithNested {
                id: "inv-nested".to_string(),
                input: NestedInput {
                    name: "test".to_string(),
                    value: 42,
                },
            }];
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            assert!(json.contains("inv-nested"));
            assert!(json.contains("test"));
            assert!(json.contains("42"));
        }

        #[test]
        fn given_invocation_with_special_characters_when_serializing_then_escaped_properly() {
            let invocations = vec![MockInvocation {
                id: "inv-with-\"quotes\"".to_string(),
                service: "service/with/slashes".to_string(),
                status: "status\nwith\nnewlines".to_string(),
            }];
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            assert!(json.contains("\\\"quotes\\\""));
            assert!(json.contains("newlines"));
        }

        #[test]
        fn given_invocation_with_unicode_when_serializing_then_unicode_preserved() {
            let invocations = vec![MockInvocation {
                id: "inv-日本語".to_string(),
                service: "服务".to_string(),
                status: "émoji 🎉".to_string(),
            }];
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            assert!(json.contains("日本語"));
            assert!(json.contains("服务"));
            assert!(json.contains("🎉"));
        }

        #[test]
        fn given_large_invocation_count_when_serializing_then_performance_is_acceptable() {
            let invocations: Vec<MockInvocation> = (0..1000)
                .map(|i| MockInvocation {
                    id: format!("inv-{}", i),
                    service: format!("Service{}", i % 10),
                    status: "running".to_string(),
                })
                .collect();

            let start = std::time::Instant::now();
            let json = serde_json::to_string_pretty(&invocations).unwrap();
            let duration = start.elapsed();

            assert!(json.len() > 10000);
            assert!(
                duration.as_millis() < 100,
                "Serialization took too long: {:?}",
                duration
            );
        }
    }
}
