//! OpenAI API transcription engine via `reqwest`.
//!
//! Targets `gpt-4o-transcribe`, `gpt-4o-mini-transcribe` and `whisper-1` on
//! the `/v1/audio/transcriptions` endpoint. One method = one chunk; the
//! orchestrator handles chunking, prompt-echo filtering, and concatenation.
//!
//! Implements exponential backoff on transient errors (429, 5xx). Hard
//! errors (4xx other than 429, network failures after retries exhausted)
//! bubble up so the UI can show them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;
use tokio::time::sleep;

use crate::api_pricing::Model;

const ENDPOINT: &str = "https://api.openai.com/v1/audio/transcriptions";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF: Duration = Duration::from_millis(500);

/// One transcription HTTP client. Hold one per app and reuse — the inner
/// `reqwest::Client` pools connections.
#[derive(Clone)]
pub struct ApiEngine {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Error)]
pub enum ApiEngineError {
    #[error("network error after {retries} retries: {source}")]
    Network {
        retries: u32,
        #[source]
        source: reqwest::Error,
    },

    #[error("api returned {status}: {body}")]
    BadStatus {
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("api key is empty")]
    EmptyKey,

    #[error("audio file not found: {0}")]
    AudioNotFound(PathBuf),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("client builder failed: {0}")]
    ClientBuilder(reqwest::Error),
}

impl ApiEngine {
    /// Builds a new engine with the given API key. The client has a 5-minute
    /// timeout — enough for a 20+ MB chunk to upload + transcribe even on
    /// slow networks, but bounded so a hung connection eventually fails.
    pub fn new(api_key: String) -> Result<Self, ApiEngineError> {
        if api_key.trim().is_empty() {
            return Err(ApiEngineError::EmptyKey);
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(ApiEngineError::ClientBuilder)?;
        Ok(Self { api_key, client })
    }

    /// Transcribes a single audio chunk. Returns the raw text (still
    /// possibly containing prompt echo — caller filters with
    /// [`crate::prompt::filter_echo`]).
    pub async fn transcribe_chunk(
        &self,
        audio_path: &Path,
        model: Model,
        language: &str,
        initial_prompt: &str,
    ) -> Result<String, ApiEngineError> {
        if !audio_path.is_file() {
            return Err(ApiEngineError::AudioNotFound(audio_path.to_path_buf()));
        }

        // Read once outside the retry loop — re-uploading the same bytes is fine.
        let bytes = tokio::fs::read(audio_path).await?;
        let filename = audio_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.mp3")
            .to_string();

        let mut last_transport_err: Option<reqwest::Error> = None;
        let mut backoff = INITIAL_BACKOFF;

        for attempt in 0..=MAX_RETRIES {
            match self
                .send_one(&bytes, &filename, model, language, initial_prompt)
                .await
            {
                Ok(text) => return Ok(text),
                Err(SendError::Transport(e)) => {
                    last_transport_err = Some(e);
                    if attempt < MAX_RETRIES {
                        sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                }
                Err(SendError::Retryable { status, body }) => {
                    if attempt < MAX_RETRIES {
                        sleep(backoff).await;
                        backoff *= 2;
                        continue;
                    }
                    return Err(ApiEngineError::BadStatus { status, body });
                }
                Err(SendError::Hard { status, body }) => {
                    return Err(ApiEngineError::BadStatus { status, body });
                }
            }
        }

        Err(ApiEngineError::Network {
            retries: MAX_RETRIES,
            source: last_transport_err
                .expect("retry loop only reaches here on transport error"),
        })
    }

    async fn send_one(
        &self,
        bytes: &[u8],
        filename: &str,
        model: Model,
        language: &str,
        initial_prompt: &str,
    ) -> Result<String, SendError> {
        // Clone the bytes per attempt: reqwest's Part takes ownership and
        // the retry loop may try multiple times.
        let part = reqwest::multipart::Part::bytes(bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str("audio/mpeg")
            .map_err(SendError::Transport)?;

        let form = reqwest::multipart::Form::new()
            .text("model", model.id())
            .text("language", language.to_string())
            .text("prompt", initial_prompt.to_string())
            .text("response_format", "text")
            .part("file", part);

        let resp = self
            .client
            .post(ENDPOINT)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(SendError::Transport)?;

        let status = resp.status();
        if status.is_success() {
            return resp.text().await.map_err(SendError::Transport);
        }

        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 429 || status.is_server_error() {
            Err(SendError::Retryable { status, body })
        } else {
            Err(SendError::Hard { status, body })
        }
    }
}

/// Internal: classify a single attempt's failure mode for retry purposes.
enum SendError {
    Transport(reqwest::Error),
    Retryable {
        status: reqwest::StatusCode,
        body: String,
    },
    Hard {
        status: reqwest::StatusCode,
        body: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_api_key_is_rejected() {
        assert!(matches!(
            ApiEngine::new("".to_string()),
            Err(ApiEngineError::EmptyKey)
        ));
        assert!(matches!(
            ApiEngine::new("   ".to_string()),
            Err(ApiEngineError::EmptyKey)
        ));
    }

    #[test]
    fn valid_key_constructs_engine() {
        let engine = ApiEngine::new("sk-test-key".to_string());
        assert!(engine.is_ok());
    }

    // Real network tests need a live API key and cost money; they live in
    // tests/integration_api.rs guarded by env vars (RUN_API_TESTS=1) and
    // are NOT run in CI per the project's test-strategy memory.
}
