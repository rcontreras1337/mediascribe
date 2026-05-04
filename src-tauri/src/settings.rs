//! Persisted app settings (TOML in `app_data_dir`) and API key (OS keystore).
//!
//! Two storage tiers, on purpose:
//! - **Settings TOML** at `<app_data_dir>/settings.toml`: non-secret prefs
//!   (default engine, default model, UI language, prompt templates). Plain
//!   text, easy to inspect and version-control if the user wants to back up.
//! - **API key in keystore** (Windows Credential Manager / macOS Keychain /
//!   libsecret) under service `mediascribe`, account `openai`. Never written
//!   to disk in plain text.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

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

// === On-disk persistence ===

/// Conventional filename inside `app_data_dir`.
pub const SETTINGS_FILENAME: &str = "settings.toml";

/// Errors from [`load`] / [`save`].
#[derive(Debug, Error)]
pub enum SettingsIoError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse settings TOML: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("could not serialize settings to TOML: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Resolves `<dir>/settings.toml`.
pub fn settings_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(SETTINGS_FILENAME)
}

/// Loads settings from `<app_data_dir>/settings.toml`. If the file doesn't
/// exist, returns [`Settings::default`] — first-run UX.
pub fn load(app_data_dir: &Path) -> Result<Settings, SettingsIoError> {
    let path = settings_path(app_data_dir);
    if !path.exists() {
        return Ok(Settings::default());
    }
    let contents = std::fs::read_to_string(&path).map_err(|source| SettingsIoError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(parse_toml(&contents)?)
}

/// Saves settings to `<app_data_dir>/settings.toml`, creating the directory
/// if needed. Atomic-ish: writes to a `.tmp` sibling then renames.
pub fn save(app_data_dir: &Path, settings: &Settings) -> Result<(), SettingsIoError> {
    std::fs::create_dir_all(app_data_dir).map_err(|source| SettingsIoError::Io {
        path: app_data_dir.to_path_buf(),
        source,
    })?;
    let path = settings_path(app_data_dir);
    let tmp = path.with_extension("toml.tmp");
    let body = to_toml(settings)?;
    std::fs::write(&tmp, body).map_err(|source| SettingsIoError::Io {
        path: tmp.clone(),
        source,
    })?;
    std::fs::rename(&tmp, &path).map_err(|source| SettingsIoError::Io {
        path: path.clone(),
        source,
    })?;
    Ok(())
}

// === API key in OS keystore ===

/// Service name we register under in the OS keystore.
pub const KEYRING_SERVICE: &str = "mediascribe";
/// Account / username used in the keystore entry. We only manage one OpenAI key.
pub const KEYRING_ACCOUNT_OPENAI: &str = "openai";

/// Errors from API key keystore operations.
#[derive(Debug, Error)]
pub enum KeyringError {
    #[error("keyring error: {0}")]
    Keyring(#[from] keyring::Error),
}

/// Stores the OpenAI API key in the OS keystore.
pub fn save_openai_api_key(key: &str) -> Result<(), KeyringError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_OPENAI)?;
    entry.set_password(key)?;
    Ok(())
}

/// Loads the OpenAI API key from the OS keystore. Returns `Ok(None)` if no
/// entry has been set yet (first-run / not configured).
pub fn load_openai_api_key() -> Result<Option<String>, KeyringError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_OPENAI)?;
    match entry.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(KeyringError::Keyring(e)),
    }
}

/// Deletes the stored OpenAI API key, if any. No-op when nothing is stored.
pub fn delete_openai_api_key() -> Result<(), KeyringError> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT_OPENAI)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(KeyringError::Keyring(e)),
    }
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

    // === settings_path / load / save ===

    #[test]
    fn settings_path_appends_filename() {
        let dir = std::path::PathBuf::from("/tmp/mediascribe");
        assert_eq!(settings_path(&dir), dir.join("settings.toml"));
    }

    #[test]
    fn load_from_missing_dir_returns_defaults() {
        let nonexistent = std::path::PathBuf::from("/this/path/does/not/exist/at/all");
        let s = load(&nonexistent).expect("missing dir should yield defaults");
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn save_then_load_round_trip_via_disk() {
        // Use a temp dir uniquely scoped to this test
        let tmp = std::env::temp_dir().join(format!(
            "mediascribe-settings-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);

        let mut original = Settings::default();
        original.default_engine = "api".into();
        original
            .prompt_templates
            .insert("python".into(), "Clase de Python.".into());

        save(&tmp, &original).expect("save should work");
        let loaded = load(&tmp).expect("load should work");
        assert_eq!(loaded, original);

        // Cleanup
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
