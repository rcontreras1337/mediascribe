//! Helpers for the `initial_prompt` we pass to Whisper / gpt-4o-transcribe.
//!
//! - [`filter_echo`]: detect when an engine echoed our prompt as transcription
//!   (gpt-4o-transcribe does this with short / silent audio chunks) and remove
//!   it from the response.
//! - [`validate`]: enforce Whisper's ~224-token prompt limit (rough estimate).

/// Whisper's documented prompt limit is ~224 tokens.
pub const MAX_PROMPT_TOKENS: usize = 224;

/// Length (in chars) of the prompt prefix we use to detect echoed prompts.
/// Long enough to avoid false positives on common phrases, short enough to
/// catch echoes when the engine truncates the prompt.
const ECHO_SIGNATURE_CHARS: usize = 60;

/// Errors returned by [`validate`].
#[derive(Debug, PartialEq, Eq)]
pub enum PromptError {
    /// The prompt exceeds the model's token limit (estimated).
    TooLong {
        estimated_tokens: usize,
        max: usize,
    },
}

/// Rough token estimate for Spanish + English mixed text. We don't ship a real
/// tokenizer in the bootstrap; this is a defensive heuristic. 1 token ≈ 3 chars
/// on average for our domain, slightly conservative on the side of "too many".
fn estimate_tokens(text: &str) -> usize {
    text.chars().count() / 3
}

/// Validates that a prompt fits Whisper's ~224-token limit (estimated).
pub fn validate(prompt: &str) -> Result<(), PromptError> {
    let tokens = estimate_tokens(prompt);
    if tokens > MAX_PROMPT_TOKENS {
        Err(PromptError::TooLong {
            estimated_tokens: tokens,
            max: MAX_PROMPT_TOKENS,
        })
    } else {
        Ok(())
    }
}

