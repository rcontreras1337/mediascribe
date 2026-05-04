pub mod api_pricing;
pub mod audio;
pub mod chunk;
pub mod engines;
pub mod ffmpeg_sidecar;
pub mod model_manager;
pub mod prompt;
pub mod settings;
pub mod srt;

// Placeholder command kept from the Tauri scaffold; replaced with real commands
// once Fase 2+ adds them.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
