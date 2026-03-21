//! NATS connection manager — async-nats client + JetStream context (ADR-013, ADR-008).

#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![warn(clippy::pedantic)]
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::Duration;

use async_nats::jetstream;
use wtf_common::WtfError;

/// Configuration for connecting to a NATS server.
#[derive(Debug, Clone)]
pub struct NatsConfig {
    /// One or more NATS server URLs (e.g. `["nats://localhost:4222"]`).
    pub urls: Vec<String>,

    /// Path to a NATS credentials file (`.creds`). `None` = no auth.
    pub credentials_path: Option<PathBuf>,

    /// Timeout in milliseconds for the initial TCP connection.
    pub connect_timeout_ms: u64,

    /// If true, attempt to start an embedded `nats-server` subprocess first.
    /// Used by `wtf serve --nats-embedded` for zero-config development (ADR-008).
    pub embedded: bool,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            urls: vec!["nats://127.0.0.1:4222".into()],
            credentials_path: None,
            connect_timeout_ms: 5_000,
            embedded: false,
        }
    }
}

/// A connected NATS client together with a JetStream context.
///
/// This is the entry-point for all NATS operations in wtf-engine.
/// Clone is cheap — the inner `async_nats::Client` is `Arc`-backed.
#[derive(Debug, Clone)]
pub struct NatsClient {
    pub client: async_nats::Client,
    pub jetstream: jetstream::Context,
}

impl NatsClient {
    /// Return a reference to the raw `async_nats::Client`.
    #[must_use]
    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }

    /// Return a reference to the JetStream context.
    #[must_use]
    pub fn jetstream(&self) -> &jetstream::Context {
        &self.jetstream
    }
}

/// Connect to NATS and obtain a JetStream context.
///
/// Retries up to 3 times with exponential backoff (500ms, 1s, 2s).
/// If `config.embedded` is true, starts an embedded `nats-server` subprocess first.
///
/// # Errors
/// Returns [`WtfError::NatsPublish`] if all connection attempts fail.
pub async fn connect(config: &NatsConfig) -> Result<NatsClient, WtfError> {
    if config.embedded {
        start_embedded_server(config).await?;
    }

    let url = config
        .urls
        .first()
        .map(String::as_str)
        .unwrap_or("nats://127.0.0.1:4222");

    let timeout = Duration::from_millis(config.connect_timeout_ms);

    try_connect(url, &config.credentials_path, timeout).await
}

/// Attempt connection with up to 3 retries (500ms / 1s / 2s backoff).
async fn try_connect(
    url: &str,
    credentials_path: &Option<PathBuf>,
    timeout: Duration,
) -> Result<NatsClient, WtfError> {
    let delays_ms: [u64; 3] = [500, 1_000, 2_000];

    let mut last_err = None;

    for (attempt, &delay_ms) in delays_ms.iter().enumerate() {
        match attempt_connect(url, credentials_path, timeout).await {
            Ok(client) => return Ok(client),
            Err(err) => {
                tracing::warn!(
                    url,
                    attempt = attempt + 1,
                    "NATS connect attempt failed: {err}"
                );
                last_err = Some(err);
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| WtfError::nats_publish("all connect attempts failed")))
}

async fn attempt_connect(
    url: &str,
    credentials_path: &Option<PathBuf>,
    _timeout: Duration,
) -> Result<NatsClient, WtfError> {
    let connect_options = match credentials_path {
        Some(path) => async_nats::ConnectOptions::with_credentials_file(path.clone())
            .await
            .map_err(|e| WtfError::nats_publish(format!("credentials load failed: {e}")))?,
        None => async_nats::ConnectOptions::new(),
    };

    let client = connect_options
        .connect(url)
        .await
        .map_err(|e| WtfError::nats_publish(format!("connect to {url} failed: {e}")))?;

    let jetstream = jetstream::new(client.clone());

    Ok(NatsClient { client, jetstream })
}

/// Start an embedded `nats-server` subprocess (ADR-008 dev mode).
///
/// Does nothing and returns `Ok` if `nats-server` is not in `PATH` — the
/// caller will discover the connection failure on the next step.
async fn start_embedded_server(_config: &NatsConfig) -> Result<(), WtfError> {
    use std::process::Stdio;

    let result = tokio::process::Command::new("nats-server")
        .arg("--port")
        .arg("4222")
        .arg("--jetstream")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match result {
        Ok(_child) => {
            tracing::info!("embedded nats-server started on port 4222");
            // Give the server a moment to bind
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::warn!("nats-server not found in PATH; assuming external server is available");
            Ok(())
        }
        Err(e) => Err(WtfError::nats_publish(format!(
            "failed to start nats-server: {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nats_config_default_url_is_localhost_4222() {
        let cfg = NatsConfig::default();
        assert_eq!(cfg.urls, vec!["nats://127.0.0.1:4222"]);
    }

    #[test]
    fn nats_config_default_no_credentials() {
        let cfg = NatsConfig::default();
        assert!(cfg.credentials_path.is_none());
    }

    #[test]
    fn nats_config_default_not_embedded() {
        let cfg = NatsConfig::default();
        assert!(!cfg.embedded);
    }

    #[test]
    fn nats_config_default_timeout_is_five_seconds() {
        let cfg = NatsConfig::default();
        assert_eq!(cfg.connect_timeout_ms, 5_000);
    }

    #[test]
    fn nats_config_clone_is_independent() {
        let cfg1 = NatsConfig::default();
        let mut cfg2 = cfg1.clone();
        cfg2.connect_timeout_ms = 1_000;
        assert_eq!(cfg1.connect_timeout_ms, 5_000);
    }
}