/// Strips the prompt from `text` if the engine echoed it back as transcription.
///
/// Returns `(cleaned_text, did_filter)`.
///
/// gpt-4o-transcribe sometimes returns the prompt verbatim instead of an
/// empty string when an audio chunk is silent or very short. We detect this
/// by looking for the first [`ECHO_SIGNATURE_CHARS`] of the prompt inside the
/// response. If found, we strip from that index forward by `prompt.len()`
/// bytes, snapping forward to the next UTF-8 boundary if needed.
pub fn filter_echo(text: &str, prompt: &str) -> (String, bool) {
    let text = text.trim();
    let prompt = prompt.trim();

    if prompt.is_empty() || text.is_empty() {
        return (text.to_string(), false);
    }

    if text == prompt {
        return (String::new(), true);
    }

    let signature: String = prompt.chars().take(ECHO_SIGNATURE_CHARS).collect();
    let Some(idx) = text.find(&signature) else {
        return (text.to_string(), false);
    };

    let before = text[..idx].trim();

    // Skip past the prompt by its byte length, snapping forward to the next
    // char boundary if the echo wasn't byte-identical past the signature
    // (defensive against unicode landing inside the slice).
    let mut end = idx + prompt.len();
    if end > text.len() {
        end = text.len();
    } else {
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
    }
    let after = text[end..].trim();

    let cleaned = match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.to_string(),
        (false, true) => before.to_string(),
        (false, false) => format!("{} {}", before, after),
    };

    (cleaned, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROMPT: &str = "Clase de Python: pandas, NumPy, matplotlib, seaborn.";

    // === filter_echo ===

    #[test]
    fn no_echo_returns_text_unchanged() {
        let text = "Hola, hoy vamos a ver visualizacion.";
        let (clean, filtered) = filter_echo(text, PROMPT);
        assert_eq!(clean, text);
        assert!(!filtered);
    }

    #[test]
    fn exact_match_returns_empty_and_filtered() {
        let (clean, filtered) = filter_echo(PROMPT, PROMPT);
        assert_eq!(clean, "");
        assert!(filtered);
    }

    #[test]
    fn strips_echo_at_end_of_text() {
        let real = "Hola, hoy vamos a ver visualizacion.";
        let text = format!("{} {}", real, PROMPT);
        let (clean, filtered) = filter_echo(&text, PROMPT);
        assert_eq!(clean, real);
        assert!(filtered);
    }

    #[test]
    fn strips_echo_at_start_of_text() {
        let real = "Hola, hoy vamos a ver visualizacion.";
        let text = format!("{} {}", PROMPT, real);
        let (clean, filtered) = filter_echo(&text, PROMPT);
        assert_eq!(clean, real);
        assert!(filtered);
    }

    #[test]
    fn strips_echo_in_middle_keeping_before_and_after() {
        let prefix = "Hola.";
        let suffix = "Hasta luego.";
        let text = format!("{} {} {}", prefix, PROMPT, suffix);
        let (clean, filtered) = filter_echo(&text, PROMPT);
        assert_eq!(clean, "Hola. Hasta luego.");
        assert!(filtered);
    }

    #[test]
    fn empty_text_returns_empty_no_filter() {
        let (clean, filtered) = filter_echo("", PROMPT);
        assert_eq!(clean, "");
        assert!(!filtered);
    }

    #[test]
    fn empty_prompt_returns_text_unchanged() {
        let text = "Hola.";
        let (clean, filtered) = filter_echo(text, "");
        assert_eq!(clean, text);
        assert!(!filtered);
    }

    #[test]
    fn trims_whitespace_from_input() {
        let text = "   Hola, hoy.   ";
        let (clean, filtered) = filter_echo(text, PROMPT);
        assert_eq!(clean, "Hola, hoy.");
        assert!(!filtered);
    }

    #[test]
    fn detects_echo_via_signature_when_engine_truncates_prompt() {
        // Prompt long enough that the 60-char signature is a strict prefix.
        let long_prompt =
            "Clase de Python con pandas, NumPy, matplotlib, seaborn. Conceptos: DataFrame, scatter.";
        // The engine returned only the first 70 chars of the prompt (truncated echo).
        let truncated_echo = &long_prompt[..70];
        let text = format!("Hola, hoy. {}", truncated_echo);

        let (clean, filtered) = filter_echo(&text, long_prompt);
        assert!(filtered, "should detect echo via 60-char signature");
        // `end` lands beyond `text.len()` because we use full prompt.len() to
        // skip; that's intentional — anything after a partial echo is unreliable.
        assert_eq!(clean, "Hola, hoy.");
    }

    #[test]
    fn handles_unicode_after_echo_without_panic() {
        // Reproduces the case where text after the echo contains multi-byte
        // chars and `idx + prompt.len()` could land on a non-boundary. We snap
        // forward to the next valid boundary.
        let prompt = "Clase de programación con pandas y NumPy.";
        let text = format!("{} acentos así: árbol, café, ñ.", prompt);
        let (clean, filtered) = filter_echo(&text, prompt);
        assert!(filtered);
        assert_eq!(clean, "acentos así: árbol, café, ñ.");
    }

    // === validate ===

    #[test]
    fn validate_short_prompt_ok() {
        assert!(validate("texto corto").is_ok());
    }

    #[test]
    fn validate_empty_prompt_ok() {
        assert!(validate("").is_ok());
    }

    #[test]
    fn validate_too_long_returns_error_with_estimate() {
        // ~3 chars per token estimate: this string crosses the limit.
        let huge = "a".repeat(MAX_PROMPT_TOKENS * 3 + 100);
        let err = validate(&huge).expect_err("should reject");
        match err {
            PromptError::TooLong {
                estimated_tokens,
                max,
            } => {
                assert!(estimated_tokens > MAX_PROMPT_TOKENS);
                assert_eq!(max, MAX_PROMPT_TOKENS);
            }
        }
    }
}
