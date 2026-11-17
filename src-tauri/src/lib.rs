mod artwork;
mod db;
mod enrich;
mod library;
mod musicbrainz;
mod names;
mod perf;
mod placeholder;
mod settings;
mod tags;
mod template;
mod worker;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::Manager;

struct AppState {
    worker: worker::Worker,
    paths: worker::Paths,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryPayload {
    root: String,
    busy: bool,
    artists: Vec<db::ArtistSummary>,
}

fn placeholder_count() -> Option<u32> {
    std::env::var("MUSIC_PLACEHOLDER_ARTISTS")
        .ok()
        .map(|v| v.parse().unwrap_or(placeholder::DEFAULT_COUNT))
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn get_library(state: tauri::State<'_, AppState>) -> Result<LibraryPayload, String> {
    let root = state.paths.root.to_string_lossy().into_owned();
    let busy = state.worker.busy.load(Ordering::Relaxed);
    if let Some(count) = placeholder_count() {
        let artists = placeholder::generate(count)
            .into_iter()
            .map(|a| db::ArtistSummary {
                id: a.id as i64,
                name: a.name,
                monogram: a.monogram,
                palette: a.palette.to_vec(),
                cover_path: None,
                headshot_path: None,
                release_count: 0,
                track_count: 0,
                mbid: None,
            })
            .collect();
        return Ok(LibraryPayload { root, busy, artists });
    }
    let db_path = state.paths.db.clone();
    let artists = blocking(move || {
        let conn = db::connect(&db_path).map_err(|e| e.to_string())?;
        db::artist_summaries(&conn).map_err(|e| e.to_string())
    })
    .await?;
    Ok(LibraryPayload { root, busy, artists })
}

#[tauri::command]
async fn get_artist(state: tauri::State<'_, AppState>, id: i64) -> Result<Option<db::ArtistDetail>, String> {
    let db_path = state.paths.db.clone();
    blocking(move || {
        let conn = db::connect(&db_path).map_err(|e| e.to_string())?;
        db::artist_detail(&conn, id).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
fn import_paths(state: tauri::State<'_, AppState>, paths: Vec<String>) {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    if !paths.is_empty() {
        state.worker.submit(worker::Job::Import(paths));
    }
}

#[derive(Serialize)]
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

/// Paths given as `--import <path>...` on the command line.
fn cli_imports() -> Vec<PathBuf> {
    let mut args = std::env::args().skip(1).skip_while(|a| a != "--import");
    if args.next().is_none() {
        return Vec::new();
    }
    args.take_while(|a| !a.starts_with("--")).map(PathBuf::from).collect()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(perf::Monitor::new()))
        .setup(|app| {
            let path = app.path();
            let data = path.app_local_data_dir()?;
            let default_root = path
                .audio_dir()
                .unwrap_or_else(|_| data.clone())
                .join("Local Music");
            let settings = settings::load(&path.app_config_dir()?, default_root);
            let cache = data.join("cache");
            let paths = worker::Paths {
                db: data.join("library.db"),
                root: settings.library_root,
                covers: cache.join("covers"),
                headshots: cache.join("headshots"),
            };
            std::fs::create_dir_all(&data)?;
            db::init(&paths.db)?;
            if std::env::args().any(|a| a == "--refresh-metadata") {
                db::reset_enrichment(&db::connect(&paths.db)?)?;
            }

            let worker = worker::spawn(app.handle().clone(), paths.clone());
            let imports = cli_imports();
            worker.submit(if imports.is_empty() {
                worker::Job::Scan
            } else {
                worker::Job::Import(imports)
            });
            app.manage(AppState { worker, paths });

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
        .invoke_handler(tauri::generate_handler![
            get_library,
            get_artist,
            import_paths,
            launch_options,
            perf_stats
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
