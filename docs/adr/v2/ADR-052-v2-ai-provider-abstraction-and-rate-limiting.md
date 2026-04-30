# ADR 052 (v2): AI Provider Abstraction and Rate Limiting

## Status

Proposed

## Context

ADR-008 establishes AI agents as first-class citizens in the vo-engine, allowing them to diagnose failures, rewrite Rust binary tasks, recompile, and redeploy. ADR-026 introduces circuit breakers to protect against AI hallucination loops during automated deployments.

However, the current architecture assumes a **single AI provider** with no abstraction for multi-provider support. This creates two critical vulnerabilities:

1. **Provider Outage Risk**: If the single AI provider (e.g., OpenAI) experiences an outage, the entire system fails - AI-powered features become unavailable, blocking autonomous healing and debugging workflows.

2. **Rate Limit Deadlock**: When the AI provider returns 429 (Too Many Requests) rate limit errors, there is no backoff and retry mechanism. Requests fail immediately, potentially blocking critical workflows.

Additionally, ADR-026's rate limiting applies to **deployment registrations** (1 per minute per workflow), not to **AI API calls**. These are orthogonal concerns requiring separate handling.

## Decision

We implement a **pluggable AI provider abstraction** with built-in rate limiting and automatic fallback capabilities.

### 1. Provider Trait

All AI providers implement a common `AIProvider` trait:

```rust
/// Errors that can occur when interacting with an AI provider.
#[derive(Debug, thiserror::Error)]
pub enum AIProviderError {
    #[error("Rate limited: retry after {retry_after:?}")]
    RateLimited { retry_after: Duration },

    #[error("Provider API error: {message}")]
    ApiError { message: String, code: u16 },

    #[error("Authentication failed")]
    AuthError,

    #[error("Provider unavailable: {reason}")]
    Unavailable { reason: String },

    #[error("Request failed: {reason}")]
    RequestFailed { reason: String },
}

/// Result type for AI provider operations.
pub type AIResult<T> = Result<T, AIProviderError>;

/// Messages for chat completion.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// Chat completion request.
#[derive(Debug, Clone)]
pub struct ChatCompletionRequest {
    pub messages: Vec<ChatMessage>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

/// Chat completion response.
#[derive(Debug, Clone)]
pub struct ChatCompletionResponse {
    pub content: String,
    pub model: String,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Trait for AI providers.
#[async_trait::async_trait]
pub trait AIProvider: Send + Sync {
    /// Provider name (e.g., "openai", "anthropic").
    fn name(&self) -> &'static str;

    /// Check if this provider is currently available.
    async fn is_available(&self) -> bool;

    /// Send a chat completion request.
    async fn complete(&self, request: ChatCompletionRequest) -> AIResult<ChatCompletionResponse>;

    /// Get the provider's rate limits.
    async fn rate_limits(&self) -> RateLimitConfig;
}

/// Rate limit configuration for a provider.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute.
    pub requests_per_minute: u32,
    /// Tokens per minute (0 if unknown).
    pub tokens_per_minute: u32,
    /// Current requests remaining (if known).
    pub requests_remaining: Option<u32>,
    /// Current tokens remaining (if known).
    pub tokens_remaining: Option<u32>,
}
```

### 2. Provider Implementations

#### OpenAI Provider

```rust
pub struct OpenAIProvider {
    api_key: String,
    base_url: Url,
    model: String,
    client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            base_url: Url::parse("https://api.openai.com/v1").unwrap(),
            model,
            client: reqwest::Client::new(),
            rate_limiter: Arc::new(RateLimiter::new(
                NonZeroU32::new(60).unwrap(), // 60 RPM
                Duration::from_secs(60),
            )),
        }
    }
}

#[async_trait::async_trait]
impl AIProvider for OpenAIProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn is_available(&self) -> bool {
        true // OpenAI doesn't have a health endpoint, assume available
    }

    async fn complete(&self, request: ChatCompletionRequest) -> AIResult<ChatCompletionResponse> {
        // Apply rate limiting with backoff
        self.rate_limiter.acquire().await?;

        let body = serde_json::json!({
            "model": self.model,
            "messages": request.messages.iter().map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        ChatRole::System => "system",
                        ChatRole::User => "user",
                        ChatRole::Assistant => "assistant",
                    },
                    "content": m.content,
                })
            }).collect::<Vec<_>>(),
            "max_tokens": request.max_tokens.unwrap_or(2048),
            "temperature": request.temperature.unwrap_or(0.7),
        });

        let response = self.client
            .post(self.base_url.join("/chat/completions").unwrap())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AIProviderError::RequestFailed { reason: e.to_string() })?;

        let status = response.status();
        if status == 429 {
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .map(Duration::from_secs)
                .unwrap_or(Duration::from_secs(60));

            return Err(AIProviderError::RateLimited { retry_after });
        }

        if status == 401 {
            return Err(AIProviderError::AuthError);
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(AIProviderError::ApiError {
                message,
                code: status.as_u16(),
            });
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            AIProviderError::RequestFailed { reason: e.to_string() }
        })?;

        Ok(ChatCompletionResponse {
            content: response_body["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            model: response_body["model"].as_str().unwrap_or(&self.model).to_string(),
            usage: TokenUsage {
                prompt_tokens: response_body["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: response_body["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: response_body["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            },
        })
    }

    async fn rate_limits(&self) -> RateLimitConfig {
        RateLimitConfig {
            requests_per_minute: 60,
            tokens_per_minute: 90000,
            requests_remaining: None,
            tokens_remaining: None,
        }
    }
}
```

