//! OpenAI API pricing tables and key format validation.
//!
//! - [`Model`]: enum of supported transcription models.
//! - [`estimate_cost`]: USD cost estimate for a given duration and model.
//! - [`validate_key_format`]: surface-level sanity check on a pasted API key.
//!   Real validation happens at the first API call; this only catches
//!   obvious paste mistakes (empty, missing `sk-` prefix, too short).
//!
//! Prices are baked in as constants. If OpenAI changes them, update here
//! AND show a "prices may have changed, verify on openai.com" notice in
//! the UI when this estimate is displayed.

/// Supported OpenAI transcription models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// `gpt-4o-transcribe`: best quality, $0.006/min.
    Gpt4oTranscribe,
    /// `gpt-4o-mini-transcribe`: half price, slightly lower quality, $0.003/min.
    Gpt4oMiniTranscribe,
    /// `whisper-1`: classic Whisper API. Same model as local large-v3,
    /// kept for users who want a non-chunked baseline. $0.006/min.
    Whisper1,
}

impl Model {
    /// USD per minute of input audio.
    pub fn price_per_minute_usd(&self) -> f64 {
        match self {
            Model::Gpt4oTranscribe => 0.006,
            Model::Gpt4oMiniTranscribe => 0.003,
            Model::Whisper1 => 0.006,
        }
    }

    /// Stable string identifier accepted by the OpenAI API.
    pub fn id(&self) -> &'static str {
        match self {
            Model::Gpt4oTranscribe => "gpt-4o-transcribe",
            Model::Gpt4oMiniTranscribe => "gpt-4o-mini-transcribe",
            Model::Whisper1 => "whisper-1",
        }
    }

    /// Parses a model id string. Returns `None` for unknown values.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "gpt-4o-transcribe" => Some(Model::Gpt4oTranscribe),
            "gpt-4o-mini-transcribe" => Some(Model::Gpt4oMiniTranscribe),
            "whisper-1" => Some(Model::Whisper1),
            _ => None,
        }
    }
}

/// Estimates the USD cost of transcribing `duration_seconds` of audio.
/// Negative or zero duration yields `0.0`.
pub fn estimate_cost(duration_seconds: f64, model: Model) -> f64 {
    if duration_seconds <= 0.0 {
        return 0.0;
    }
    let minutes = duration_seconds / 60.0;
    minutes * model.price_per_minute_usd()
}

/// Surface-level sanity check on a pasted API key. Catches obvious paste
/// mistakes — does NOT prove the key is valid (only OpenAI can do that).
///
/// Accepts: keys starting with `sk-` (incl. `sk-proj-...`), at least 20 chars,
/// containing only ASCII alphanumeric, `-`, or `_`. Whitespace around the key
/// is trimmed before checking.
pub fn validate_key_format(key: &str) -> bool {
    let key = key.trim();
    if key.is_empty() || !key.starts_with("sk-") || key.len() < 20 {
        return false;
    }
    key.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    // === estimate_cost ===

    #[test]
    fn cost_zero_duration_is_zero() {
        assert_eq!(estimate_cost(0.0, Model::Gpt4oTranscribe), 0.0);
    }

    #[test]
    fn cost_negative_duration_is_zero() {
        assert_eq!(estimate_cost(-100.0, Model::Gpt4oTranscribe), 0.0);
    }

    #[test]
    fn cost_one_minute_gpt4o_transcribe() {
        let cost = estimate_cost(60.0, Model::Gpt4oTranscribe);
        assert!((cost - 0.006).abs() < 1e-9, "got {}", cost);
    }

    #[test]
    fn cost_fifty_minutes_gpt4o_transcribe_matches_real_usage() {
        // The figure we quoted to the user for clase4.mp4 (51.2 min): ~$0.30.
        let cost = estimate_cost(50.0 * 60.0, Model::Gpt4oTranscribe);
        assert!((cost - 0.30).abs() < 1e-9, "got {}", cost);
    }

    #[test]
    fn cost_mini_is_half_of_full() {
        let dur = 1500.0;
        let full = estimate_cost(dur, Model::Gpt4oTranscribe);
        let mini = estimate_cost(dur, Model::Gpt4oMiniTranscribe);
        assert!((full - 2.0 * mini).abs() < 1e-9);
    }

    #[test]
    fn cost_whisper1_equals_gpt4o_transcribe() {
        let dur = 1234.5;
        let a = estimate_cost(dur, Model::Whisper1);
        let b = estimate_cost(dur, Model::Gpt4oTranscribe);
        assert!((a - b).abs() < 1e-9);
    }

    // === Model id round-trip ===

    #[test]
    fn model_id_round_trip_for_all_variants() {
        for m in [
            Model::Gpt4oTranscribe,
            Model::Gpt4oMiniTranscribe,
            Model::Whisper1,
        ] {
            let parsed = Model::from_id(m.id()).expect("must round-trip");
            assert_eq!(parsed, m);
        }
    }

    #[test]
    fn model_from_id_unknown_returns_none() {
        assert_eq!(Model::from_id(""), None);
        assert_eq!(Model::from_id("gpt-4"), None);
        assert_eq!(Model::from_id("WHISPER-1"), None); // case sensitive
    }

    // === validate_key_format ===

    #[test]
    fn key_format_valid_classic_sk() {
        // 51 chars total, all alphanumeric — typical sk-... shape
        let key = "sk-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789ABCDEFGHIJK";
        assert!(validate_key_format(key));
    }

    #[test]
    fn key_format_valid_project_scoped() {
        // sk-proj- prefix is the modern project-scoped key shape
        let key = "sk-proj-AbCdEfGhIjKlMnOpQrStUvWxYz_AbCdEfGhIj-KlMn";
        assert!(validate_key_format(key));
    }

    #[test]
    fn key_format_empty_is_invalid() {
        assert!(!validate_key_format(""));
        assert!(!validate_key_format("   "));
    }

    #[test]
    fn key_format_missing_prefix_is_invalid() {
        assert!(!validate_key_format("AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"));
        assert!(!validate_key_format("pk-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789"));
    }

    #[test]
    fn key_format_too_short_is_invalid() {
        assert!(!validate_key_format("sk-short"));
        assert!(!validate_key_format("sk-1234567890123456")); // 19 chars total
    }

    #[test]
    fn key_format_with_surrounding_whitespace_is_trimmed() {
        let key = "  sk-AbCdEfGhIjKlMnOpQrStUvWxYz0123456789  ";
        assert!(validate_key_format(key));
    }

    #[test]
    fn key_format_with_internal_invalid_chars_is_invalid() {
        // Space inside the key indicates a paste mistake (e.g. line break).
        assert!(!validate_key_format("sk-AbCd EfGhIjKlMnOpQrStUvWxYz01"));
        // Special chars like ! aren't part of OpenAI's alphabet.
        assert!(!validate_key_format("sk-AbCd!EfGhIjKlMnOpQrStUvWxYz01"));
    }
}
