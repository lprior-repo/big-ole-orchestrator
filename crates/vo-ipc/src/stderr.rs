use tokio::io::AsyncReadExt;

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
/// # Errors
///
/// Returns an error if reading from the underlying reader fails.
pub async fn read_bounded_stderr<R>(mut reader: R) -> std::io::Result<StderrCapture>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut capture = StderrCapture::empty();
    let mut buffer = [0u8; 8192];

    while let Ok(n) = reader.read(&mut buffer).await {
        if n == 0 {
            break;
        }
        capture = update_capture(capture, &buffer[..n]);
    }

    Ok(finalize_capture(capture))
}