#### Anthropic Provider

```rust
pub struct AnthropicProvider {
    api_key: String,
    base_url: Url,
    model: String,
    client: reqwest::Client,
    rate_limiter: Arc<RateLimiter>,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            base_url: Url::parse("https://api.anthropic.com/v1").unwrap(),
            model,
            client: reqwest::Client::new(),
            rate_limiter: Arc::new(RateLimiter::new(
                NonZeroU32::new(50).unwrap(), // 50 RPM for Claude
                Duration::from_secs(60),
            )),
        }
    }
}

#[async_trait::async_trait]
impl AIProvider for AnthropicProvider {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn complete(&self, request: ChatCompletionRequest) -> AIResult<ChatCompletionResponse> {
        self.rate_limiter.acquire().await?;

        let system_message = request.messages
            .iter()
            .filter(|m| m.role == ChatRole::System)
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        let messages: Vec<_> = request.messages
            .iter()
            .filter(|m| m.role != ChatRole::System)
            .map(|m| serde_json::json!({
                "role": match m.role {
                    ChatRole::System => "user", // Anthropic doesn't have system role in messages
                    ChatRole::User => "user",
                    ChatRole::Assistant => "assistant",
                },
                "content": m.content,
            }))
            .collect();

        let body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "System": system_message,
            "max_tokens": request.max_tokens.unwrap_or(2048),
        });

        let response = self.client
            .post(self.base_url.join("/messages").unwrap())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AIProviderError::RequestFailed { reason: e.to_string() })?;

        let status = response.status();
        if status == 429 {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .map(Duration::from_secs)
                .unwrap_or(Duration::from_secs(60));

            return Err(AIProviderError::RateLimited { retry_after });
        }

        if status == 401 {
            return Err(AIProviderError::AuthError);
        }

        if !status.is_success() {
            let message = response.text().await.unwrap_or_default();
            return Err(AIProviderError::ApiError {
                message,
                code: status.as_u16(),
            });
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            AIProviderError::RequestFailed { reason: e.to_string() }
        })?;

        Ok(ChatCompletionResponse {
            content: response_body["content"][0]["text"].as_str().unwrap_or("").to_string(),
            model: response_body["model"].as_str().unwrap_or(&self.model).to_string(),
            usage: TokenUsage {
                prompt_tokens: response_body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: response_body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: response_body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32
                    + response_body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            },
        })
    }

    async fn rate_limits(&self) -> RateLimitConfig {
        RateLimitConfig {
            requests_per_minute: 50,
            tokens_per_minute: 100000,
            requests_remaining: None,
            tokens_remaining: None,
        }
    }
}
```

### 3. Rate Limiter with Exponential Backoff

```rust
use std::sync::Arc;
use tokio::sync::{Semaphore, RwLock};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    sem: Semaphore,
    interval: Duration,
    last_reset: RwLock<Instant>,
}

impl RateLimiter {
    pub fn new(requests_per_interval: NonZeroU32, interval: Duration) -> Self {
        Self {
            sem: Semaphore::new(requests_per_interval.get() as usize),
            interval,
            last_reset: RwLock::new(Instant::now()),
        }
    }

    pub async fn acquire(&self) -> AIResult<()> {
        // Check if we need to reset the semaphore
        {
            let last_reset = self.last_reset.read().await;
            if last_reset.elapsed() >= self.interval {
                drop(last_reset);
                let mut last_reset = self.last_reset.write().await;
                *last_reset = Instant::now();
                // Re-acquire permits by adding them back
                // Note: This is a simplified approach; a token bucket would be more efficient
            }
        }

        self.sem.acquire().await
            .map_err(|_| AIProviderError::Unavailable { reason: "Rate limiter closed".to_string() })
            .map(|_| ())
    }
}

/// Acquire with exponential backoff on rate limiting.
pub async fn acquire_with_backoff(
    limiter: &RateLimiter,
    max_retries: u32,
) -> AIResult<()> {
    let mut attempts = 0;

    loop {
        match limiter.acquire().await {
            Ok(()) => return Ok(()),
            Err(AIProviderError::RateLimited { retry_after }) if attempts < max_retries => {
                attempts += 1;
                let backoff = Duration::from_secs(2u64.pow(attempts))
                    .min(*retry_after);
                tokio::time::sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

### 4. Multi-Provider Router with Fallback

```rust
pub struct AIRouter {
    providers: Vec<Arc<dyn AIProvider>>,
    current_index: RwLock<usize>,
}

impl AIRouter {
    pub fn new(providers: Vec<Arc<dyn AIProvider>>) -> Self {
        Self {
            providers,
            current_index: RwLock::new(0),
        }
    }

