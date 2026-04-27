use std::io::Cursor;
use vo_ipc::stderr::{finalize_capture, update_capture, StderrCapture, MAX_STDERR_BYTES};
use vo_ipc::*;

#[test]
fn stderr_empty_capture_is_default() {
    let cap = StderrCapture::empty();
    assert!(cap.bytes.is_empty());
    assert!(!cap.truncated);
    assert_eq!(cap.observed_bytes, 0);
}

#[test]
fn stderr_update_accumulates_observed_bytes() {
    let cap = StderrCapture::empty();
    let cap = update_capture(cap, b"hello");
    let cap = update_capture(cap, b" world");
    assert_eq!(cap.observed_bytes, 11);
    assert_eq!(cap.bytes, b"hello world");
}

#[test]
fn stderr_update_truncates_at_max() {
    let cap = StderrCapture::empty();
    let cap = update_capture(cap, &vec![b'x'; MAX_STDERR_BYTES]);
    assert_eq!(cap.bytes.len(), MAX_STDERR_BYTES);
    assert!(!cap.truncated);
    assert_eq!(cap.observed_bytes, MAX_STDERR_BYTES);

    let cap = update_capture(cap, b"overflow");
    assert_eq!(cap.bytes.len(), MAX_STDERR_BYTES);
    assert!(cap.truncated);
    assert_eq!(cap.observed_bytes, MAX_STDERR_BYTES + 8);
}

#[test]
fn stderr_finalize_does_not_marker_when_not_truncated() {
    let cap = StderrCapture {
        bytes: b"hello".to_vec(),
        truncated: false,
        observed_bytes: 5,
    };
    let result = finalize_capture(cap);
    assert_eq!(result.bytes, b"hello");
}

#[test]
fn stderr_finalize_adds_marker_when_truncated() {
    let cap = StderrCapture {
        bytes: vec![b'x'; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 100,
    };
    let result = finalize_capture(cap);
    assert!(result.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
}

#[test]
fn stderr_finalize_does_not_double_marker() {
    let cap = StderrCapture {
        bytes: vec![b'x'; MAX_STDERR_BYTES],
        truncated: true,
        observed_bytes: MAX_STDERR_BYTES + 1,
    };
    let first = finalize_capture(cap);
    let second = finalize_capture(first);
    let marker = TRUNCATION_MARKER.as_bytes();
    let count = second
        .bytes
        .windows(marker.len())
        .filter(|w| *w == marker)
        .count();
    assert_eq!(count, 1);
}

#[test]
fn stderr_single_chunk_exceeding_max() {
    let cap = StderrCapture::empty();
    let cap = update_capture(cap, &vec![b'a'; MAX_STDERR_BYTES * 2]);
    assert!(cap.truncated);
    assert_eq!(cap.observed_bytes, MAX_STDERR_BYTES * 2);
    assert_eq!(cap.bytes.len(), MAX_STDERR_BYTES);
}

#[test]
fn stderr_many_small_chunks_accumulate() {
    let mut cap = StderrCapture::empty();
    for i in 0..100 {
        cap = update_capture(cap, format!("chunk{} ", i).as_bytes());
    }
    assert_eq!(cap.observed_bytes, cap.bytes.len());
    assert!(!cap.truncated);
}

#[test]
fn stderr_empty_chunk_is_noop() {
    let cap = StderrCapture {
        bytes: b"hello".to_vec(),
        truncated: false,
        observed_bytes: 5,
    };
    let cap = update_capture(cap, b"");
    assert_eq!(cap.bytes, b"hello");
    assert_eq!(cap.observed_bytes, 5);
}

#[test]
fn envelope_read_with_exactly_one_byte_header_fails() {
    let buf = vec![0x00];
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(IpcError::IncompleteRead {
            expected: 4,
            actual: 1
        })
    ));
}

#[test]
fn envelope_read_with_two_byte_header_fails() {
    let buf = vec![0x00, 0x00];
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(IpcError::IncompleteRead {
            expected: 4,
            actual: 2
        })
    ));
}

#[test]
fn envelope_read_with_three_byte_header_fails() {
    let buf = vec![0x00, 0x00, 0x00];
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(IpcError::IncompleteRead {
            expected: 4,
            actual: 3
        })
    ));
}

#[test]
fn envelope_read_empty_stream_fails() {
    let buf: Vec<u8> = vec![];
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(IpcError::IncompleteRead {
            expected: 4,
            actual: 0
        })
    ));
}

#[test]
fn envelope_read_valid_header_but_zero_payload() {
    let mut buf = 0u32.to_be_bytes().to_vec();
    buf.extend(b"{}");
    let result: Result<Fd3Envelope, IpcError> = read_envelope(&mut Cursor::new(&buf[..4]));
    match result {
        Err(IpcError::IncompleteRead { .. }) => {}
        Err(IpcError::InvalidJson(_)) => {}
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn write_envelope_to_vec_is_correct_length() {
    let env = Fd4Envelope {
        version: 1,
        instance_id: "i".into(),
        node_id: "n".into(),
        result: TaskResult::Success {
            output: serde_json::json!(42),
        },
    };
    let mut buf = Vec::new();
    write_envelope(&mut buf, &env).unwrap();
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    assert_eq!(buf.len(), len + 4);
}
