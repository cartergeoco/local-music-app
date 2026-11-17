//! Importing files into the managed library and scanning it into the index.

use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::Connection;
use serde::Serialize;
use walkdir::WalkDir;

use crate::tags::{self, TagEdits, TrackTags};
use crate::{artwork, db, names, template};

pub const UNKNOWN_ARTIST: &str = "Unknown Artist";
pub const UNKNOWN_ALBUM: &str = "Unknown Album";
/// In-progress copies are named `<stem>.lmpart.<ext>` so tag code still sees the format.
const PART_MARKER: &str = ".lmpart.";

fn part_path(dest: &Path) -> PathBuf {
    let stem = dest.file_stem().unwrap_or_default().to_string_lossy();
    let ext = dest.extension().unwrap_or_default().to_string_lossy();
    dest.with_file_name(format!("{stem}{PART_MARKER}{ext}"))
}

// ---------- planning (pure) ----------

#[derive(Debug, Clone)]
pub struct Source {
    pub path: PathBuf,
    pub tags: TrackTags,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    pub source: PathBuf,
    pub album_artist: String,
    pub album: String,
    pub year: Option<i32>,
    pub title: String,
    pub artist: String,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub disc_width: usize,
    pub ext: String,
    pub edits: TagEdits,
}

fn most_common<'a>(values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut counts: Vec<(&str, String, usize)> = Vec::new();
    for v in values {
        let n = names::normalize(v);
        match counts.iter_mut().find(|(_, k, _)| *k == n) {
            Some(entry) => entry.2 += 1,
            None => counts.push((v, n, 1)),
        }
    }
    // Ties go to the first value seen.
    counts.iter().rev().max_by_key(|(_, _, c)| *c).map(|(v, _, _)| *v)
}

fn stem(path: &Path) -> &str {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("")
}

/// Decides final metadata for each file, repairing common tag problems:
/// - missing album artist: the most common lead artist within the album
/// - missing title: taken from the file name
/// - disc encoded in file names ("1003. Title" with track 3 means disc 10)
/// - every track sharing one number while file names are numbered distinctly
pub fn plan(sources: Vec<Source>) -> Vec<Planned> {
    let mut groups: Vec<((PathBuf, String), Vec<Source>)> = Vec::new();
    for s in sources {
        let key = (
            s.path.parent().map(Path::to_path_buf).unwrap_or_default(),
            names::normalize(s.tags.album.as_deref().unwrap_or("")),
        );
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, members)) => members.push(s),
            None => groups.push((key, vec![s])),
        }
    }

    let mut out = Vec::new();
    for (_, members) in groups {
        let album_artist = most_common(members.iter().filter_map(|s| s.tags.album_artist.as_deref()))
            .or_else(|| {
                most_common(members.iter().filter_map(|s| s.tags.artist.as_deref()).map(names::primary_artist))
            })
            .unwrap_or(UNKNOWN_ARTIST)
            .to_owned();
        let album = members
            .iter()
            .find_map(|s| s.tags.album.clone())
            .unwrap_or_else(|| UNKNOWN_ALBUM.to_owned());
        let year = members.iter().filter_map(|s| s.tags.year).max();

        let numbers: Vec<Option<u32>> = members.iter().map(|s| names::leading_number(stem(&s.path))).collect();
        let distinct_numbers = numbers.iter().all(Option::is_some)
            && numbers.iter().collect::<HashSet<_>>().len() == numbers.len();
        let shared_track = members.len() > 1
            && members.iter().all(|s| s.tags.track == members[0].tags.track);
        let renumber = shared_track && distinct_numbers;

        // "301. Title" means disc 3, track 1 when most of the folder agrees with
        // that reading and it spans several discs; then the file name wins over
        // tags that disagree.
        let agrees = |s: &Source, n: u32| {
            n >= 100
                && s.tags.track.is_none_or(|t| t == n % 100)
                && s.tags.disc.is_none_or(|d| d == n / 100)
        };
        let agreeing: Vec<u32> = members
            .iter()
            .zip(&numbers)
            .filter_map(|(s, n)| n.filter(|&n| agrees(s, n)).map(|n| n / 100))
            .collect();
        let disc_encoded =
            agreeing.len() * 2 > members.len() && agreeing.iter().collect::<HashSet<_>>().len() > 1;

        let mut discs = Vec::with_capacity(members.len());
        let mut track_nos = Vec::with_capacity(members.len());
        for (s, n) in members.iter().zip(&numbers) {
            let mut disc = s.tags.disc;
            let mut track = s.tags.track;
            match *n {
                Some(n) if disc_encoded && n >= 100 => {
                    disc = Some(n / 100);
                    track = Some(n % 100);
                }
                Some(n) if renumber => track = Some(n),
                _ => {}
            }
            discs.push(disc);
            track_nos.push(track);
        }
        let max_disc = discs.iter().flatten().copied().max().unwrap_or(1);
        let disc_width = if max_disc > 1 { max_disc.to_string().len() } else { 0 };

        for ((s, disc), track) in members.into_iter().zip(discs).zip(track_nos) {
            let title = s.tags.title.clone().unwrap_or_else(|| names::title_from_stem(stem(&s.path)));
            let edits = TagEdits {
                title: s.tags.title.is_none().then(|| title.clone()),
                album_artist: s.tags.album_artist.is_none().then(|| album_artist.clone()),
                track: (track != s.tags.track).then_some(track).flatten(),
                disc: (disc != s.tags.disc).then_some(disc).flatten(),
                disc_total: (disc_width > 0 && s.tags.disc_total.is_none()).then_some(max_disc),
            };
            out.push(Planned {
                ext: s.path.extension().and_then(|e| e.to_str()).unwrap_or("mp3").to_owned(),
                artist: s.tags.artist.clone().unwrap_or_else(|| album_artist.clone()),
                source: s.path,
                album_artist: album_artist.clone(),
                album: album.clone(),
                year,
                title,
                track,
                disc,
                disc_width,
                edits,
            });
        }
    }
    out
}

