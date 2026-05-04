//! SRT subtitle output: timestamps and segments to `.srt` format.
//!
//! - [`format_timestamp`]: seconds → `HH:MM:SS,mmm` (SRT uses comma, not dot).
//! - [`format`]: a slice of [`Segment`]s → full `.srt` body, 1-indexed.

/// A timed transcription segment.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// Start time in seconds.
    pub start: f64,
    /// End time in seconds.
    pub end: f64,
    /// Spoken text for this segment.
    pub text: String,
}

/// Formats a duration in seconds as an SRT timestamp `HH:MM:SS,mmm`.
///
/// Negative values clamp to zero. Values are rounded to the nearest
/// millisecond and computed in integer ms to avoid floating-point carry
/// errors at second boundaries (e.g. `0.9995s` → `00:00:01,000`).
pub fn format_timestamp(seconds: f64) -> String {
    let secs = seconds.max(0.0);
    let total_ms = (secs * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms / 60_000) % 60;
    let s = (total_ms / 1000) % 60;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

/// Renders a slice of segments as SRT body. 1-indexed, blank line between
/// blocks, segment text trimmed. Empty input → empty string.
pub fn format(segments: &[Segment]) -> String {
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&(i + 1).to_string());
        out.push('\n');
        out.push_str(&format_timestamp(seg.start));
        out.push_str(" --> ");
        out.push_str(&format_timestamp(seg.end));
        out.push('\n');
        out.push_str(seg.text.trim());
        out.push_str("\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // === format_timestamp ===

    #[test]
    fn timestamp_zero() {
        assert_eq!(format_timestamp(0.0), "00:00:00,000");
    }

    #[test]
    fn timestamp_subsecond() {
        assert_eq!(format_timestamp(0.234), "00:00:00,234");
    }

    #[test]
    fn timestamp_one_minute_one_second() {
        assert_eq!(format_timestamp(61.234), "00:01:01,234");
    }

    #[test]
    fn timestamp_over_an_hour() {
        assert_eq!(format_timestamp(3661.5), "01:01:01,500");
    }

    #[test]
    fn timestamp_carries_when_milliseconds_round_up() {
        // 0.9995 * 1000 = 999.5 → rounds to 1000 ms → must carry to 1 second
        assert_eq!(format_timestamp(0.9995), "00:00:01,000");
    }

    #[test]
    fn timestamp_does_not_carry_when_below_half() {
        assert_eq!(format_timestamp(0.9994), "00:00:00,999");
    }

    #[test]
    fn timestamp_negative_clamps_to_zero() {
        assert_eq!(format_timestamp(-1.0), "00:00:00,000");
        assert_eq!(format_timestamp(-0.1), "00:00:00,000");
    }

    // === format ===

    #[test]
    fn format_empty_returns_empty() {
        assert_eq!(format(&[]), "");
    }

    #[test]
    fn format_single_segment() {
        let segs = [Segment {
            start: 0.0,
            end: 5.0,
            text: "Hello".into(),
        }];
        assert_eq!(
            format(&segs),
            "1\n00:00:00,000 --> 00:00:05,000\nHello\n\n"
        );
    }

    #[test]
    fn format_multiple_segments_numbered_1_indexed() {
        let segs = [
            Segment {
                start: 0.0,
                end: 5.0,
                text: "Hello".into(),
            },
            Segment {
                start: 5.0,
                end: 10.0,
                text: "World".into(),
            },
        ];
        let expected = "1\n00:00:00,000 --> 00:00:05,000\nHello\n\n\
                        2\n00:00:05,000 --> 00:00:10,000\nWorld\n\n";
        assert_eq!(format(&segs), expected);
    }

    #[test]
    fn format_trims_segment_text() {
        let segs = [Segment {
            start: 0.0,
            end: 5.0,
            text: "   Hola, hoy.   ".into(),
        }];
        let out = format(&segs);
        assert!(out.contains("\nHola, hoy.\n\n"));
        assert!(!out.contains("   Hola"));
    }

    #[test]
    fn format_preserves_internal_newlines_in_segment_text() {
        // Multi-line subtitle text within a single SRT block is valid.
        let segs = [Segment {
            start: 0.0,
            end: 5.0,
            text: "Line 1\nLine 2".into(),
        }];
        let out = format(&segs);
        assert!(out.contains("Line 1\nLine 2"));
    }
}
