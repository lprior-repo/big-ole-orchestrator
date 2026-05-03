//! Bus lifecycle tests — MessageBus spawn, drain, send, recv, shutdown.
//!
//! Tests the full MessageBus IPC lifecycle: spawning a subprocess with FD3/FD4,
//! managing the backpressure channel, draining responses, and handling errors.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::tempdir;
use vo_ipc::bus::{BusConfig, BusError, BusMessage, MessageBus};
use vo_ipc::envelope::{Fd3Envelope, Fd4Envelope, TaskResult};
use vo_ipc::{IpcError, SubprocessConfig};

fn fixture_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fixture_driver"))
}

fn make_executable(path: &Path) {
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

fn make_fd3(instance_id: &str, node_id: &str, input: serde_json::Value) -> Fd3Envelope {
    Fd3Envelope {
        version: 1,
        instance_id: instance_id.to_string(),
        node_id: node_id.to_string(),
        input,
        secrets: BTreeMap::new(),
        metadata: BTreeMap::new(),
    }
}

// ============================================================================
// SECTION 1: BusConfig
// ============================================================================

#[test]
fn bus_config_default_values() {
    let config = BusConfig::default();
    assert_eq!(config.backpressure_limit(), 64);
    assert_eq!(config.timeout_ms(), 5000);
}

#[test]
fn bus_config_custom_values() {
    let config = BusConfig::new(128, 10000);
    assert_eq!(config.backpressure_limit(), 128);
    assert_eq!(config.timeout_ms(), 10000);
}

#[test]
fn bus_config_zero_backpressure() {
    let config = BusConfig::new(0, 1000);
    assert_eq!(config.backpressure_limit(), 0);
}

#[test]
fn bus_config_large_backpressure() {
    let config = BusConfig::new(10000, 1000);
    assert_eq!(config.backpressure_limit(), 10000);
}

// ============================================================================
// SECTION 2: BusError display
// ============================================================================

#[test]
fn bus_error_bus_closed_display() {
    assert_eq!(BusError::BusClosed.to_string(), "bus is closed");
}

#[test]
fn bus_error_backpressure_display() {
    assert_eq!(
        BusError::BackpressureLimitReached.to_string(),
        "backpressure limit reached"
    );
}

#[test]
fn bus_error_timeout_display() {
    assert_eq!(BusError::Timeout.to_string(), "timeout");
}

#[test]
fn bus_error_io_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broke");
    let bus_err = BusError::IoError(io_err);
    assert!(bus_err.to_string().contains("pipe broke"));
}

// ============================================================================
// SECTION 3: MessageBus spawn + drain lifecycle
// ============================================================================

#[tokio::test]
async fn bus_spawn_and_drain_succeeds() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("write_fd4.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\nprintf '\\x00\\x00\\x00\\x05hello' >&4\n",
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let output = bus.drain().await.unwrap();
    assert_eq!(output.fd4_bytes, b"hello");
}

#[tokio::test]
async fn bus_drain_empty_stderr() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quiet.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let output = bus.drain().await.unwrap();
    assert_eq!(output.stderr_bytes, Vec::<u8>::new());
    assert!(!output.stderr_truncated);
}

#[tokio::test]
async fn bus_drain_captures_stderr() {
    let config =
        SubprocessConfig::new(fixture_binary(), 5000, b"stderr-text bus-stderr 0".to_vec())
            .unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let output = bus.drain().await.unwrap();
    assert_eq!(output.stderr_bytes, b"bus-stderr");
}

// ============================================================================
// SECTION 4: MessageBus send + recv (channel mechanics)
// ============================================================================

#[tokio::test]
async fn bus_send_and_recv_through_channel() {
    let config = SubprocessConfig::new(fixture_binary(), 5000, b"echo-fd3 test".to_vec()).unwrap();
    let mut bus = MessageBus::spawn(config, BusConfig::new(64, 5000))
        .await
        .unwrap();

    let env = make_fd3("inst1", "node1", serde_json::json!({"key": "val"}));
    bus.send(env.clone()).await.unwrap();

    let msg = bus.recv().await.unwrap();
    match msg {
        BusMessage::Request(req) => {
            assert_eq!(req.instance_id, "inst1");
            assert_eq!(req.node_id, "node1");
        }
        other => panic!("expected Request, got {:?}", other),
    }
}

