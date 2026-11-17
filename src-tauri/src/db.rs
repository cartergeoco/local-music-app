//! SQLite index of the library. Tags in the files are the source of truth;
//! everything here can be rebuilt by rescanning.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::names;

const SCHEMA_VERSION: i32 = 1;

const SCHEMA: &str = "
CREATE TABLE artists (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    norm TEXT NOT NULL UNIQUE,
    sort_name TEXT NOT NULL,
    mbid TEXT,
    mb_status TEXT NOT NULL DEFAULT 'pending',
    mb_verified INTEGER NOT NULL DEFAULT 0,
    disambiguation TEXT,
    headshot_path TEXT
);
CREATE TABLE releases (
    id INTEGER PRIMARY KEY,
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    norm TEXT NOT NULL,
    year INTEGER,
    mbid TEXT,
    mb_status TEXT NOT NULL DEFAULT 'pending',
    mb_type TEXT,
    mb_date TEXT,
    cover_path TEXT,
    cover_thumb_path TEXT,
    palette BLOB,
    UNIQUE (artist_id, norm)
);
CREATE TABLE tracks (
    id INTEGER PRIMARY KEY,
    release_id INTEGER NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
    path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    artist TEXT NOT NULL,
    track_no INTEGER,
    disc_no INTEGER,
    duration_ms INTEGER NOT NULL,
    bitrate INTEGER,
    genre TEXT,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    source_hash TEXT
);
CREATE INDEX tracks_release ON tracks(release_id);
CREATE INDEX tracks_source_hash ON tracks(source_hash);
CREATE INDEX releases_artist ON releases(artist_id);
";

/// Opens a connection to an existing index. Call [`init`] once at startup first.
pub fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

/// Creates or rebuilds the schema. Must run before any other connection opens.
pub fn init(path: &Path) -> rusqlite::Result<()> {
    let conn = connect(path)?;
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != SCHEMA_VERSION {
        // The index is a cache; rebuild it rather than migrating.
        conn.execute_batch(
            "DROP TABLE IF EXISTS tracks; DROP TABLE IF EXISTS releases; DROP TABLE IF EXISTS artists;",
        )?;
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

pub fn upsert_artist(conn: &Connection, name: &str) -> rusqlite::Result<i64> {
    let norm = names::normalize(name);
    if let Some(id) = conn
        .query_row("SELECT id FROM artists WHERE norm = ?1", [&norm], |r| r.get(0))
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO artists (name, norm, sort_name) VALUES (?1, ?2, ?3)",
        params![name, norm, names::sort_name(name)],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn upsert_release(
    conn: &Connection,
    artist_id: i64,
    title: &str,
    year: Option<i32>,
) -> rusqlite::Result<i64> {
    let norm = names::normalize(title);
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM releases WHERE artist_id = ?1 AND norm = ?2",
            params![artist_id, norm],
            |r| r.get(0),
        )
        .optional()?
    {
        if let Some(y) = year {
            // Keep the latest year seen; leak compilations often mix dates per track.
            conn.execute(
                "UPDATE releases SET year = ?2 WHERE id = ?1 AND (year IS NULL OR year < ?2)",
                params![id, y],
            )?;
        }
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO releases (artist_id, title, norm, year) VALUES (?1, ?2, ?3, ?4)",
        params![artist_id, title, norm, year],
    )?;
    Ok(conn.last_insert_rowid())
}

pub struct TrackRow<'a> {
    pub release_id: i64,
    pub path: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: u64,
    pub bitrate: Option<u32>,
    pub genre: Option<&'a str>,
    pub size: u64,
    pub mtime: i64,
    pub source_hash: Option<&'a str>,
}

pub fn upsert_track(conn: &Connection, t: &TrackRow) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO tracks (release_id, path, title, artist, track_no, disc_no, duration_ms, bitrate, genre, size, mtime, source_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(path) DO UPDATE SET
            release_id = excluded.release_id, title = excluded.title, artist = excluded.artist,
            track_no = excluded.track_no, disc_no = excluded.disc_no, duration_ms = excluded.duration_ms,
            bitrate = excluded.bitrate, genre = excluded.genre, size = excluded.size, mtime = excluded.mtime,
            source_hash = COALESCE(excluded.source_hash, tracks.source_hash)",
        params![
            t.release_id,
            t.path,
            t.title,
            t.artist,
            t.track_no,
            t.disc_no,
            t.duration_ms as i64,
            t.bitrate,
            t.genre,
            t.size as i64,
            t.mtime,
            t.source_hash
        ],
    )?;
    Ok(())
}