// ---------- filesystem helpers ----------

pub fn collect_audio(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            for entry in WalkDir::new(p).sort_by_file_name().into_iter().flatten() {
                if entry.file_type().is_file() && tags::is_audio(entry.path()) {
                    out.push(entry.into_path());
                }
            }
        } else if p.is_file() && tags::is_audio(p) {
            out.push(p.clone());
        }
    }
    out
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = fs::File::open(path)?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn mtime_secs(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64)
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Appends " (2)", " (3)", ... before the extension until the path is free.
fn unique_path(path: PathBuf, taken: &HashSet<PathBuf>) -> PathBuf {
    if !path.exists() && !taken.contains(&path) {
        return path;
    }
    let stem = stem(&path).to_owned();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_owned();
    for n in 2.. {
        let candidate = path.with_file_name(format!("{stem} ({n}).{ext}"));
        if !candidate.exists() && !taken.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

// ---------- import ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: usize,
    pub duplicates: usize,
    pub failed: Vec<(String, String)>,
}

/// Copies `sources` into `root` following the path template. Originals are never
/// modified; tag repairs are applied to the copies only.
pub fn import(
    conn: &mut Connection,
    root: &Path,
    sources: &[PathBuf],
    mut progress: impl FnMut(Progress),
) -> ImportReport {
    let mut report = ImportReport::default();
    let files = collect_audio(sources);
    let total = files.len();

    let mut readable = Vec::with_capacity(total);
    for (i, path) in files.into_iter().enumerate() {
        progress(Progress { phase: "reading", done: i, total });
        match tags::read(&path, false) {
            Ok((tags, _)) => readable.push(Source { path, tags }),
            Err(e) => report.failed.push((path_str(&path), e)),
        }
    }

    let root_chars = root.to_string_lossy().chars().count();
    let mut taken = HashSet::new();
    let planned = plan(readable);
    for (i, item) in planned.iter().enumerate() {
        progress(Progress { phase: "copying", done: i, total });
        match import_one(conn, root, root_chars, item, &mut taken) {
            Ok(true) => report.imported += 1,
            Ok(false) => report.duplicates += 1,
            Err(e) => report.failed.push((path_str(&item.source), e)),
        }
    }
    progress(Progress { phase: "copying", done: total, total });
    report
}

/// Returns Ok(false) when the file is already in the library.
fn import_one(
    conn: &mut Connection,
    root: &Path,
    root_chars: usize,
    item: &Planned,
    taken: &mut HashSet<PathBuf>,
) -> Result<bool, String> {
    let source_hash = hash_file(&item.source).map_err(|e| e.to_string())?;
    if db::has_source_hash(conn, &source_hash).map_err(|e| e.to_string())? {
        return Ok(false);
    }

    let rel = template::render(
        &template::PathFields {
            album_artist: &item.album_artist,
            album: &item.album,
            year: item.year,
            disc: item.disc,
            disc_width: item.disc_width,
            track: item.track,
            title: &item.title,
            ext: &item.ext,
        },
        root_chars,
    );
    let dest = unique_path(root.join(rel), taken);
    taken.insert(dest.clone());
    let parent = dest.parent().ok_or("invalid destination")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;

    let part = part_path(&dest);
    let result = (|| {
        fs::copy(&item.source, &part).map_err(|e| e.to_string())?;
        if hash_file(&part).map_err(|e| e.to_string())? != source_hash {
            return Err("copy verification failed".to_owned());
        }
        // A copy with unrepaired tags is still better than no copy; the index
        // uses the planned values either way.
        if let Err(e) = tags::write(&part, &item.edits) {
            eprintln!("tags not repaired for {}: {e}", item.source.display());
        }
        fs::rename(&part, &dest).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        fs::remove_file(&part).ok();
    }
    result?;

    let meta = fs::metadata(&dest).map_err(|e| e.to_string())?;
    let (tags, _) = tags::read(&dest, false)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let artist_id = db::upsert_artist(&tx, &item.album_artist).map_err(|e| e.to_string())?;
    let release_id = db::upsert_release(&tx, artist_id, &item.album, item.year).map_err(|e| e.to_string())?;
    db::upsert_track(
        &tx,
        &db::TrackRow {
            release_id,
            path: &path_str(&dest),
            title: &item.title,
            artist: &item.artist,
            track_no: item.track,
            disc_no: item.disc,
            duration_ms: tags.duration_ms,
            bitrate: tags.bitrate_kbps,
            genre: tags.genre.as_deref(),
            size: meta.len(),
            mtime: mtime_secs(&meta),
            source_hash: Some(&source_hash),
        },
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(true)
}

// ---------- scan ----------

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub indexed: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub failed: Vec<(String, String)>,
}

/// Brings the index in line with the files under `root`. Unchanged files
/// (same size and modified time) are skipped without reading tags.
pub fn scan(
    conn: &mut Connection,
    root: &Path,
    covers_dir: &Path,
    mut progress: impl FnMut(Progress),
) -> Result<ScanReport, String> {
    let mut report = ScanReport::default();
    if !root.exists() {
        return Ok(report);
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().flatten() {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        if path.file_name().is_some_and(|n| n.to_string_lossy().contains(PART_MARKER)) {
            fs::remove_file(path).ok();
        } else if tags::is_audio(path) {
            files.push(entry.into_path());
        }
    }

    let total = files.len();
    let mut seen = HashSet::with_capacity(total);
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for (i, path) in files.iter().enumerate() {
        if i % 25 == 0 {
            progress(Progress { phase: "scanning", done: i, total });
        }
        let key = path_str(path);
        seen.insert(key.clone());
        let Ok(meta) = fs::metadata(path) else { continue };
        let fingerprint = (meta.len(), mtime_secs(&meta));
        if db::track_fingerprint(&tx, &key).map_err(|e| e.to_string())? == Some(fingerprint) {
            report.unchanged += 1;
            continue;
        }
        match tags::read(path, false) {
            Ok((t, _)) => {
                index_file(&tx, &key, &t, path, fingerprint).map_err(|e| e.to_string())?;
                report.indexed += 1;
            }
            Err(e) => report.failed.push((key, e)),
        }
    }

    for path in db::all_track_paths(&tx).map_err(|e| e.to_string())? {
        if !seen.contains(&path) {
            db::delete_track(&tx, &path).map_err(|e| e.to_string())?;
            report.removed += 1;
        }
    }
    db::prune(&tx).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    extract_covers(conn, covers_dir, &mut progress)?;
    Ok(report)
}

fn index_file(
    conn: &Connection,
    key: &str,
    t: &TrackTags,
    path: &Path,
    (size, mtime): (u64, i64),
) -> rusqlite::Result<()> {
    let artist_credit = t.artist.as_deref().unwrap_or(UNKNOWN_ARTIST);
    let album_artist = t.album_artist.as_deref().unwrap_or_else(|| names::primary_artist(artist_credit));
    let title = t.title.clone().unwrap_or_else(|| names::title_from_stem(stem(path)));
    let artist_id = db::upsert_artist(conn, album_artist)?;
    let release_id = db::upsert_release(conn, artist_id, t.album.as_deref().unwrap_or(UNKNOWN_ALBUM), t.year)?;
    db::upsert_track(
        conn,
        &db::TrackRow {
            release_id,
            path: key,
            title: &title,
            artist: artist_credit,
            track_no: t.track,
            disc_no: t.disc,
            duration_ms: t.duration_ms,
            bitrate: t.bitrate_kbps,
            genre: t.genre.as_deref(),
            size,
            mtime,
            source_hash: None,
        },
    )
}

fn extract_covers(
    conn: &Connection,
    covers_dir: &Path,
    progress: &mut impl FnMut(Progress),
) -> Result<(), String> {
    let pending = db::releases_missing_cover(conn).map_err(|e| e.to_string())?;
    let total = pending.len();
    for (i, (release_id, paths)) in pending.into_iter().enumerate() {
        progress(Progress { phase: "artwork", done: i, total });
        let cover = paths
            .iter()
            .take(4)
            .find_map(|p| tags::read(Path::new(p), true).ok().and_then(|(_, c)| c));
        let saved = cover.and_then(|c| artwork::save_cover(&c.data, release_id, covers_dir).ok());
        match saved {
            Some(s) => db::set_release_cover(conn, release_id, &path_str(&s.cover), &path_str(&s.thumb), &s.palette),
            None => db::set_release_no_cover(conn, release_id),
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
fn planned_paths(planned: &[Planned], root: &Path) -> std::collections::HashMap<PathBuf, PathBuf> {
    let root_chars = root.to_string_lossy().chars().count();
    planned
        .iter()
        .map(|p| {
            let rel = template::render(
                &template::PathFields {
                    album_artist: &p.album_artist,
                    album: &p.album,
                    year: p.year,
                    disc: p.disc,
                    disc_width: p.disc_width,
                    track: p.track,
                    title: &p.title,
                    ext: &p.ext,
                },
                root_chars,
            );
            (p.source.clone(), rel)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(path: &str, f: impl FnOnce(&mut TrackTags)) -> Source {
        let mut tags = TrackTags { title: Some("T".into()), artist: Some("A".into()), ..Default::default() };
        f(&mut tags);
        Source { path: PathBuf::from(path), tags }
    }

    fn by_source<'a>(plan: &'a [Planned], file: &str) -> &'a Planned {
        plan.iter().find(|p| p.source.ends_with(file)).unwrap()
    }

    #[test]
    fn fills_album_artist_from_album_majority() {
        let plan = plan(vec![
            src("dc/nf/a.mp3", |t| { t.album = Some("Nuclear Family".into()); t.artist = Some("Daniel Caesar".into()); }),
            src("dc/nf/b.mp3", |t| { t.album = Some("Nuclear Family".into()); t.artist = Some("Daneil Caesar".into()); }),
            src("dc/nf/c.mp3", |t| { t.album = Some("Nuclear Family".into()); t.artist = Some("Daniel Caesar, Sean Leon".into()); }),
        ]);
        assert!(plan.iter().all(|p| p.album_artist == "Daniel Caesar"));
        assert_eq!(by_source(&plan, "b.mp3").artist, "Daneil Caesar", "track credit is left alone");
        assert_eq!(by_source(&plan, "b.mp3").edits.album_artist.as_deref(), Some("Daniel Caesar"));
    }

    #[test]
    fn existing_album_artist_wins_and_is_not_rewritten() {
        let plan = plan(vec![src("k/w/1.mp3", |t| {
            t.album = Some("WAR".into());
            t.artist = Some("Kanye West, Ty Dolla $ign".into());
            t.album_artist = Some("Kanye West".into());
        })]);
        assert_eq!(plan[0].album_artist, "Kanye West");
        assert!(plan[0].edits.is_empty());
    }

    #[test]
    fn separates_albums_in_one_folder() {
        let plan = plan(vec![
            src("other/x.mp3", |t| { t.album = Some("One".into()); t.artist = Some("Post Malone".into()); }),
            src("other/y.mp3", |t| { t.album = Some("Two".into()); t.artist = Some("Skyte, Lil Tecca".into()); }),
        ]);
        assert_eq!(by_source(&plan, "x.mp3").album_artist, "Post Malone");
        assert_eq!(by_source(&plan, "y.mp3").album_artist, "Skyte");
    }

    #[test]
    fn takes_missing_titles_from_file_names() {
        let plan = plan(vec![src("f/bw/301. Life Goes On (ft. Blue Whale).mp3", |t| t.title = None)]);
        assert_eq!(plan[0].title, "Life Goes On (ft. Blue Whale)");
        assert_eq!(plan[0].edits.title.as_deref(), Some("Life Goes On (ft. Blue Whale)"));
    }

    #[test]
    fn recovers_discs_encoded_in_file_names() {
        let plan = plan(vec![
            src("f/bw/101. Anywhere.mp3", |t| { t.album = Some("bw".into()); t.track = Some(1); }),
            src("f/bw/201. Math.mp3", |t| { t.album = Some("bw".into()); t.track = Some(1); }),
            src("f/bw/1003. Bad Religion.mp3", |t| { t.album = Some("bw".into()); t.track = Some(3); }),
        ]);
        let p = by_source(&plan, "1003. Bad Religion.mp3");
        assert_eq!((p.disc, p.track, p.disc_width), (Some(10), Some(3), 2));
        assert_eq!(p.edits.disc, Some(10));
        assert_eq!(p.edits.disc_total, Some(10));
        assert_eq!(by_source(&plan, "201. Math.mp3").disc, Some(2));
    }

    #[test]
    fn file_name_disc_overrides_conflicting_tags() {
        let plan = plan(vec![
            src("f/bw/101. Anywhere.mp3", |t| { t.album = Some("bw".into()); t.track = Some(1); t.disc = Some(1); }),
            src("f/bw/202. Static.mp3", |t| { t.album = Some("bw".into()); t.track = Some(2); t.disc = Some(2); }),
            src("f/bw/301. Life Goes On.mp3", |t| { t.album = Some("bw".into()); t.track = Some(1); t.disc = Some(1); }),
        ]);
        let p = by_source(&plan, "301. Life Goes On.mp3");
        assert_eq!((p.disc, p.track, p.edits.disc), (Some(3), Some(1), Some(3)));
    }

    #[test]
    fn single_with_hundreds_number_gets_no_disc() {
        let plan = plan(vec![src("f/101. Wiseman.mp3", |t| { t.album = Some("Wiseman".into()); t.track = Some(1); })]);
        assert_eq!((plan[0].disc, plan[0].disc_width, plan[0].edits.disc), (None, 0, None));
    }

    #[test]
    fn renumbers_when_every_track_shares_one_number() {
        let plan = plan(vec![
            src("k/y2/01. AFTER YOU.mp3", |t| { t.album = Some("Yeezus II".into()); t.track = Some(1); }),
            src("k/y2/02. ALL DAY.mp3", |t| { t.album = Some("Yeezus II".into()); t.track = Some(1); }),
            src("k/y2/10. IT_S ONLY RIGHT.mp3", |t| { t.album = Some("Yeezus II".into()); t.track = Some(1); }),
        ]);
        assert_eq!(by_source(&plan, "02. ALL DAY.mp3").track, Some(2));
        assert_eq!(by_source(&plan, "02. ALL DAY.mp3").edits.track, Some(2));
        assert_eq!(by_source(&plan, "01. AFTER YOU.mp3").edits.track, None, "unchanged numbers are not rewritten");
        assert_eq!(by_source(&plan, "10. IT_S ONLY RIGHT.mp3").track, Some(10));
    }

    #[test]
    fn leaves_duplicate_file_numbers_alone() {
        let plan = plan(vec![
            src("k/m/13. DEAR SUMMER.mp3", |t| { t.album = Some("M".into()); t.track = Some(13); }),
            src("k/m/13. PRAISE.mp3", |t| { t.album = Some("M".into()); t.track = Some(13); }),
        ]);
        assert!(plan.iter().all(|p| p.track == Some(13) && p.edits.track.is_none()));
    }

    #[test]
    fn uses_latest_year_for_the_album_folder() {
        let plan = plan(vec![
            src("k/b/1.mp3", |t| { t.album = Some("BULLY".into()); t.year = Some(2025); }),
            src("k/b/2.mp3", |t| { t.album = Some("BULLY".into()); t.year = Some(2024); }),
        ]);
        let paths = planned_paths(&plan, Path::new("C:/lib"));
        assert!(paths.values().all(|p| p.starts_with("A/2025 - BULLY")));
    }

    /// Writes planned edits to temp copies and checks nothing else was lost.
    #[test]
    #[ignore]
    fn roundtrip_real_folder() {
        let dir = PathBuf::from(std::env::var("MUSIC_PREVIEW_DIR").expect("set MUSIC_PREVIEW_DIR"));
        let tmp = std::env::temp_dir().join("lm-roundtrip");
        fs::create_dir_all(&tmp).unwrap();
        let sources = collect_audio(std::slice::from_ref(&dir))
            .into_iter()
            .map(|path| Source { tags: tags::read(&path, false).unwrap().0, path })
            .collect();
        let (mut ok, mut failed) = (0, 0);
        for (i, p) in plan(sources).iter().enumerate().filter(|(_, p)| !p.edits.is_empty()) {
            let copy = tmp.join(format!("{i}.mp3"));
            fs::copy(&p.source, &copy).unwrap();
            let before = id3::Tag::read_from_path(&p.source);
            let result = tags::write(&copy, &p.edits);
            let after = id3::Tag::read_from_path(&copy);
            let (tags_after, cover_after) = tags::read(&copy, true).unwrap();
            let problem = match (&result, &before, &after) {
                (Err(e), _, _) => Some(format!("write refused: {e}")),
                (_, Ok(b), Ok(a)) if b.pictures().count() != a.pictures().count() => Some("pictures changed".into()),
                (_, Ok(b), Ok(a)) if a.frames().count() < b.frames().count() => {
                    Some(format!("frames {} -> {}", b.frames().count(), a.frames().count()))
                }
                (_, _, Err(e)) => Some(format!("unreadable after write: {e}")),
                _ if p.edits.disc.is_some_and(|d| tags_after.disc != Some(d)) => Some("disc not applied".into()),
                _ if p.edits.album_artist.is_some() && tags_after.album_artist != p.edits.album_artist => {
                    Some("album artist not applied".into())
                }
                _ if cover_after.is_none() => Some("no cover".into()),
                _ => None,
            };
            match problem {
                Some(why) => {
                    failed += 1;
                    println!("FAIL {}: {why}", p.source.display());
                }
                None => ok += 1,
            }
            fs::remove_file(&copy).ok();
        }
        println!("roundtrip ok={ok} failed={failed}");
    }

    /// Dry run against a real folder: `MUSIC_PREVIEW_DIR=<dir> cargo test preview -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn preview() {
        let dir = PathBuf::from(std::env::var("MUSIC_PREVIEW_DIR").expect("set MUSIC_PREVIEW_DIR"));
        let sources = collect_audio(std::slice::from_ref(&dir))
            .into_iter()
            .filter_map(|path| match tags::read(&path, false) {
                Ok((tags, _)) => Some(Source { path, tags }),
                Err(e) => {
                    println!("UNREADABLE {} ({e})", path.display());
                    None
                }
            })
            .collect();
        let plan = plan(sources);
        let paths = planned_paths(&plan, Path::new("C:/Users/x/Music/Local Music"));
        let mut rows: Vec<_> = plan.iter().map(|p| (&paths[&p.source], p)).collect();
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (dest, p) in rows {
            let edits = if p.edits.is_empty() { String::new() } else { format!("   edits {:?}", p.edits) };
            println!("{}{edits}", dest.display());
        }
    }
}
