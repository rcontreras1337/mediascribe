//! Tauri commands exposed to the Svelte frontend.
//!
//! All commands return `Result<T, String>` because `tauri::command` requires
//! the error type to be `Serialize` and `String` is the cheapest path. We
//! lose typed errors at the IPC boundary but the UI gets a human-readable
//! message it can show directly.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::api_pricing::Model;
use crate::audio::{self, AudioExtractOpts};
use crate::chunk;
use crate::engines::api::ApiEngine;
use crate::ffmpeg_sidecar::{run_ffmpeg, run_ffprobe};
use crate::model_manager::{self, WhisperModel};
use crate::prompt;
use crate::settings::{self, Settings};
use crate::srt;

const CHUNK_SECONDS: u64 = 480;
const SAFETY_THRESHOLD: u64 = 540;

/// What the frontend sends to start a transcription job.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeRequest {
    /// Absolute path to the video / audio file the user picked.
    pub video_path: PathBuf,
    /// Engine to use: `"api"` or `"local"`.
    pub engine: String,
    /// API model id (`"gpt-4o-transcribe"`, `"gpt-4o-mini-transcribe"`,
    /// `"whisper-1"`). Required when `engine == "api"`.
    pub api_model: Option<String>,
    /// Local Whisper model id. Required when `engine == "local"`.
    pub local_model: Option<String>,
    /// Audio language hint (e.g. `"es"`).
    pub language: String,
    /// Initial prompt to bias the model. Required (use empty string if none).
    pub initial_prompt: String,
    /// Where to write the final `.txt` and `.srt`.
    pub output_dir: PathBuf,
}

/// What we return when the job finishes successfully.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeResponse {
    pub txt_path: PathBuf,
    pub srt_path: PathBuf,
    pub total_chunks: usize,
    pub duration_seconds: f64,
    pub warnings: Vec<String>,
}

/// Progress event emitted to the frontend during a transcription job.
/// The frontend listens with `listen<TranscribeProgress>("transcribe-progress", ...)`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeProgress {
    pub stage: &'static str,
    pub chunk: Option<usize>,
    pub total: Option<usize>,
    pub message: Option<String>,
}