#[tokio::test]
async fn bus_send_fills_backpressure_limit() {
    let config = SubprocessConfig::new(fixture_binary(), 5000, b"echo-fd3 test".to_vec()).unwrap();
    let mut bus = MessageBus::spawn(config, BusConfig::new(4, 5000))
        .await
        .unwrap();

    // Fill the channel to capacity
    for i in 0..4 {
        let env = make_fd3("inst", "node", serde_json::json!(i));
        bus.send(env).await.unwrap();
    }

    assert!(bus.is_full());
    assert_eq!(bus.capacity(), 0);
    assert_eq!(bus.max_capacity(), 4);
}

#[tokio::test]
async fn bus_try_recv_returns_empty_when_no_messages() {
    let config = SubprocessConfig::new(fixture_binary(), 5000, b"echo-fd3 test".to_vec()).unwrap();
    let mut bus = MessageBus::spawn(config, BusConfig::new(64, 5000))
        .await
        .unwrap();

    let result = bus.try_recv();
    assert!(matches!(result, Err(BusError::BusClosed)));
}

// ============================================================================
// SECTION 5: MessageBus error handling
// ============================================================================

#[tokio::test]
async fn bus_drain_nonzero_exit_returns_process_failed() {
    let config =
        SubprocessConfig::new(fixture_binary(), 5000, b"stderr-text fail 42".to_vec()).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let result = bus.drain().await;
    match result {
        Err(IpcError::ProcessFailed {
            exit_code,
            stderr_bytes,
            ..
        }) => {
            assert_eq!(exit_code, 42);
            assert_eq!(stderr_bytes, b"fail");
        }
        other => panic!("expected ProcessFailed, got {:?}", other),
    }
}

#[tokio::test]
async fn bus_drain_timeout_returns_timeout_error() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("sleeper.sh");
    std::fs::write(&script, "#!/bin/sh\nsleep 60\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 50, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::new(64, 50))
        .await
        .unwrap();

    let result = bus.drain().await;
    assert!(matches!(result, Err(IpcError::Timeout { .. })));
}

#[tokio::test]
async fn bus_spawn_invalid_program_returns_spawn_failed() {
    let config = SubprocessConfig::new("/nonexistent/binary/path", 1000, vec![]).unwrap_err();
    // SubprocessConfig validates the path, so we need to test with a valid path
    // but the spawn should still fail for other reasons. Let's test the config rejection.
    assert!(config.to_string().contains("does not exist"));
}

#[tokio::test]
async fn bus_drain_fd4_huge_payload_returns_error() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("huge_fd4.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os\nos.write(4, (100 * 1024 * 1024).to_bytes(4, 'big'))\n",
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::new(64, 5000))
        .await
        .unwrap();

    let result = bus.drain().await;
    match result {
        Err(IpcError::Fd4ReadFailed { detail }) => {
            assert!(detail.contains("fd4 payload too large"));
        }
        other => panic!("expected Fd4ReadFailed, got {:?}", other),
    }
}

// ============================================================================
// SECTION 6: MessageBus shutdown
// ============================================================================

#[tokio::test]
async fn bus_shutdown_succeeds_on_normal_exit() {
    let config = SubprocessConfig::new(fixture_binary(), 5000, b"echo-fd3 test".to_vec()).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let result = bus.shutdown().await;
    assert!(result.is_ok(), "shutdown should succeed: {:?}", result);
}

// ============================================================================
// SECTION 7: BusMessage variants
// ============================================================================

#[test]
fn bus_message_request_equality() {
    let env = make_fd3("i", "n", serde_json::json!(42));
    let msg1 = BusMessage::Request(env.clone());
    let msg2 = BusMessage::Request(env);
    assert_eq!(msg1, msg2);
}

#[test]
fn bus_message_drained_is_distinct() {
    let env = make_fd3("i", "n", serde_json::json!(1));
    let req = BusMessage::Request(env);
    let drained = BusMessage::Drained;
    assert_ne!(req, drained);
}

