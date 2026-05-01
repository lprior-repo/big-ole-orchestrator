pub const MAX_STDERR_BYTES: usize = 1_048_576;
pub const TRUNCATION_MARKER: &str = "\n[... TRUNCATED AT 1MB ...]";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StderrCapture {
    pub bytes: Vec<u8>,
    pub truncated: bool,
    pub observed_bytes: usize,
}

impl StderrCapture {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }
}

#[must_use]
pub fn update_capture(current: StderrCapture, chunk: &[u8]) -> StderrCapture {
    let available = MAX_STDERR_BYTES.saturating_sub(current.bytes.len());
    let to_copy = chunk.len().min(available);

    let bytes = current
        .bytes
        .into_iter()
        .chain(chunk.iter().take(to_copy).copied())
        .collect::<Vec<u8>>();

    StderrCapture {
        bytes,
        truncated: current.truncated || to_copy < chunk.len(),
        observed_bytes: current.observed_bytes + chunk.len(),
    }
}

#[must_use]
pub fn finalize_capture(capture: StderrCapture) -> StderrCapture {
    let marker = TRUNCATION_MARKER.as_bytes();
    if capture.truncated && !capture.bytes.ends_with(marker) {
        let bytes = capture
            .bytes
            .into_iter()
            .chain(marker.iter().copied())
            .collect::<Vec<u8>>();
        StderrCapture { bytes, ..capture }
    } else {
        capture
    }
}