    /// Create a router with primary and fallback providers.
    pub fn with_fallback(primary: Arc<dyn AIProvider>, fallback: Arc<dyn AIProvider>) -> Self {
        Self {
            providers: vec![primary, fallback],
            current_index: RwLock::new(0),
        }
    }

    /// Send a request to the current provider, falling back on failure.
    pub async fn complete(&self, request: ChatCompletionRequest) -> AIResult<ChatCompletionResponse> {
        let mut errors = Vec::new();

        for i in 0..self.providers.len() {
            let provider = self.providers[i].clone();

            if !provider.is_available().await {
                errors.push(AIProviderError::Unavailable {
                    reason: format!("{} not available", provider.name()),
                });
                continue;
            }

            match provider.complete(request.clone()).await {
                Ok(response) => {
                    // Success - reset to first provider for next request
                    let mut idx = self.current_index.write().await;
                    *idx = 0;
                    return Ok(response);
                }
                Err(e) => {
                    errors.push(e);
                    // Try next provider
                    continue;
                }
            }
        }

        // All providers failed
        Err(AIProviderError::Unavailable {
            reason: format!(
                "All {} providers failed: {}",
                self.providers.len(),
                errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
            ),
        })
    }

    /// Get the currently active provider name.
    pub async fn current_provider(&self) -> &'static str {
        let idx = *self.current_index.read().await;
        self.providers.get(idx).map(|p| p.name()).unwrap_or("none")
    }
}
```

### 5. Circuit Breaker Integration

Following ADR-026 and ADR-051 patterns, the `AIRouter` integrates with the circuit breaker:

```rust
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub open_duration: Duration,
}

pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: RwLock<CircuitBreakerState>,
    inner: AIRouter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing if recovery is possible
}

impl CircuitBreaker {
    pub fn new(inner: AIRouter, config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitBreakerState::Closed),
            inner,
        }
    }

    pub async fn complete(&self, request: ChatCompletionRequest) -> AIResult<ChatCompletionResponse> {
        let state = *self.state.read().await;

        if state == CircuitBreakerState::Open {
            return Err(AIProviderError::Unavailable {
                reason: "Circuit breaker is open".to_string(),
            });
        }

        let result = self.inner.complete(request).await;

        match result {
            Ok(_) => {
                // Success - potentially transition from HalfOpen to Closed
                let mut state = self.state.write().await;
                if *state == CircuitBreakerState::HalfOpen {
                    *state = CircuitBreakerState::Closed;
                }
                Ok(result.unwrap())
            }
            Err(e) => {
                // Failure - track and potentially open the circuit
                self.record_failure().await;
                Err(e)
            }
        }
    }

    async fn record_failure(&self) {
        // Implementation tracks failures and transitions state
        // See ADR-051 for detailed state machine
    }
}
```

## Consequences

### Positive
- **Provider Flexibility**: New AI providers can be added by implementing the `AIProvider` trait
- **Resilience**: System continues operating when primary provider fails via automatic fallback
- **Rate Limit Handling**: Built-in rate limiting with exponential backoff prevents API throttling
- **Observability**: Provider name, rate limits, and error types are explicit in the API
- **Circuit Breaker Integration**: Follows established ADR-026/ADR-051 patterns for failure handling

### Negative
- **Complexity**: Additional abstraction layer increases codebase complexity
- **Latency**: Fallback to secondary provider adds latency on primary failure
- **Cost**: Multi-provider setup may increase API costs
- **Credential Management**: Multiple API keys must be secured and managed

## Invariants

| ID | Description |
|----|-------------|
| INV-052-1 | At least one provider must be available for `AIRouter::complete` to succeed |
| INV-052-2 | Rate limited requests MUST be retried with exponential backoff |
| INV-052-3 | Circuit breaker MUST transition through HalfOpen before closing after Open |
| INV-052-4 | Provider implementations MUST be Send + Sync for concurrent access |

## Configuration

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct AIProviderConfig {
    pub providers: Vec<ProviderConfig>,
    pub fallback_enabled: bool,
    pub rate_limit_backoff_max_retries: u32,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: ProviderType,
    pub api_key_secret: String,
    pub model: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
}
```

## Implementation Notes

1. **New Crate**: Create `crates/vo-ai/` for AI provider abstractions
2. **Secret Management**: API keys should be loaded via `vo_sdk::secret()` per ADR-014
3. **Async Runtime**: Use `reqwest` with `rustls` for HTTPS, `tokio` for async
4. **Testing**: Mock provider for unit tests, integration tests with real providers via environment flags
5. **Metrics**: Export `ai_provider_requests_total`, `ai_provider_errors_total`, `ai_provider_latency_seconds`

## References

- [ADR-008](ADR-008-v2-ai-native-agent-interfaces.md) — AI-Native Agent Interfaces
- [ADR-026](ADR-026-v2-ai-loop-poisoning-circuit-breakers.md) — AI Loop Poisoning Circuit Breakers
- [ADR-051](ADR-051-v2-circuit-breaker-reset-and-auto-recovery.md) — Circuit Breaker Reset and Auto-Recovery
