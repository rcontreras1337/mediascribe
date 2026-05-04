//! Transcription engines: trait and implementations.
//!
//! - `local` uses whisper-rs (whisper.cpp) — Fase 4.
//! - `api` uses reqwest against OpenAI — Fase 5.
//!
//! Both implement the same trait so the orchestrator can swap them per-request.

pub mod api;
pub mod local;

// pub trait TranscriptionEngine { ... } — to be defined in Fase 4 once we know
// exactly what shape the chunked-flow needs.

#[cfg(test)]
mod tests {}
