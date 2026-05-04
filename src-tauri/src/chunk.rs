//! Audio chunking strategy and truncation heuristics.
//!
//! `gpt-4o-transcribe` limits output tokens (~2000 per request), so for long
//! audio we split into ~8-min chunks. [`plan`] produces the time ranges;
//! [`detect_truncation`] flags chunks whose density (chars / sec) is
//! suspiciously low.

/// A planned audio chunk: half-open interval `[start, end)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    /// 0-indexed.
    pub index: usize,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

impl Chunk {
    /// Duration in seconds. Always non-negative.
    pub fn duration(&self) -> f64 {
        (self.end_seconds - self.start_seconds).max(0.0)
    }
}

/// Plans how to split an audio of `total_duration_s` into chunks of at most
/// `chunk_seconds` long.
///
/// - If `total_duration_s <= 0`, returns an empty plan.
/// - If `total_duration_s <= safety_threshold_s`, returns a single chunk
///   spanning the whole audio (no chunking needed).
/// - Otherwise returns N chunks of `chunk_seconds` each, with the last one
///   possibly shorter to cover the remainder.
pub fn plan(total_duration_s: f64, chunk_seconds: u64, safety_threshold_s: u64) -> Vec<Chunk> {
    if total_duration_s <= 0.0 {
        return Vec::new();
    }
    if total_duration_s <= safety_threshold_s as f64 {
        return vec![Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: total_duration_s,
        }];
    }

    let chunk = chunk_seconds as f64;
    if chunk <= 0.0 {
        // Defensive: caller passed a zero chunk size; treat as one chunk total.
        return vec![Chunk {
            index: 0,
            start_seconds: 0.0,
            end_seconds: total_duration_s,
        }];
    }

    let mut out = Vec::new();
    let mut start = 0.0;
    let mut idx = 0;
    while start < total_duration_s {
        let end = (start + chunk).min(total_duration_s);
        out.push(Chunk {
            index: idx,
            start_seconds: start,
            end_seconds: end,
        });
        start = end;
        idx += 1;
    }
    out
}

/// Heuristic: returns `true` if the chunk seems suspiciously sparse,
/// suggesting truncation by the engine (e.g. `gpt-4o-transcribe` hitting its
/// output token limit before the audio ends, or returning the prompt echo
/// instead of real text).
///
/// Spanish narration runs ~10–12 chars/sec; values under ~6 are suspicious.
/// Returns `false` when `duration_seconds <= 0` to avoid false positives on
/// degenerate input.
pub fn detect_truncation(
    text_chars: usize,
    duration_seconds: f64,
    min_chars_per_second: f64,
) -> bool {
    if duration_seconds <= 0.0 {
        return false;
    }
    let density = text_chars as f64 / duration_seconds;
    density < min_chars_per_second
}

#[cfg(test)]
mod tests {
    use super::*;

    // === plan ===

    #[test]
    fn plan_zero_duration_returns_empty() {
        assert!(plan(0.0, 480, 540).is_empty());
        assert!(plan(-5.0, 480, 540).is_empty());
    }

    #[test]
    fn plan_short_audio_below_threshold_returns_single_chunk() {
        let p = plan(120.0, 480, 540);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].index, 0);
        assert_eq!(p[0].start_seconds, 0.0);
        assert_eq!(p[0].end_seconds, 120.0);
    }

    #[test]
    fn plan_at_exact_threshold_still_single_chunk() {
        let p = plan(540.0, 480, 540);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].end_seconds, 540.0);
    }

    #[test]
    fn plan_just_over_threshold_starts_chunking() {
        let p = plan(540.1, 480, 540);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].start_seconds, 0.0);
        assert_eq!(p[0].end_seconds, 480.0);
        assert_eq!(p[1].start_seconds, 480.0);
        assert!((p[1].end_seconds - 540.1).abs() < 1e-9);
    }

    #[test]
    fn plan_exact_multiple_produces_equal_chunks() {
        // 1440s / 480s per chunk = 3 chunks, all equal
        let p = plan(1440.0, 480, 540);
        assert_eq!(p.len(), 3);
        for (i, c) in p.iter().enumerate() {
            assert_eq!(c.index, i);
            assert_eq!(c.duration(), 480.0);
        }
        assert_eq!(p[0].start_seconds, 0.0);
        assert_eq!(p[2].end_seconds, 1440.0);
    }

    #[test]
    fn plan_with_remainder_last_chunk_is_shorter() {
        // 51 min = 3060s, chunk_seconds=480 → 3060/480 = 6.375 → 7 chunks,
        // last one 180s (this matches what we ran on clase4.mp4).
        let p = plan(3060.0, 480, 540);
        assert_eq!(p.len(), 7);
        for c in &p[..6] {
            assert_eq!(c.duration(), 480.0);
        }
        assert!((p[6].duration() - 180.0).abs() < 1e-9);
        assert_eq!(p[6].end_seconds, 3060.0);
    }

    #[test]
    fn plan_chunks_are_contiguous_and_non_overlapping() {
        let p = plan(1500.0, 480, 540);
        for w in p.windows(2) {
            assert_eq!(w[0].end_seconds, w[1].start_seconds);
        }
    }

    #[test]
    fn plan_zero_chunk_size_is_handled_defensively() {
        // Caller bug: chunk_seconds=0 would loop forever. We treat it as
        // "no chunking", returning a single span.
        let p = plan(1000.0, 0, 540);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].duration(), 1000.0);
    }

    // === detect_truncation ===

    #[test]
    fn truncation_normal_density_is_false() {
        // 5000 chars in 480 s = 10.4 c/s → above 6 threshold → false
        assert!(!detect_truncation(5000, 480.0, 6.0));
    }

    #[test]
    fn truncation_low_density_is_true() {
        // 2000 chars in 480 s = 4.16 c/s → below 6 → true
        assert!(detect_truncation(2000, 480.0, 6.0));
    }

    #[test]
    fn truncation_exactly_at_threshold_is_false() {
        // strictly less-than: at threshold should not trigger
        assert!(!detect_truncation(2880, 480.0, 6.0)); // 2880/480 = 6.0
    }

    #[test]
    fn truncation_zero_duration_returns_false() {
        // Avoid division by zero false-positives.
        assert!(!detect_truncation(0, 0.0, 6.0));
        assert!(!detect_truncation(100, 0.0, 6.0));
    }

    #[test]
    fn truncation_zero_text_with_real_duration_is_true() {
        // The infamous "model returned prompt echo, we filtered it to 0" case.
        assert!(detect_truncation(0, 180.0, 6.0));
    }

    #[test]
    fn truncation_negative_duration_returns_false() {
        assert!(!detect_truncation(100, -10.0, 6.0));
    }
}
