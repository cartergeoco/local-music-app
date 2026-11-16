mod perf;
mod placeholder;

use std::sync::Arc;
use std::time::Duration;

use tauri::Manager;

#[tauri::command]
fn get_artists() -> Vec<placeholder::ArtistCard> {
    let count = std::env::var("MUSIC_PLACEHOLDER_ARTISTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(placeholder::DEFAULT_COUNT);
    placeholder::generate(count)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchOptions {
    /// Run the random scroll/filter soak test for this many seconds on launch,
    /// for unattended leak checks. Zero disables it.
    soak_seconds: u32,
}

#[tauri::command]
fn launch_options() -> LaunchOptions {
    LaunchOptions {
        soak_seconds: std::env::var("MUSIC_SOAK")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
    }
}

#[tauri::command]
async fn perf_stats(monitor: tauri::State<'_, Arc<perf::Monitor>>) -> Result<perf::Stats, String> {
    let monitor = Arc::clone(&monitor);
    tauri::async_runtime::spawn_blocking(move || monitor.sample())
        .await
        .map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .manage(Arc::new(perf::Monitor::new()))
        .setup(|app| {
            // The UI shows the window after its first frame; this is a fallback
            // so a broken frontend can never leave the app invisible.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.show();
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_artists, launch_options, perf_stats])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
