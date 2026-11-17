//! Background threads: one serializes library jobs (import, scan), another
//! runs network enrichment so slow lookups never block imports.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::{db, enrich, library, musicbrainz};

pub const EVENT_CHANGED: &str = "library-changed";
pub const EVENT_PROGRESS: &str = "library-progress";
const ENRICH_RETRY_DELAY: Duration = Duration::from_secs(120);
const ENRICH_MAX_RETRIES: u32 = 3;

pub enum Job {
    Scan,
    Import(Vec<PathBuf>),
}

#[derive(Clone)]
pub struct Paths {
    pub db: PathBuf,
    pub root: PathBuf,
    pub covers: PathBuf,
    pub headshots: PathBuf,
}

pub struct Worker {
    jobs: Sender<Job>,
    pub busy: Arc<AtomicBool>,
}

impl Worker {
    pub fn submit(&self, job: Job) {
        self.jobs.send(job).ok();
    }
}

pub fn spawn(app: AppHandle, paths: Paths) -> Worker {
    let (jobs, rx) = mpsc::channel::<Job>();
    let (enrich_tx, enrich_rx) = mpsc::channel::<()>();
    let busy = Arc::new(AtomicBool::new(false));

    {
        let app = app.clone();
        let paths = paths.clone();
        let busy = Arc::clone(&busy);
        std::thread::Builder::new()
            .name("library".into())
            .spawn(move || {
                let mut conn = match db::connect(&paths.db) {
                    Ok(c) => c,
                    Err(e) => return eprintln!("library db: {e}"),
                };
                let emit_progress = |p: library::Progress| {
                    app.emit(EVENT_PROGRESS, p).ok();
                };
                while let Ok(job) = rx.recv() {
                    busy.store(true, Ordering::Relaxed);
                    if let Job::Import(sources) = job {
                        std::fs::create_dir_all(&paths.root).ok();
                        let report = library::import(&mut conn, &paths.root, &sources, emit_progress);
                        for (path, e) in &report.failed {
                            eprintln!("import failed for {path}: {e}");
                        }
                        app.emit(EVENT_CHANGED, ()).ok();
                    }
                    match library::scan(&mut conn, &paths.root, &paths.covers, emit_progress) {
                        Ok(report) => {
                            for (path, e) in &report.failed {
                                eprintln!("scan failed for {path}: {e}");
                            }
                        }
                        Err(e) => eprintln!("scan: {e}"),
                    }
                    busy.store(false, Ordering::Relaxed);
                    app.emit(EVENT_CHANGED, ()).ok();
                    enrich_tx.send(()).ok();
                }
            })
            .expect("failed to spawn library thread");
    }

    std::thread::Builder::new()
        .name("enrich".into())
        .spawn(move || {
            let conn = match db::connect(&paths.db) {
                Ok(c) => c,
                Err(e) => return eprintln!("enrich db: {e}"),
            };
            let client = musicbrainz::Client::new();
            let mut retries = 0;
            loop {
                // After failures (usually MusicBrainz being busy), try again later
                // even if nothing new arrives.
                let signal = if retries > 0 {
                    enrich_rx.recv_timeout(ENRICH_RETRY_DELAY)
                } else {
                    enrich_rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
                };
                match signal {
                    Ok(()) => retries = 0,
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                // Collapse a burst of requests into one pass.
                while enrich_rx.try_recv().is_ok() {}
                let failed = enrich::enrich_pending(&conn, &client, &paths.headshots, || {
                    app.emit(EVENT_CHANGED, ()).ok();
                });
                retries = if failed > 0 && retries < ENRICH_MAX_RETRIES { retries + 1 } else { 0 };
            }
        })
        .expect("failed to spawn enrich thread");

    Worker { jobs, busy }
}
