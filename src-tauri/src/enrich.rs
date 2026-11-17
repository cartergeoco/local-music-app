//! Matches local artists and releases to MusicBrainz and fetches artist photos.

use std::path::Path;

use rusqlite::Connection;

use crate::musicbrainz::{self, Client};
use crate::{artwork, db, library, names};

const VERIFY_TITLES: usize = 4;
const HEADSHOT_FETCH_WIDTH: u32 = 640;

/// Processes every artist still pending (or that failed last time) and returns
/// how many failed. `on_update` runs whenever something visible changed.
pub fn enrich_pending(conn: &Connection, client: &Client, headshots_dir: &Path, mut on_update: impl FnMut()) -> usize {
    let pending = match db::artists_pending_enrichment(conn) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("enrichment: {e}");
            return 0;
        }
    };
    let mut failed = 0;
    for artist in pending {
        if let Err(e) = enrich_artist(conn, client, headshots_dir, &artist, &mut on_update) {
            eprintln!("enrichment of {}: {e}", artist.name);
            db::set_artist_status(conn, artist.id, "error").ok();
            failed += 1;
        }
        on_update();
    }
    failed
}

fn verification_titles(titles: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in titles {
        let c = names::clean_title(t);
        if !c.is_empty() && !out.iter().any(|o| names::normalize(o) == names::normalize(&c)) {
            out.push(c);
        }
        if out.len() == VERIFY_TITLES {
            break;
        }
    }
    out
}

fn enrich_artist(
    conn: &Connection,
    client: &Client,
    headshots_dir: &Path,
    artist: &db::PendingArtist,
    on_update: &mut impl FnMut(),
) -> Result<(), String> {
    let err = |e: rusqlite::Error| e.to_string();
    if artist.name == library::UNKNOWN_ARTIST {
        return db::set_artist_status(conn, artist.id, "unmatched").map_err(err);
    }

    let candidates = musicbrainz::name_matches(&artist.name, &client.search_artists(&artist.name)?);
    let titles = verification_titles(&artist.track_titles);

    // Prefer a candidate who actually recorded one of our songs; fall back to an
    // unambiguous exact-name match. Anything else stays unmatched rather than risk
    // showing the wrong person's photo.
    let mut chosen = None;
    'outer: for c in candidates.iter().take(3) {
        for t in &titles {
            if client.recording_exists(&c.id, t)? {
                chosen = Some((c, true));
                break 'outer;
            }
        }
    }
    if chosen.is_none() && candidates.len() == 1 && candidates[0].score == 100 {
        chosen = Some((&candidates[0], false));
    }
    let Some((candidate, verified)) = chosen else {
        eprintln!("no confident match for {} ({} name matches)", artist.name, candidates.len());
        return db::set_artist_status(conn, artist.id, "unmatched").map_err(err);
    };

    let info = client.artist_info(&candidate.id)?;
    db::set_artist_match(
        conn,
        artist.id,
        &candidate.id,
        verified,
        info.sort_name.as_deref(),
        info.disambiguation.as_deref(),
    )
    .map_err(err)?;

    let mut headshot = false;
    if let Some(qid) = &info.wikidata {
        match fetch_headshot(client, qid, artist.id, headshots_dir) {
            Ok(Some(path)) => {
                if let Some(old) = db::set_artist_headshot(conn, artist.id, &path.to_string_lossy()).map_err(err)? {
                    std::fs::remove_file(old).ok();
                }
                headshot = true;
                on_update();
            }
            Ok(None) => {}
            Err(e) => eprintln!("headshot for {}: {e}", artist.name),
        }
    }

    let mut matched_releases = 0;
    let releases = db::pending_releases(conn, artist.id).map_err(err)?;
    let release_count = releases.len();
    for (release_id, title) in releases {
        let group = client.search_release_group(&candidate.id, &title)?;
        matched_releases += group.is_some() as usize;
        db::set_release_match(
            conn,
            release_id,
            group.as_ref().map(|g| g.id.as_str()),
            group.as_ref().and_then(|g| g.primary_type.as_deref()),
            group.as_ref().and_then(|g| g.first_release_date.as_deref()),
        )
        .map_err(err)?;
    }
    eprintln!(
        "enriched {} -> {} ({}), verified={verified}, headshot={headshot}, releases {matched_releases}/{release_count}",
        artist.name, candidate.name, candidate.id
    );
    Ok(())
}

fn fetch_headshot(
    client: &Client,
    qid: &str,
    artist_id: i64,
    dir: &Path,
) -> Result<Option<std::path::PathBuf>, String> {
    let Some(file) = client.wikidata_image(qid)? else { return Ok(None) };
    let bytes = client.commons_image(&file, HEADSHOT_FETCH_WIDTH)?;
    artwork::save_headshot(&bytes, artist_id, dir).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_distinct_clean_titles() {
        let titles: Vec<String> = ["Bad Religion (feat. The Roots)", "Bad Religion", "Srk [Live]", "Pink", "Math"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(verification_titles(&titles), ["Bad Religion", "Srk", "Pink", "Math"]);
    }
}
