//! Audio chunking strategy and truncation heuristics.
//!
//! gpt-4o-transcribe limits output tokens (~2000 per request), so for long audio
//! we split into ~8-min chunks. `plan` produces the time ranges; `detect_truncation`
//! flags chunks whose density (chars / sec) is suspiciously low.
//!
//! Implementation lands in Fase 2 (TDD).

#[cfg(test)]
mod tests {
    // Tests for plan and detect_truncation go here.
}