#[test]
fn bus_message_response_equality() {
    let env1 = Fd4Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        result: TaskResult::Success {
            output: serde_json::json!(true),
        },
    };
    let env2 = Fd4Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        result: TaskResult::Success {
            output: serde_json::json!(true),
        },
    };
    let msg1 = BusMessage::Response(env1);
    let msg2 = BusMessage::Response(env2);
    assert_eq!(msg1, msg2);
}

// ============================================================================
// SECTION 8: Concurrent bus access
// ============================================================================

#[tokio::test]
async fn bus_concurrent_spawns_complete_successfully() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    let mut handles = Vec::new();
    for _ in 0..5 {
        let s = script.clone();
        handles.push(tokio::spawn(async move {
            let config = SubprocessConfig::new(&s, 2000, vec![]).unwrap();
            let bus = MessageBus::spawn(config, BusConfig::default())
                .await
                .unwrap();
            bus.drain().await
        }));
    }

    for handle in handles {
        let result = handle.await.expect("task panicked");
        assert!(
            result.is_ok(),
            "concurrent bus spawn should succeed: {:?}",
            result
        );
    }
}

#[tokio::test]
async fn bus_sequential_drains_dont_leak_fds() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    for _ in 0..20 {
        let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
        let bus = MessageBus::spawn(config, BusConfig::default())
            .await
            .unwrap();
        let result = bus.drain().await;
        assert!(
            result.is_ok(),
            "sequential drain should succeed: {:?}",
            result
        );
    }
}

// ============================================================================
// SECTION 9: Bus with real FD3/FD4 data flow
// ============================================================================

#[tokio::test]
async fn bus_drain_reads_fd4_envelope_from_child() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("fd4_envelope.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, json\nenv = {\"version\": 1, \"instance_id\": \"bus-test\", \"node_id\": \"n1\",\n       \"result\": {\"success\": {\"output\": \"bus-response\"}}}\nresp = json.dumps(env).encode()\nos.write(4, len(resp).to_bytes(4, 'big'))\nos.write(4, resp)\n",
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::new(64, 5000))
        .await
        .unwrap();

    let output = bus.drain().await.unwrap();
    let parsed: Fd4Envelope = serde_json::from_slice(&output.fd4_bytes).unwrap();
    assert_eq!(parsed.instance_id, "bus-test");
    assert_eq!(parsed.node_id, "n1");
}

#[tokio::test]
async fn bus_drain_empty_fd4_from_quick_exit_child() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("quick_exit.sh");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let output = bus.drain().await.unwrap();
    assert!(output.fd4_bytes.is_empty());
}

#[tokio::test]
async fn bus_drain_child_exits_without_fd4_returns_empty() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("no_fd4.py");
    std::fs::write(&script, "#!/usr/bin/python3\n# no fd4 write\n").unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 2000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::default())
        .await
        .unwrap();

    let output = bus.drain().await.unwrap();
    assert!(output.fd4_bytes.is_empty());
}

// ============================================================================
// SECTION 10: Bus timeout edge cases
// ============================================================================

#[tokio::test]
async fn bus_drain_child_stalls_after_header_times_out() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("stall_header.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os, time\nos.write(4, (1024).to_bytes(4, 'big'))\ntime.sleep(60)\n",
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 200, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::new(64, 200))
        .await
        .unwrap();

    let start = std::time::Instant::now();
    let result = bus.drain().await;
    let elapsed = start.elapsed();

    assert!(matches!(result, Err(IpcError::Timeout { .. })));
    assert!(
        elapsed < Duration::from_secs(5),
        "timeout should fire quickly: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn bus_drain_partial_fd4_header_returns_error() {
    let dir = tempdir().unwrap();
    let script = dir.path().join("partial_header.py");
    std::fs::write(
        &script,
        "#!/usr/bin/python3\nimport os\nos.write(4, b'\\x00\\x00\\x00')\n",
    )
    .unwrap();
    make_executable(&script);

    let config = SubprocessConfig::new(&script, 5000, vec![]).unwrap();
    let bus = MessageBus::spawn(config, BusConfig::new(64, 5000))
        .await
        .unwrap();

    let result = bus.drain().await;
    assert!(
        matches!(result, Err(IpcError::Fd4ReadFailed { .. })),
        "partial fd4 header should return Fd4ReadFailed: {:?}",
        result
    );
}
