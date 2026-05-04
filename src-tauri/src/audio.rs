//! Audio extraction & probing helpers.
//!
//! This file holds the **pure** pieces — argument construction and ffprobe
//! JSON parsing — so they can run in `cargo test --lib` without ffmpeg
//! installed. The actual `subprocess` wrappers (`extract_audio`,
//! `probe_duration`) and the binary-distribution decision land in Fase 3b.
//!
//! Defaults mirror the validated Python flow (`transcribir.py` /
//! `transcribir_api.py`): mono, 16 kHz, mp3 @ 48 kbps. That gave ~18 MB for
//! 50 min of audio while preserving Whisper-grade transcription quality.

use std::fmt;
use std::path::Path;

use serde::Deserialize;

/// Audio codec for the extracted track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Mp3,
}

impl AudioCodec {
    /// FFmpeg codec id (`-c:a` value).
    pub fn ffmpeg_id(&self) -> &'static str {
        match self {
            AudioCodec::Mp3 => "libmp3lame",
        }
    }
}

/// Options for extracting audio from a video.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioExtractOpts {
    /// Force mono (`-ac 1`). Whisper internally downmixes anyway.
    pub mono: bool,
    /// Sample rate in Hz (`-ar`). Whisper works at 16 kHz internally.
    pub sample_rate_hz: u32,
    /// Audio bitrate in kbps (`-b:a`).
    pub bitrate_kbps: u32,
    /// Codec to encode with.
    pub codec: AudioCodec,
}

impl Default for AudioExtractOpts {
    /// Defaults that match the validated Python pipeline: mono, 16 kHz,
    /// mp3 @ 48 kbps.
    fn default() -> Self {
        Self {
            mono: true,
            sample_rate_hz: 16_000,
            bitrate_kbps: 48,
            codec: AudioCodec::Mp3,
        }
    }
}

/// Builds the FFmpeg argument vector for extracting audio from `video` into
/// `output`. Pure: no subprocess, no filesystem access.
///
/// The argument order is deterministic so tests can assert it exactly.
pub fn build_extract_audio_args(
    video: &Path,
    output: &Path,
    opts: &AudioExtractOpts,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::with_capacity(16);
    // `-y`: overwrite output without asking. Caller is responsible for not
    // pointing at a file they care about.
    args.push("-y".to_string());
    args.push("-i".to_string());
    args.push(video.display().to_string());
    // `-vn`: drop the video stream entirely.
    args.push("-vn".to_string());
    if opts.mono {
        args.push("-ac".to_string());
        args.push("1".to_string());
    }
    args.push("-ar".to_string());
    args.push(opts.sample_rate_hz.to_string());
    args.push("-c:a".to_string());
    args.push(opts.codec.ffmpeg_id().to_string());
    args.push("-b:a".to_string());
    args.push(format!("{}k", opts.bitrate_kbps));
    args.push(output.display().to_string());
    args
}

// === ffprobe duration parsing ===

#[derive(Deserialize)]
struct FfprobeRoot {
    format: FfprobeFormat,
}

#[derive(Deserialize)]
struct FfprobeFormat {
    duration: String,
}

/// Errors returned by [`parse_ffprobe_duration`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeError {
    /// The output isn't valid JSON at all.
    InvalidJson,
    /// Required field missing from the ffprobe output.
    MissingField(&'static str),
    /// `format.duration` isn't a parseable number.
    InvalidNumber(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::InvalidJson => write!(f, "ffprobe output is not valid JSON"),
            ProbeError::MissingField(field) => {
                write!(f, "ffprobe output is missing required field: {}", field)
            }
            ProbeError::InvalidNumber(s) => {
                write!(f, "ffprobe duration is not a parseable number: {:?}", s)
            }
        }
    }
}

impl std::error::Error for ProbeError {}

/// Parses the JSON output of
/// `ffprobe -show_entries format=duration -of json` into seconds.
///
/// ffprobe emits the duration as a string (e.g. `"51.234"`) inside a nested
/// `format` object; we deserialize it through serde and then parse to `f64`.
/// Extra fields in the JSON are ignored (forward-compat with future ffprobe).
pub fn parse_ffprobe_duration(json: &str) -> Result<f64, ProbeError> {
    // Fast path for whole-document parse errors.
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|_| ProbeError::InvalidJson)?;

    // Distinguish "no `format` key" from "format.duration missing" so the
    // error message is actionable.
    let format = value
        .get("format")
        .ok_or(ProbeError::MissingField("format"))?;
    let duration = format
        .get("duration")
        .ok_or(ProbeError::MissingField("format.duration"))?;
    let duration_str = duration
        .as_str()
        .ok_or(ProbeError::MissingField("format.duration"))?;

    duration_str
        .parse::<f64>()
        .map_err(|_| ProbeError::InvalidNumber(duration_str.to_string()))
}

