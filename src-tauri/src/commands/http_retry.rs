//! HTTP Retry Helper
//!
//! Provides exponential backoff retry logic for HTTP requests.
//! Retries on transient errors (429, 503, 502) with configurable
//! max attempts and base delay.

use reqwest::{Client, RequestBuilder, Response};
use std::time::Duration;

use crate::error::AppError;

/// Configuration for HTTP retry behavior.
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    /// Only retry on these HTTP status codes (beyond network/timeout errors).
    pub retryable_statuses: &'static [u16],
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            retryable_statuses: &[429, 502, 503],
        }
    }
}

/// Execute an HTTP request with exponential backoff retry on transient failures.
///
/// `build_request` is called on each attempt to produce a fresh `RequestBuilder`
/// (reqwest RequestBuilders are consumed on `.send()`).
///
/// Retries on:
/// - Network connection errors (not timeouts — those suggest the server is too slow)
/// - HTTP 429 (Rate Limited), 502 (Bad Gateway), 503 (Service Unavailable)
///
/// Does NOT retry on:
/// - Client errors (4xx other than 429)
/// - Server errors that likely won't resolve (500, 501)
/// - Timeout errors (server is overloaded; more requests make it worse)
pub async fn send_with_retry<F>(
    client: &Client,
    config: &RetryConfig,
    build_request: F,
) -> Result<Response, AppError>
where
    F: Fn(&Client) -> RequestBuilder,
{
    let mut last_error: Option<AppError> = None;

    for attempt in 0..config.max_attempts {
        if attempt > 0 {
            // Exponential backoff: base_delay * 2^(attempt-1)
            let delay = config.base_delay_ms * (1u64 << (attempt - 1).min(5));
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        let request = build_request(client);
        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                if config.retryable_statuses.contains(&status) && attempt + 1 < config.max_attempts
                {
                    last_error = Some(AppError::NetworkError(format!(
                        "HTTP {} (attempt {}/{})",
                        status,
                        attempt + 1,
                        config.max_attempts
                    )));
                    continue;
                }
                return Ok(response);
            }
            Err(e) => {
                if e.is_timeout() {
                    // Don't retry timeouts — indicates server overload
                    return Err(AppError::Timeout(format!("Request timed out: {}", e)));
                }
                if attempt + 1 < config.max_attempts && e.is_connect() {
                    last_error = Some(AppError::NetworkError(format!(
                        "Connection failed (attempt {}/{}): {}",
                        attempt + 1,
                        config.max_attempts,
                        e
                    )));
                    continue;
                }
                return Err(AppError::NetworkError(format!("Request failed: {}", e)));
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| AppError::NetworkError("All retry attempts exhausted".to_string())))
}