/// Reads stderr from a reader up to a configured maximum size.
///
/// Uses stream combinators over imperative looping.
///
/// # Errors
///
/// Returns an error if reading from the underlying reader fails.
#[tracing::instrument(skip(reader))]
pub async fn read_bounded_stderr<R>(mut reader: R) -> std::io::Result<StderrCapture>
where
    R: tokio::io::AsyncRead + Unpin,
{
    use tokio::io::AsyncReadExt;

    let mut capture = StderrCapture::empty();
    let mut buf = [0u8; 4096];

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        capture = update_capture(capture, &buf[..n]);
    }

    Ok(finalize_capture(capture))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn stderr_capture_default_is_empty() {
        let cap = StderrCapture::default();
        assert!(cap.bytes.is_empty());
        assert!(!cap.truncated);
        assert_eq!(cap.observed_bytes, 0);
    }

    #[test]
    fn stderr_capture_empty_method() {
        let cap = StderrCapture::empty();
        assert!(cap.bytes.is_empty());
    }

    #[test]
    fn stderr_capture_clone_eq() {
        let cap = StderrCapture {
            bytes: b"hello".to_vec(),
            truncated: false,
            observed_bytes: 5,
        };
        let clone = cap.clone();
        assert_eq!(cap, clone);
    }

    #[test]
    fn stderr_capture_debug() {
        let cap = StderrCapture {
            bytes: b"test".to_vec(),
            truncated: true,
            observed_bytes: 100,
        };
        let debug = format!("{:?}", cap);
        assert!(debug.contains("StderrCapture"));
    }

    #[test]
    fn update_capture_empty_to_small() {
        let cap = update_capture(StderrCapture::empty(), b"hello");
        assert_eq!(cap.bytes, b"hello");
        assert!(!cap.truncated);
        assert_eq!(cap.observed_bytes, 5);
    }

    #[test]
    fn update_capture_multiple_chunks() {
        let cap = update_capture(StderrCapture::empty(), b"hello ");
        let cap = update_capture(cap, b"world");
        assert_eq!(cap.bytes, b"hello world");
        assert_eq!(cap.observed_bytes, 11);
    }

    #[test]
    fn update_capture_truncation_mid_chunk() {
        let almost_full = StderrCapture {
            bytes: vec![b'a'; MAX_STDERR_BYTES - 3],
            truncated: false,
            observed_bytes: MAX_STDERR_BYTES - 3,
        };
        let chunk = b"12345";
        let cap = update_capture(almost_full, chunk);
        assert_eq!(cap.bytes.len(), MAX_STDERR_BYTES);
        assert!(cap.truncated);
        assert_eq!(cap.observed_bytes, MAX_STDERR_BYTES - 3 + 5);
    }

    #[test]
    fn update_capture_already_truncated_stays_truncated() {
        let truncated = StderrCapture {
            bytes: vec![b'x'; MAX_STDERR_BYTES],
            truncated: true,
            observed_bytes: MAX_STDERR_BYTES + 50,
        };
        let cap = update_capture(truncated, b"more");
        assert!(cap.truncated);
    }

    #[test]
    fn finalize_capture_no_truncation_unchanged() {
        let cap = StderrCapture {
            bytes: b"hello".to_vec(),
            truncated: false,
            observed_bytes: 5,
        };
        let result = finalize_capture(cap);
        assert_eq!(result.bytes, b"hello");
    }

    #[test]
    fn finalize_capture_truncated_appends_marker() {
        let cap = StderrCapture {
            bytes: vec![b'x'; MAX_STDERR_BYTES],
            truncated: true,
            observed_bytes: MAX_STDERR_BYTES + 10,
        };
        let result = finalize_capture(cap);
        assert!(result.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
    }

    #[test]
    fn finalize_capture_idempotent() {
        let cap = StderrCapture {
            bytes: vec![b'x'; MAX_STDERR_BYTES],
            truncated: true,
            observed_bytes: MAX_STDERR_BYTES + 10,
        };
        let first = finalize_capture(cap);
        let second = finalize_capture(first.clone());
        let marker = TRUNCATION_MARKER.as_bytes();
        let count = second
            .bytes
            .windows(marker.len())
            .filter(|w| *w == marker)
            .count();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn read_bounded_stderr_empty_reader() {
        let reader = tokio::io::empty();
        let capture = read_bounded_stderr(reader).await.unwrap();
        assert!(capture.bytes.is_empty());
        assert!(!capture.truncated);
    }

    #[tokio::test]
    async fn read_bounded_stderr_small_payload() {
        let data = b"hello stderr";
        let reader = Cursor::new(data.to_vec());
        let capture = read_bounded_stderr(reader).await.unwrap();
        assert_eq!(capture.bytes, data);
        assert!(!capture.truncated);
        assert_eq!(capture.observed_bytes, data.len());
    }

    #[tokio::test]
    async fn read_bounded_stderr_multiple_reads() {
        let data = b"chunk1chunk2chunk3";
        let reader = Cursor::new(data.to_vec());
        let capture = read_bounded_stderr(reader).await.unwrap();
        assert_eq!(capture.bytes, data);
    }

    #[test]
    fn update_capture_exact_limit_no_truncation() {
        let chunk = vec![b'a'; MAX_STDERR_BYTES];
        let cap = update_capture(StderrCapture::empty(), &chunk);
        assert_eq!(cap.bytes.len(), MAX_STDERR_BYTES);
        assert!(!cap.truncated);
        assert_eq!(cap.observed_bytes, MAX_STDERR_BYTES);
    }

    #[test]
    fn update_capture_one_byte_over_limit_truncates() {
        let chunk = vec![b'a'; MAX_STDERR_BYTES + 1];
        let cap = update_capture(StderrCapture::empty(), &chunk);
        assert_eq!(cap.bytes.len(), MAX_STDERR_BYTES);
        assert!(cap.truncated);
        assert_eq!(cap.observed_bytes, MAX_STDERR_BYTES + 1);
    }

    #[test]
    fn update_capture_empty_chunk_no_change() {
        let initial = StderrCapture {
            bytes: b"existing".to_vec(),
            truncated: false,
            observed_bytes: 8,
        };
        let cap = update_capture(initial.clone(), b"");
        assert_eq!(cap, initial);
    }

    #[tokio::test]
    async fn read_bounded_stderr_finalizes_on_truncation() {
        let data = vec![b'x'; MAX_STDERR_BYTES + 1000];
        let reader = Cursor::new(data);
        let capture = read_bounded_stderr(reader).await.unwrap();
        assert!(capture.bytes.ends_with(TRUNCATION_MARKER.as_bytes()));
    }

    #[test]
    fn max_stderr_bytes_is_one_megabyte() {
        assert_eq!(MAX_STDERR_BYTES, 1_048_576);
    }
}