pub fn track_fingerprint(conn: &Connection, path: &str) -> rusqlite::Result<Option<(u64, i64)>> {
    conn.query_row("SELECT size, mtime FROM tracks WHERE path = ?1", [path], |r| {
        Ok((r.get::<_, i64>(0)? as u64, r.get(1)?))
    })
    .optional()
}

pub fn has_source_hash(conn: &Connection, hash: &str) -> rusqlite::Result<bool> {
    conn.query_row("SELECT 1 FROM tracks WHERE source_hash = ?1 LIMIT 1", [hash], |_| Ok(()))
        .optional()
        .map(|r| r.is_some())
}

pub fn all_track_paths(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM tracks")?;
    stmt.query_map([], |r| r.get(0))?.collect()
}

pub fn delete_track(conn: &Connection, path: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM tracks WHERE path = ?1", [path]).map(|_| ())
}

/// Removes releases without tracks and artists without releases.
pub fn prune(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "DELETE FROM releases WHERE id NOT IN (SELECT DISTINCT release_id FROM tracks);
         DELETE FROM artists WHERE id NOT IN (SELECT DISTINCT artist_id FROM releases);",
    )
}

/// Releases with no cached cover yet, with a track path to pull embedded art from.
pub fn releases_missing_cover(conn: &Connection) -> rusqlite::Result<Vec<(i64, Vec<String>)>> {
    let mut stmt = conn.prepare(
        "SELECT r.id, t.path FROM releases r JOIN tracks t ON t.release_id = r.id
         WHERE r.cover_path IS NULL ORDER BY r.id, t.disc_no, t.track_no",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    let mut out: Vec<(i64, Vec<String>)> = Vec::new();
    for row in rows {
        let (id, path) = row?;
        match out.last_mut() {
            Some((last, paths)) if *last == id => paths.push(path),
            _ => out.push((id, vec![path])),
        }
    }
    Ok(out)
}

pub fn set_release_cover(
    conn: &Connection,
    release_id: i64,
    cover: &str,
    thumb: &str,
    palette: &[u8; 15],
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE releases SET cover_path = ?2, cover_thumb_path = ?3, palette = ?4 WHERE id = ?1",
        params![release_id, cover, thumb, palette.as_slice()],
    )
    .map(|_| ())
}

/// Marks a release as having no usable embedded art so it is not retried every scan.
pub fn set_release_no_cover(conn: &Connection, release_id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE releases SET cover_path = '' WHERE id = ?1", [release_id]).map(|_| ())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistSummary {
    pub id: i64,
    pub name: String,
    pub monogram: String,
    pub palette: Vec<u8>,
    pub cover_path: Option<String>,
    pub headshot_path: Option<String>,
    pub release_count: u32,
    pub track_count: u32,
    pub mbid: Option<String>,
}

const NEUTRAL_PALETTE: [u8; 15] = [18, 18, 22, 40, 40, 48, 70, 70, 82, 110, 110, 124, 160, 160, 172];

pub fn artist_summaries(conn: &Connection) -> rusqlite::Result<Vec<ArtistSummary>> {
    // The card background uses the newest release that has art.
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.mbid, a.headshot_path,
            (SELECT COUNT(*) FROM releases r WHERE r.artist_id = a.id),
            (SELECT COUNT(*) FROM tracks t JOIN releases r ON r.id = t.release_id WHERE r.artist_id = a.id),
            (SELECT r.cover_path FROM releases r WHERE r.artist_id = a.id AND r.cover_path != ''
               ORDER BY r.year DESC NULLS LAST, r.id DESC LIMIT 1),
            (SELECT r.palette FROM releases r WHERE r.artist_id = a.id AND r.cover_path != ''
               ORDER BY r.year DESC NULLS LAST, r.id DESC LIMIT 1)
         FROM artists a ORDER BY a.sort_name",
    )?;
    stmt.query_map([], |r| {
        let name: String = r.get(1)?;
        let palette: Option<Vec<u8>> = r.get(7)?;
        Ok(ArtistSummary {
            id: r.get(0)?,
            monogram: names::monogram(&name),
            name,
            mbid: r.get(2)?,
            headshot_path: r.get(3)?,
            release_count: r.get(4)?,
            track_count: r.get(5)?,
            cover_path: r.get(6)?,
            palette: palette
                .filter(|p| p.len() == 15)
                .unwrap_or_else(|| NEUTRAL_PALETTE.to_vec()),
        })
    })?
    .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub duration_ms: u64,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub mbid: Option<String>,
    pub mb_type: Option<String>,
    pub mb_date: Option<String>,
    pub cover_thumb_path: Option<String>,
    pub tracks: Vec<TrackInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetail {
    pub id: i64,
    pub name: String,
    pub mbid: Option<String>,
    pub mb_status: String,
    pub mb_verified: bool,
    pub disambiguation: Option<String>,
    pub headshot_path: Option<String>,
    pub releases: Vec<ReleaseInfo>,
}

