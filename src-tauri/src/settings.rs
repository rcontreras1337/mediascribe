//! Persisted app settings (TOML in `app_data_dir`) and API key (in OS keystore — Fase 6).
//!
//! This module owns the on-disk config schema. The actual file I/O and the
//! `app_data_dir` resolution land in Fase 6 once we wire up Tauri commands;
//! for now we expose [`parse_toml`] / [`to_toml`] so the rest of the app can
//! be built and tested against the data model.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// On-disk app settings. Each field has a `serde(default)` so old config
/// files keep loading after we add new fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    /// Default engine for new transcriptions: `"local"` or `"api"`.
    /// User always confirms per-job; this only seeds the UI default.
    #[serde(default = "default_engine")]
    pub default_engine: String,

    /// Default local Whisper model name (e.g. `"large-v3"`).
    #[serde(default = "default_local_model")]
    pub default_local_model: String,

    /// Default OpenAI API model id (e.g. `"gpt-4o-transcribe"`).
    #[serde(default = "default_api_model")]
    pub default_api_model: String,

    /// UI language code: `"es"` or `"en"`.
    #[serde(default = "default_ui_language")]
    pub ui_language: String,

    /// Saved prompt templates: name → prompt body. `BTreeMap` for stable
    /// ordering on serialization (predictable diffs in the TOML file).
    #[serde(default)]
    pub prompt_templates: BTreeMap<String, String>,
}

fn default_engine() -> String {
    "local".to_string()
}
fn default_local_model() -> String {
    "large-v3".to_string()
}
fn default_api_model() -> String {
    "gpt-4o-transcribe".to_string()
}
fn default_ui_language() -> String {
    "es".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_engine: default_engine(),
            default_local_model: default_local_model(),
            default_api_model: default_api_model(),
            ui_language: default_ui_language(),
            prompt_templates: BTreeMap::new(),
        }
    }
}

/// Parses a TOML string into [`Settings`]. Missing fields take their default
/// value (forward-compatibility).
pub fn parse_toml(s: &str) -> Result<Settings, toml::de::Error> {
    toml::from_str(s)
}

/// Serializes [`Settings`] to TOML.
pub fn to_toml(settings: &Settings) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_what_we_expect() {
        let s = Settings::default();
        assert_eq!(s.default_engine, "local");
        assert_eq!(s.default_local_model, "large-v3");
        assert_eq!(s.default_api_model, "gpt-4o-transcribe");
        assert_eq!(s.ui_language, "es");
        assert!(s.prompt_templates.is_empty());
    }

    #[test]
    fn parse_empty_toml_yields_defaults() {
        let s = parse_toml("").expect("empty TOML should parse");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn parse_partial_toml_fills_missing_fields_with_defaults() {
        let toml = r#"default_engine = "api""#;
        let s = parse_toml(toml).expect("should parse");
        assert_eq!(s.default_engine, "api");
        // The rest take defaults
        assert_eq!(s.default_local_model, "large-v3");
        assert_eq!(s.ui_language, "es");
    }

    #[test]
    fn parse_full_toml_populates_all_fields() {
        let toml = r#"
            default_engine = "api"
            default_local_model = "small"
            default_api_model = "gpt-4o-mini-transcribe"
            ui_language = "en"
        "#;
        let s = parse_toml(toml).expect("should parse");
        assert_eq!(s.default_engine, "api");
        assert_eq!(s.default_local_model, "small");
        assert_eq!(s.default_api_model, "gpt-4o-mini-transcribe");
        assert_eq!(s.ui_language, "en");
    }

    #[test]
    fn parse_toml_with_prompt_templates() {
        let toml = r#"
            [prompt_templates]
            python = "Clase de Python con pandas y NumPy."
            estadistica = "Curso de estadistica descriptiva."
        "#;
        let s = parse_toml(toml).expect("should parse");
        assert_eq!(s.prompt_templates.len(), 2);
        assert_eq!(
            s.prompt_templates.get("python").map(String::as_str),
            Some("Clase de Python con pandas y NumPy.")
        );
        assert_eq!(
            s.prompt_templates.get("estadistica").map(String::as_str),
            Some("Curso de estadistica descriptiva.")
        );
    }

    #[test]
    fn parse_invalid_toml_returns_error() {
        let bad = "this is = not valid TOML [";
        assert!(parse_toml(bad).is_err());
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let mut original = Settings::default();
        original.default_engine = "api".into();
        original.ui_language = "en".into();
        original
            .prompt_templates
            .insert("python".into(), "Clase de Python.".into());
        original
            .prompt_templates
            .insert("ml".into(), "Curso de machine learning.".into());

        let serialized = to_toml(&original).expect("must serialize");
        let parsed = parse_toml(&serialized).expect("must parse back");
        assert_eq!(parsed, original);
    }

    #[test]
    fn round_trip_default_settings() {
        let original = Settings::default();
        let serialized = to_toml(&original).expect("must serialize");
        let parsed = parse_toml(&serialized).expect("must parse back");
        assert_eq!(parsed, original);
    }
}
