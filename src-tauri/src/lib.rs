pub mod api_pricing;
pub mod audio;
pub mod chunk;
pub mod commands;
pub mod engines;
pub mod ffmpeg_sidecar;
pub mod model_manager;
pub mod prompt;
pub mod settings;
pub mod srt;

use std::sync::Arc;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .manage(Arc::new(commands::AppState::new()))
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::has_api_key,
            commands::delete_api_key,
            commands::download_model,
            commands::transcribe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