/// Singleton state held by Tauri's runtime. Keeps a cached ApiEngine so we
/// don't rebuild a reqwest::Client per request.
pub struct AppState {
    pub api_engine: Mutex<Option<ApiEngine>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            api_engine: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// === Settings commands ===

#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app_data_dir: {}", e))?;
    settings::load(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings_value: Settings) -> Result<(), String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app_data_dir: {}", e))?;
    settings::save(&dir, &settings_value).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_api_key(state: State<'_, Arc<AppState>>, key: String) -> Result<(), String> {
    settings::save_openai_api_key(&key).map_err(|e| e.to_string())?;
    // Invalidate the cached engine; next call rebuilds with the new key.
    *state.api_engine.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn has_api_key() -> Result<bool, String> {
    Ok(settings::load_openai_api_key()
        .map_err(|e| e.to_string())?
        .is_some())
}

#[tauri::command]
pub async fn delete_api_key(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    settings::delete_openai_api_key().map_err(|e| e.to_string())?;
    *state.api_engine.lock().await = None;
    Ok(())
}

// === Model download command ===

/// Returns the path of the downloaded model. Emits `"model-download-progress"`
/// events as the download progresses (`{ downloaded, total }`).
#[tauri::command]
pub async fn download_model(app: AppHandle, model_id: String) -> Result<PathBuf, String> {
    let model = WhisperModel::from_id(&model_id)
        .ok_or_else(|| format!("unknown model id: {}", model_id))?;
    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("could not resolve app_data_dir: {}", e))?
        .join("models");

    if model_manager::is_present(&models_dir, model) {
        return Ok(model_manager::local_path(&models_dir, model));
    }

    let app_for_progress = app.clone();
    let path = model_manager::download(model, &models_dir, None, move |downloaded, total| {
        let _ = app_for_progress.emit(
            "model-download-progress",
            serde_json::json!({
                "modelId": model.id(),
                "downloaded": downloaded,
                "total": total,
            }),
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    Ok(path)
}

// === The main transcribe command ===

#[tauri::command]
pub async fn transcribe(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    request: TranscribeRequest,
) -> Result<TranscribeResponse, String> {
    if !request.video_path.is_file() {
        return Err(format!("video does not exist: {}", request.video_path.display()));
    }
    std::fs::create_dir_all(&request.output_dir)
        .map_err(|e| format!("create output_dir: {}", e))?;

    emit_progress(&app, "starting", None, None, None);

    // Working directory for intermediate audio files (full + chunks).
    let stem = request
        .video_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".into());
    let work_dir = request.output_dir.join(format!(".{}.work", stem));
    std::fs::create_dir_all(&work_dir).map_err(|e| format!("create work_dir: {}", e))?;
    let full_audio = work_dir.join("full.mp3");
    let chunks_dir = work_dir.join("chunks");

    // 1) Extract audio (video -> mono 16 kHz mp3 48k).
    emit_progress(&app, "extracting_audio", None, None, None);
    let extract_args = audio::build_extract_audio_args(
        &request.video_path,
        &full_audio,
        &AudioExtractOpts::default(),
    );
    run_ffmpeg(&app, &extract_args)
        .await
        .map_err(|e| e.to_string())?;

    // 2) Probe duration.
    let duration = probe_duration(&app, &full_audio).await?;
    emit_progress(
        &app,
        "audio_extracted",
        None,
        None,
        Some(format!("{:.1} min", duration / 60.0)),
    );

    // 3) Plan chunks and split full audio.
    let plan = chunk::plan(duration, CHUNK_SECONDS, SAFETY_THRESHOLD);
    if plan.is_empty() {
        cleanup_work_dir(&work_dir);
        return Err("audio has zero duration".into());
    }

    let chunk_paths = if plan.len() == 1 {
        // Short audio: a single "chunk" is the whole file.
        vec![full_audio.clone()]
    } else {
        std::fs::create_dir_all(&chunks_dir)
            .map_err(|e| format!("create chunks_dir: {}", e))?;
        split_into_chunks(&app, &full_audio, &chunks_dir, CHUNK_SECONDS).await?
    };

    // 4) Run the chosen engine.
    let engine_kind = request.engine.as_str();
    let (text, segments_for_srt, mut warnings) = match engine_kind {
        "api" => {
            run_api_engine(
                &app,
                &state,
                &chunk_paths,
                &request,
                duration,
            )
            .await?
        }
        "local" => {
            return Err(local_engine_unavailable_message());
        }
        other => {
            return Err(format!("unknown engine: {:?}", other));
        }
    };

    // 5) Write outputs.
    emit_progress(&app, "writing_outputs", None, None, None);
    let txt_path = request.output_dir.join(format!("{}.{}.txt", stem, engine_kind));
    let srt_path = request.output_dir.join(format!("{}.{}.srt", stem, engine_kind));
    std::fs::write(&txt_path, &text).map_err(|e| format!("write txt: {}", e))?;
    let srt_body = srt::format(&segments_for_srt);
    std::fs::write(&srt_path, srt_body).map_err(|e| format!("write srt: {}", e))?;

    // 6) Cleanup intermediates (caller can disable later via a flag if desired).
    if std::env::var("MEDIASCRIBE_KEEP_WORK").is_err() {
        cleanup_work_dir(&work_dir);
    } else {
        warnings.push(format!("intermediates kept at {}", work_dir.display()));
    }

    emit_progress(&app, "done", None, None, None);
    Ok(TranscribeResponse {
        txt_path,
        srt_path,
        total_chunks: chunk_paths.len(),
        duration_seconds: duration,
        warnings,
    })
}

// === Helpers ===

fn emit_progress(
    app: &AppHandle,
    stage: &'static str,
    chunk: Option<usize>,
    total: Option<usize>,
    message: Option<String>,
) {
    let _ = app.emit(
        "transcribe-progress",
        TranscribeProgress {
            stage,
            chunk,
            total,
            message,
        },
    );
}

async fn probe_duration(app: &AppHandle, audio: &Path) -> Result<f64, String> {
    let args: Vec<String> = vec![
        "-v".into(),
        "error".into(),
        "-show_entries".into(),
        "format=duration".into(),
        "-of".into(),
        "json".into(),
        audio.display().to_string(),
    ];
    let out = run_ffprobe(app, &args).await.map_err(|e| e.to_string())?;
    audio::parse_ffprobe_duration(&out.stdout)
        .map_err(|e| format!("parse ffprobe duration: {}", e))
}

async fn split_into_chunks(
    app: &AppHandle,
    full_audio: &Path,
    chunks_dir: &Path,
    chunk_seconds: u64,
) -> Result<Vec<PathBuf>, String> {
    let pattern = chunks_dir.join("chunk_%03d.mp3");
    let args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        full_audio.display().to_string(),
        "-f".into(),
        "segment".into(),
        "-segment_time".into(),
        chunk_seconds.to_string(),
        "-c".into(),
        "copy".into(),
        pattern.display().to_string(),
    ];
    run_ffmpeg(app, &args).await.map_err(|e| e.to_string())?;

    let mut chunks: Vec<PathBuf> = std::fs::read_dir(chunks_dir)
        .map_err(|e| format!("read chunks_dir: {}", e))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("mp3"))
        .collect();
    chunks.sort();
    if chunks.is_empty() {
        return Err("ffmpeg segment produced no chunks".into());
    }
    Ok(chunks)
}

async fn run_api_engine(
    app: &AppHandle,
    state: &State<'_, Arc<AppState>>,
    chunk_paths: &[PathBuf],
    request: &TranscribeRequest,
    duration: f64,
) -> Result<(String, Vec<srt::Segment>, Vec<String>), String> {
    let model_id = request
        .api_model
        .clone()
        .unwrap_or_else(|| "gpt-4o-transcribe".to_string());
    let model = Model::from_id(&model_id)
        .ok_or_else(|| format!("unknown api model: {}", model_id))?;

    // Build (or reuse) an engine.
    let api_key = settings::load_openai_api_key()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "OpenAI API key is not set. Configure it in Settings before using the API engine."
                .to_string()
        })?;

