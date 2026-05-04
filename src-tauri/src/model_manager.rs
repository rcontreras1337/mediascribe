//! Whisper model registry + downloader.
//!
//! Models are GGML-quantized binaries hosted by ggerganov on HuggingFace.
//! We download on-demand into the app's data directory the first time a
//! given model is requested.
//!
//! See: <https://huggingface.co/ggerganov/whisper.cpp>

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use sha2::Digest;
use thiserror::Error;
use tokio::io::AsyncWriteExt;

const HF_BASE: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Whisper models we know how to fetch. Sized from tiny to large; the
/// "turbo" and quantized variants are bigger speed-vs-quality trade-offs
/// for users who don't want to pay for full `large-v3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperModel {
    Tiny,
    Base,
    Small,
    Medium,
    LargeV3,
    LargeV3Turbo,
    /// 5-bit quantized large-v3: ~1.1 GB on disk, near-large-v3 quality,
    /// notably faster on CPU.
    LargeV3Q5,
}

impl WhisperModel {
    /// Stable identifier that the UI / settings file uses.
    pub fn id(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => "tiny",
            WhisperModel::Base => "base",
            WhisperModel::Small => "small",
            WhisperModel::Medium => "medium",
            WhisperModel::LargeV3 => "large-v3",
            WhisperModel::LargeV3Turbo => "large-v3-turbo",
            WhisperModel::LargeV3Q5 => "large-v3-q5_0",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "tiny" => Some(WhisperModel::Tiny),
            "base" => Some(WhisperModel::Base),
            "small" => Some(WhisperModel::Small),
            "medium" => Some(WhisperModel::Medium),
            "large-v3" => Some(WhisperModel::LargeV3),
            "large-v3-turbo" => Some(WhisperModel::LargeV3Turbo),
            "large-v3-q5_0" => Some(WhisperModel::LargeV3Q5),
            _ => None,
        }
    }

    /// Filename inside the HuggingFace repo. Matches the on-disk filename we use too.
    pub fn ggml_filename(&self) -> &'static str {
        match self {
            WhisperModel::Tiny => "ggml-tiny.bin",
            WhisperModel::Base => "ggml-base.bin",
            WhisperModel::Small => "ggml-small.bin",
            WhisperModel::Medium => "ggml-medium.bin",
            WhisperModel::LargeV3 => "ggml-large-v3.bin",
            WhisperModel::LargeV3Turbo => "ggml-large-v3-turbo.bin",
            WhisperModel::LargeV3Q5 => "ggml-large-v3-q5_0.bin",
        }
    }

    /// Approximate on-disk size in bytes. For UI display ("~3 GB").
    pub fn approximate_size_bytes(&self) -> u64 {
        match self {
            WhisperModel::Tiny => 75 * 1024 * 1024,
            WhisperModel::Base => 142 * 1024 * 1024,
            WhisperModel::Small => 466 * 1024 * 1024,
            WhisperModel::Medium => 1_500 * 1024 * 1024,
            WhisperModel::LargeV3 => 2_900 * 1024 * 1024,
            WhisperModel::LargeV3Turbo => 1_500 * 1024 * 1024,
            WhisperModel::LargeV3Q5 => 1_100 * 1024 * 1024,
        }
    }

    /// Public download URL.
    pub fn download_url(&self) -> String {
        format!("{}/{}", HF_BASE, self.ggml_filename())
    }
}

/// Resolves where the GGML file for `model` lives (or would live) under
/// `models_dir`. `models_dir` is typically `<app_data_dir>/models/`.
pub fn local_path(models_dir: &Path, model: WhisperModel) -> PathBuf {
    models_dir.join(model.ggml_filename())
}

/// Whether a usable copy of `model` already exists on disk.
pub fn is_present(models_dir: &Path, model: WhisperModel) -> bool {
    local_path(models_dir, model).is_file()
}

/// Errors from the download flow.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("server returned status {0}")]
    BadStatus(reqwest::StatusCode),

    #[error("expected sha256 {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
}

