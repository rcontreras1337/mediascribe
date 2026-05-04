//! OpenAI API pricing tables and key format validation.
//!
//! - `estimate_cost(duration_seconds, model) -> f64`: USD cost estimate.
//! - `validate_key_format(key) -> bool`: catches obvious paste mistakes
//!   (empty, missing `sk-` prefix). Real validation happens at first API call.
//!
//! Implementation lands in Fase 2 (TDD).

#[cfg(test)]
mod tests {
    // Tests for estimate_cost and validate_key_format go here.
}