pub fn artist_detail(conn: &Connection, artist_id: i64) -> rusqlite::Result<Option<ArtistDetail>> {
    let Some(mut detail) = conn
        .query_row(
            "SELECT id, name, mbid, mb_status, mb_verified, disambiguation, headshot_path FROM artists WHERE id = ?1",
            [artist_id],
            |r| {
                Ok(ArtistDetail {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    mbid: r.get(2)?,
                    mb_status: r.get(3)?,
                    mb_verified: r.get(4)?,
                    disambiguation: r.get(5)?,
                    headshot_path: r.get(6)?,
                    releases: Vec::new(),
                })
            },
        )
        .optional()?
    else {
        return Ok(None);
    };

    let mut releases = conn.prepare(
        "SELECT id, title, year, mbid, mb_type, mb_date, NULLIF(cover_thumb_path, '') FROM releases
         WHERE artist_id = ?1 ORDER BY year DESC NULLS LAST, title",
    )?;
    let mut tracks = conn.prepare(
        "SELECT id, title, artist, track_no, disc_no, duration_ms, path FROM tracks
         WHERE release_id = ?1 ORDER BY disc_no NULLS FIRST, track_no NULLS LAST, title",
    )?;
    let rows: Vec<ReleaseInfo> = releases
        .query_map([artist_id], |r| {
            Ok(ReleaseInfo {
                id: r.get(0)?,
                title: r.get(1)?,
                year: r.get(2)?,
                mbid: r.get(3)?,
                mb_type: r.get(4)?,
                mb_date: r.get(5)?,
                cover_thumb_path: r.get(6)?,
                tracks: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    for mut release in rows {
        release.tracks = tracks
            .query_map([release.id], |r| {
                Ok(TrackInfo {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    artist: r.get(2)?,
                    track_no: r.get(3)?,
                    disc_no: r.get(4)?,
                    duration_ms: r.get::<_, i64>(5)? as u64,
                    path: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        detail.releases.push(release);
    }
    Ok(Some(detail))
}

// ---- MusicBrainz enrichment state ----

pub struct PendingArtist {
    pub id: i64,
    pub name: String,
    pub track_titles: Vec<String>,
}

pub fn artists_pending_enrichment(conn: &Connection) -> rusqlite::Result<Vec<PendingArtist>> {
    let mut stmt = conn.prepare("SELECT id, name FROM artists WHERE mb_status IN ('pending', 'error')")?;
    let artists: Vec<(i64, String)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?;
    let mut titles = conn.prepare(
        "SELECT t.title FROM tracks t JOIN releases r ON r.id = t.release_id
         WHERE r.artist_id = ?1 ORDER BY t.id LIMIT 12",
    )?;
    artists
        .into_iter()
        .map(|(id, name)| {
            let track_titles = titles.query_map([id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            Ok(PendingArtist { id, name, track_titles })
        })
        .collect()
}

pub fn set_artist_status(conn: &Connection, id: i64, status: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE artists SET mb_status = ?2 WHERE id = ?1", params![id, status]).map(|_| ())
}

pub fn set_artist_match(
    conn: &Connection,
    id: i64,
    mbid: &str,
    verified: bool,
    sort_name: Option<&str>,
    disambiguation: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE artists SET mbid = ?2, mb_status = 'matched', mb_verified = ?3,
            sort_name = COALESCE(?4, sort_name), disambiguation = ?5 WHERE id = ?1",
        params![id, mbid, verified, sort_name.map(names::normalize), disambiguation],
    )
    .map(|_| ())
}

/// Stores a new headshot path and returns the previous one, so its file can be removed.
pub fn set_artist_headshot(conn: &Connection, id: i64, path: &str) -> rusqlite::Result<Option<String>> {
    let old: Option<String> = conn
        .query_row("SELECT headshot_path FROM artists WHERE id = ?1", [id], |r| r.get(0))
        .optional()?
        .flatten();
    conn.execute("UPDATE artists SET headshot_path = ?2 WHERE id = ?1", params![id, path])?;
    Ok(old.filter(|o| o != path))
}

/// Queues every artist and release for another MusicBrainz pass.
pub fn reset_enrichment(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "UPDATE artists SET mb_status = 'pending';
         UPDATE releases SET mb_status = 'pending';",
    )
}

pub fn pending_releases(conn: &Connection, artist_id: i64) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, title FROM releases WHERE artist_id = ?1 AND mb_status IN ('pending', 'error')",
    )?;
    stmt.query_map([artist_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect()
}

pub fn set_release_match(
    conn: &Connection,
    id: i64,
    mbid: Option<&str>,
    mb_type: Option<&str>,
    mb_date: Option<&str>,
) -> rusqlite::Result<()> {
    let status = if mbid.is_some() { "matched" } else { "unmatched" };
    conn.execute(
        "UPDATE releases SET mbid = ?2, mb_type = ?3, mb_date = ?4, mb_status = ?5 WHERE id = ?1",
        params![id, mbid, mb_type, mb_date, status],
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn
    }

    fn track<'a>(release_id: i64, path: &'a str) -> TrackRow<'a> {
        TrackRow {
            release_id,
            path,
            title: "Song",
            artist: "A",
            track_no: Some(1),
            disc_no: None,
            duration_ms: 1000,
            bitrate: Some(320),
            genre: None,
            size: 10,
            mtime: 1,
            source_hash: Some("h1"),
        }
    }

    #[test]
    fn artists_group_by_normalized_name() {
        let conn = mem();
        let a = upsert_artist(&conn, "Frank Ocean").unwrap();
        let b = upsert_artist(&conn, "frank  ocean").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn release_year_keeps_latest() {
        let conn = mem();
        let a = upsert_artist(&conn, "Kanye West").unwrap();
        let r = upsert_release(&conn, a, "BULLY", Some(2025)).unwrap();
        upsert_release(&conn, a, "bully", Some(2024)).unwrap();
        let year: i32 = conn.query_row("SELECT year FROM releases WHERE id = ?1", [r], |r| r.get(0)).unwrap();
        assert_eq!(year, 2025);
    }

    #[test]
    fn tracks_upsert_by_path_and_prune_cascades() {
        let conn = mem();
        let a = upsert_artist(&conn, "A").unwrap();
        let r = upsert_release(&conn, a, "R", None).unwrap();
        upsert_track(&conn, &track(r, "x.mp3")).unwrap();
        upsert_track(&conn, &TrackRow { size: 20, source_hash: None, ..track(r, "x.mp3") }).unwrap();
        assert_eq!(track_fingerprint(&conn, "x.mp3").unwrap(), Some((20, 1)));
        assert!(has_source_hash(&conn, "h1").unwrap(), "hash survives updates without one");

        delete_track(&conn, "x.mp3").unwrap();
        prune(&conn).unwrap();
        assert!(artist_summaries(&conn).unwrap().is_empty());
    }

    #[test]
    fn summaries_use_newest_cover() {
        let conn = mem();
        let a = upsert_artist(&conn, "A").unwrap();
        let old = upsert_release(&conn, a, "Old", Some(2010)).unwrap();
        let new = upsert_release(&conn, a, "New", Some(2020)).unwrap();
        upsert_track(&conn, &track(old, "1.mp3")).unwrap();
        upsert_track(&conn, &track(new, "2.mp3")).unwrap();
        set_release_cover(&conn, old, "old.jpg", "old-s.jpg", &[1; 15]).unwrap();
        set_release_cover(&conn, new, "new.jpg", "new-s.jpg", &[2; 15]).unwrap();
        let s = &artist_summaries(&conn).unwrap()[0];
        assert_eq!(s.cover_path.as_deref(), Some("new.jpg"));
        assert_eq!(s.palette, vec![2; 15]);
        assert_eq!((s.release_count, s.track_count), (2, 2));
    }

    #[test]
    fn detail_orders_tracks_by_disc_then_number() {
        let conn = mem();
        let a = upsert_artist(&conn, "A").unwrap();
        let r = upsert_release(&conn, a, "R", None).unwrap();
        upsert_track(&conn, &TrackRow { disc_no: Some(2), track_no: Some(1), title: "D2T1", ..track(r, "a") }).unwrap();
        upsert_track(&conn, &TrackRow { disc_no: Some(1), track_no: Some(2), title: "D1T2", ..track(r, "b") }).unwrap();
        upsert_track(&conn, &TrackRow { disc_no: Some(1), track_no: Some(1), title: "D1T1", ..track(r, "c") }).unwrap();
        let d = artist_detail(&conn, a).unwrap().unwrap();
        let titles: Vec<_> = d.releases[0].tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["D1T1", "D1T2", "D2T1"]);
    }
}