/// Downloads `model` into `<models_dir>/<filename>` if not already there.
///
/// Streaming download: we tee bytes through a SHA-256 hasher and a `.partial`
/// file, then atomically rename on success so a partial download is never
/// mistaken for a complete one. `progress(downloaded, total)` is invoked on
/// every chunk so the UI can render a progress bar.
///
/// `expected_sha256` is optional. The HuggingFace repo doesn't publish a
/// canonical SHA-256 alongside the binaries, so for now we only compute it
/// (and surface it back) — verification kicks in if you pass a known hash.
pub async fn download(
    model: WhisperModel,
    models_dir: &Path,
    expected_sha256: Option<&str>,
    progress: impl Fn(u64, u64) + Send,
) -> Result<PathBuf, ModelError> {
    tokio::fs::create_dir_all(models_dir).await?;

    let final_path = local_path(models_dir, model);
    let partial_path = final_path.with_extension("bin.partial");

    let url = model.download_url();
    let resp = reqwest::Client::new().get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(ModelError::BadStatus(resp.status()));
    }
    let total = resp.content_length().unwrap_or(model.approximate_size_bytes());

    let mut hasher = sha2::Sha256::new();
    let mut downloaded: u64 = 0;
    let mut file = tokio::fs::File::create(&partial_path).await?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        file.write_all(&chunk).await?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        progress(downloaded, total);
    }
    file.flush().await?;
    drop(file);

    let actual_hex = hex::encode(hasher.finalize());
    if let Some(expected) = expected_sha256 {
        if !expected.eq_ignore_ascii_case(&actual_hex) {
            // Don't leave a corrupted .partial laying around
            let _ = tokio::fs::remove_file(&partial_path).await;
            return Err(ModelError::HashMismatch {
                expected: expected.to_string(),
                actual: actual_hex,
            });
        }
    }

    tokio::fs::rename(&partial_path, &final_path).await?;
    Ok(final_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Pure tests only; the actual HTTP download is integration-tier and
    // gated behind opt-in env vars (lives in tests/, not run in CI).

    #[test]
    fn id_round_trip_for_all_variants() {
        for m in [
            WhisperModel::Tiny,
            WhisperModel::Base,
            WhisperModel::Small,
            WhisperModel::Medium,
            WhisperModel::LargeV3,
            WhisperModel::LargeV3Turbo,
            WhisperModel::LargeV3Q5,
        ] {
            let parsed = WhisperModel::from_id(m.id()).expect("must round-trip");
            assert_eq!(parsed, m);
        }
    }

    #[test]
    fn from_id_unknown_returns_none() {
        assert_eq!(WhisperModel::from_id(""), None);
        assert_eq!(WhisperModel::from_id("LARGE-V3"), None); // case-sensitive
        assert_eq!(WhisperModel::from_id("nope"), None);
    }

    #[test]
    fn ggml_filename_is_consistent_with_repo_layout() {
        // Sanity: every filename starts with "ggml-" and ends with ".bin"
        // — that's the convention in ggerganov/whisper.cpp on HF.
        for m in [
            WhisperModel::Tiny,
            WhisperModel::LargeV3,
            WhisperModel::LargeV3Q5,
        ] {
            let f = m.ggml_filename();
            assert!(f.starts_with("ggml-"), "{}", f);
            assert!(f.ends_with(".bin"), "{}", f);
        }
    }

    #[test]
    fn download_url_includes_huggingface_resolve_path() {
        let url = WhisperModel::Tiny.download_url();
        assert!(url.starts_with("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/"));
        assert!(url.ends_with("ggml-tiny.bin"));
    }

    #[test]
    fn approximate_size_increases_with_model_capacity() {
        // Loose monotonicity check — catches accidental size regressions.
        assert!(
            WhisperModel::Tiny.approximate_size_bytes()
                < WhisperModel::Small.approximate_size_bytes()
        );
        assert!(
            WhisperModel::Small.approximate_size_bytes()
                < WhisperModel::Medium.approximate_size_bytes()
        );
        assert!(
            WhisperModel::Medium.approximate_size_bytes()
                < WhisperModel::LargeV3.approximate_size_bytes()
        );
    }

    #[test]
    fn local_path_joins_models_dir_with_filename() {
        let dir = PathBuf::from("/data/models");
        let p = local_path(&dir, WhisperModel::LargeV3);
        assert_eq!(p, dir.join("ggml-large-v3.bin"));
    }

    #[test]
    fn is_present_returns_false_when_file_missing() {
        let dir = PathBuf::from("/this/path/does/not/exist");
        assert!(!is_present(&dir, WhisperModel::Tiny));
    }
}