    let mut cached = state.api_engine.lock().await;
    if cached.is_none() {
        *cached = Some(ApiEngine::new(api_key).map_err(|e| e.to_string())?);
    }
    let engine = cached.as_ref().expect("just set above").clone();
    drop(cached);

    let total = chunk_paths.len();
    let mut all_text = String::new();
    let mut warnings = Vec::new();

    for (i, chunk_path) in chunk_paths.iter().enumerate() {
        emit_progress(app, "transcribing_chunk", Some(i + 1), Some(total), None);
        let raw = engine
            .transcribe_chunk(chunk_path, model, &request.language, &request.initial_prompt)
            .await
            .map_err(|e| e.to_string())?;
        let (clean, was_echoed) = prompt::filter_echo(&raw, &request.initial_prompt);
        if was_echoed {
            warnings.push(format!(
                "chunk {} contained prompt echo and was filtered",
                i + 1
            ));
        }
        if !clean.is_empty() {
            if !all_text.is_empty() {
                all_text.push('\n');
            }
            all_text.push_str(&clean);
        }
    }

    // gpt-4o-transcribe response_format=text doesn't return segments, so the
    // SRT we emit is a single block spanning the whole audio. Document this
    // in the UI so users don't expect per-line timestamps.
    let segments_for_srt = vec![srt::Segment {
        start: 0.0,
        end: duration,
        text: all_text.clone(),
    }];
    Ok((all_text, segments_for_srt, warnings))
}

fn local_engine_unavailable_message() -> String {
    if cfg!(feature = "local-engine") {
        "local engine code path is wired up but not yet implemented end-to-end. \
         Use the API engine for now.".to_string()
    } else {
        "local engine is disabled in this build. Rebuild with \
         `npm run tauri:dev:cuda` (or `cargo tauri dev --features local-engine`) \
         to include it.".to_string()
    }
}

fn cleanup_work_dir(work_dir: &Path) {
    let _ = std::fs::remove_dir_all(work_dir);
}
