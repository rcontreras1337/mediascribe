//! Helpers for the `initial_prompt` we pass to Whisper / gpt-4o-transcribe.
//!
//! - `filter_echo`: detect when an engine echoed our prompt as transcription
//!   (gpt-4o-transcribe does this with short / silent audio chunks) and remove it.
//! - `validate`: enforce the ~224-token Whisper prompt limit.
//!
//! Implementation lands in Fase 2 (TDD).

#[cfg(test)]
mod tests {
    // Tests for filter_echo and validate go here.
}