// Round-trip type to give the strongly-typed parse a nice error path if we
// ever want to swap in. Currently unused but kept here so adding it later is
// a one-line change. Suppress dead_code so it doesn't warn.
#[allow(dead_code)]
fn parse_ffprobe_strict(json: &str) -> Result<f64, ProbeError> {
    let parsed: FfprobeRoot = serde_json::from_str(json).map_err(|_| ProbeError::InvalidJson)?;
    parsed
        .format
        .duration
        .parse::<f64>()
        .map_err(|_| ProbeError::InvalidNumber(parsed.format.duration))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // === build_extract_audio_args ===

    #[test]
    fn args_default_options_match_python_pipeline() {
        let video = PathBuf::from("clase4.mp4");
        let output = PathBuf::from("clase4.api.mp3");
        let args = build_extract_audio_args(&video, &output, &AudioExtractOpts::default());
        assert_eq!(
            args,
            vec![
                "-y",
                "-i",
                "clase4.mp4",
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "48k",
                "clase4.api.mp3",
            ]
        );
    }

    #[test]
    fn args_stereo_skips_ac_flag() {
        let opts = AudioExtractOpts {
            mono: false,
            ..AudioExtractOpts::default()
        };
        let args = build_extract_audio_args(
            Path::new("in.mp4"),
            Path::new("out.mp3"),
            &opts,
        );
        // -ac and "1" must NOT be in the args
        assert!(!args.iter().any(|a| a == "-ac"));
        // But everything else still in the same order
        assert!(args.contains(&"-ar".to_string()));
        assert!(args.contains(&"-b:a".to_string()));
    }

    #[test]
    fn args_custom_sample_rate_and_bitrate_serialized_correctly() {
        let opts = AudioExtractOpts {
            sample_rate_hz: 44_100,
            bitrate_kbps: 128,
            ..AudioExtractOpts::default()
        };
        let args = build_extract_audio_args(
            Path::new("in.mp4"),
            Path::new("out.mp3"),
            &opts,
        );
        // -ar 44100
        let ar_idx = args.iter().position(|a| a == "-ar").expect("ar present");
        assert_eq!(args[ar_idx + 1], "44100");
        // -b:a 128k
        let ba_idx = args.iter().position(|a| a == "-b:a").expect("b:a present");
        assert_eq!(args[ba_idx + 1], "128k");
    }

    #[test]
    fn args_input_and_output_paths_are_in_expected_positions() {
        let args = build_extract_audio_args(
            Path::new("foo/bar.mkv"),
            Path::new("baz/qux.mp3"),
            &AudioExtractOpts::default(),
        );
        // -i is followed by the input path
        let i_idx = args.iter().position(|a| a == "-i").expect("input present");
        assert_eq!(args[i_idx + 1], "foo/bar.mkv");
        // Output is the last argument
        assert_eq!(args.last().map(String::as_str), Some("baz/qux.mp3"));
    }

    #[test]
    fn audio_extract_opts_default_matches_validated_pipeline() {
        let d = AudioExtractOpts::default();
        assert!(d.mono);
        assert_eq!(d.sample_rate_hz, 16_000);
        assert_eq!(d.bitrate_kbps, 48);
        assert_eq!(d.codec, AudioCodec::Mp3);
    }

    // === parse_ffprobe_duration ===

    #[test]
    fn duration_valid_minimal_json() {
        let json = r#"{"format":{"duration":"51.234"}}"#;
        let d = parse_ffprobe_duration(json).expect("must parse");
        assert!((d - 51.234).abs() < 1e-9);
    }

    #[test]
    fn duration_with_extra_fields_is_ignored_forward_compat() {
        let json = r#"{
            "streams": [{"codec_name": "h264"}],
            "format": {
                "filename": "clase4.mp4",
                "duration": "3072.5",
                "size": "1234567",
                "bit_rate": "1000000"
            }
        }"#;
        let d = parse_ffprobe_duration(json).expect("must parse");
        assert!((d - 3072.5).abs() < 1e-9);
    }

    #[test]
    fn duration_invalid_json_returns_invalid_json_error() {
        let bad = "this is not json {[";
        assert_eq!(parse_ffprobe_duration(bad), Err(ProbeError::InvalidJson));
    }

    #[test]
    fn duration_missing_format_field_returns_missing_field() {
        let json = r#"{"streams": []}"#;
        assert_eq!(
            parse_ffprobe_duration(json),
            Err(ProbeError::MissingField("format"))
        );
    }

    #[test]
    fn duration_missing_duration_field_returns_missing_field() {
        let json = r#"{"format": {"filename": "x.mp4"}}"#;
        assert_eq!(
            parse_ffprobe_duration(json),
            Err(ProbeError::MissingField("format.duration"))
        );
    }

    #[test]
    fn duration_non_numeric_returns_invalid_number() {
        let json = r#"{"format":{"duration":"not-a-number"}}"#;
        match parse_ffprobe_duration(json) {
            Err(ProbeError::InvalidNumber(s)) => assert_eq!(s, "not-a-number"),
            other => panic!("expected InvalidNumber, got {:?}", other),
        }
    }

    #[test]
    fn duration_field_as_number_instead_of_string_returns_missing_field() {
        // Some ffprobe builds quote the duration; our caller path uses `-of json`
        // which always emits a string. If we ever encounter a non-string we
        // surface it as a missing-field rather than silently coercing.
        let json = r#"{"format":{"duration":51.234}}"#;
        assert_eq!(
            parse_ffprobe_duration(json),
            Err(ProbeError::MissingField("format.duration"))
        );
    }

    #[test]
    fn duration_zero_is_valid() {
        let json = r#"{"format":{"duration":"0.0"}}"#;
        let d = parse_ffprobe_duration(json).expect("must parse");
        assert_eq!(d, 0.0);
    }

    // === ProbeError display ===

    #[test]
    fn probe_error_display_messages_are_informative() {
        let e1 = ProbeError::InvalidJson;
        assert!(e1.to_string().contains("not valid JSON"));

        let e2 = ProbeError::MissingField("format.duration");
        assert!(e2.to_string().contains("format.duration"));

        let e3 = ProbeError::InvalidNumber("xyz".to_string());
        assert!(e3.to_string().contains("xyz"));
    }
}
