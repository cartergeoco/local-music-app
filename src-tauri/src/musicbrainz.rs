//! MusicBrainz lookups plus artist photos from Wikidata / Wikimedia Commons.
//! All reads are anonymous; MusicBrainz only asks for an identifying User-Agent
//! and at most one request per second.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::names;

const USER_AGENT: &str = concat!(
    "LocalMusic/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/cartergeoco/local-music-app )"
);
const MB_BASE: &str = "https://musicbrainz.org/ws/2";
const MB_SPACING: Duration = Duration::from_millis(1100);
const MB_ATTEMPTS: u32 = 4;
const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;

pub struct Client {
    agent: ureq::Agent,
    last_mb: Mutex<Option<Instant>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistCandidate {
    pub id: String,
    pub name: String,
    pub score: u32,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ArtistInfo {
    pub sort_name: Option<String>,
    pub disambiguation: Option<String>,
    pub wikidata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReleaseGroup {
    pub id: String,
    pub primary_type: Option<String>,
    pub first_release_date: Option<String>,
}

/// Escapes a value for use inside a quoted Lucene phrase.
pub fn lucene_phrase(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

impl Client {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .build()
            .into();
        Self { agent, last_mb: Mutex::new(None) }
    }

    fn throttle(&self) {
        let mut last = self.last_mb.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = *last {
            let wait = MB_SPACING.saturating_sub(t.elapsed());
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        }
        *last = Some(Instant::now());
    }

    fn mb(&self, path: &str, query: &[(&str, &str)]) -> Result<Value, String> {
        for attempt in 0..MB_ATTEMPTS {
            self.throttle();
            let mut req = self.agent.get(format!("{MB_BASE}/{path}")).query("fmt", "json");
            for (k, v) in query {
                req = req.query(*k, *v);
            }
            let res = req.call().map_err(|e| e.to_string())?;
            let status = res.status().as_u16();
            if status == 503 || status == 429 {
                let retry_after = res
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok()?.trim().parse::<u64>().ok());
                let wait = retry_after.unwrap_or(3 << attempt).min(60);
                if attempt + 1 < MB_ATTEMPTS {
                    std::thread::sleep(Duration::from_secs(wait));
                }
                continue;
            }
            return checked(res)?.body_mut().read_json::<Value>().map_err(|e| e.to_string());
        }
        Err("MusicBrainz is busy (503)".to_owned())
    }

    /// Searches names and aliases, since artists get renamed (Kanye West is now "Ye").
    pub fn search_artists(&self, name: &str) -> Result<Vec<ArtistCandidate>, String> {
        let phrase = lucene_phrase(name);
        let q = format!("artist:{phrase} OR alias:{phrase}");
        let v = self.mb("artist", &[("query", &q), ("limit", "8")])?;
        Ok(v["artists"]
            .as_array()
            .map(|a| a.iter().filter_map(parse_candidate).collect())
            .unwrap_or_default())
    }

    /// Whether the artist is credited on a recording with this title. Search hit
    /// counts are fuzzy, so the returned recordings themselves are checked.
    pub fn recording_exists(&self, artist_mbid: &str, title: &str) -> Result<bool, String> {
        let q = format!("recording:{} AND arid:{artist_mbid}", lucene_phrase(title));
        let v = self.mb("recording", &[("query", &q), ("limit", "10")])?;
        Ok(has_credited_recording(&v, artist_mbid, title))
    }

    pub fn artist_info(&self, mbid: &str) -> Result<ArtistInfo, String> {
        let v = self.mb(&format!("artist/{mbid}"), &[("inc", "url-rels")])?;
        let wikidata = v["relations"].as_array().and_then(|rels| {
            rels.iter().find(|r| r["type"] == "wikidata").and_then(|r| {
                r["url"]["resource"].as_str()?.rsplit('/').next().map(str::to_owned)
            })
        });
        let text = |k: &str| v[k].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
        Ok(ArtistInfo { sort_name: text("sort-name"), disambiguation: text("disambiguation"), wikidata })
    }

    pub fn search_release_group(&self, artist_mbid: &str, title: &str) -> Result<Option<ReleaseGroup>, String> {
        let q = format!("releasegroup:{} AND arid:{artist_mbid}", lucene_phrase(title));
        let v = self.mb("release-group", &[("query", &q), ("limit", "5")])?;
        let want = names::normalize(title);
        Ok(v["release-groups"].as_array().and_then(|groups| {
            groups
                .iter()
                .filter(|g| g["score"].as_u64().unwrap_or(0) >= 90)
                .find(|g| g["title"].as_str().is_some_and(|t| names::normalize(t) == want))
                .map(|g| ReleaseGroup {
                    id: g["id"].as_str().unwrap_or_default().to_owned(),
                    primary_type: g["primary-type"].as_str().map(str::to_owned),
                    first_release_date: g["first-release-date"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                })
        }))
    }

    /// File name of the Wikidata entity's main image (property P18).
    pub fn wikidata_image(&self, qid: &str) -> Result<Option<String>, String> {
        let res = self
            .agent
            .get("https://www.wikidata.org/w/api.php")
            .query("action", "wbgetclaims")
            .query("entity", qid)
            .query("property", "P18")
            .query("format", "json")
            .call()
            .map_err(|e| e.to_string())?;
        let v: Value = checked(res)?.body_mut().read_json().map_err(|e| e.to_string())?;
        Ok(v["claims"]["P18"][0]["mainsnak"]["datavalue"]["value"].as_str().map(str::to_owned))
    }

    /// Downloads a Commons image scaled to `width` pixels.
    pub fn commons_image(&self, file_name: &str, width: u32) -> Result<Vec<u8>, String> {
        let res = self
            .agent
            .get("https://commons.wikimedia.org/w/index.php")
            .query("title", &format!("Special:Redirect/file/{}", file_name.replace(' ', "_")))
            .query("width", &width.to_string())
            .call()
            .map_err(|e| e.to_string())?;
        checked(res)?
            .body_mut()
            .with_config()
            .limit(MAX_IMAGE_BYTES)
            .read_to_vec()
            .map_err(|e| e.to_string())
    }
}

type Response = ureq::http::Response<ureq::Body>;

fn checked(res: Response) -> Result<Response, String> {
    let status = res.status();
    if status.is_success() { Ok(res) } else { Err(format!("http status {}", status.as_u16())) }
}

fn parse_candidate(v: &Value) -> Option<ArtistCandidate> {
    Some(ArtistCandidate {
        id: v["id"].as_str()?.to_owned(),
        name: v["name"].as_str()?.to_owned(),
        score: v["score"].as_u64().unwrap_or(0) as u32,
        aliases: v["aliases"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["name"].as_str().map(str::to_owned)).collect())
            .unwrap_or_default(),
    })
}

fn has_credited_recording(v: &Value, artist_mbid: &str, title: &str) -> bool {
    let want = names::normalize(&names::clean_title(title));
    v["recordings"].as_array().is_some_and(|recs| {
        recs.iter().any(|r| {
            let same_title = r["title"]
                .as_str()
                .is_some_and(|t| names::normalize(&names::clean_title(t)) == want);
            let credited = r["artist-credit"]
                .as_array()
                .is_some_and(|ac| ac.iter().any(|c| c["artist"]["id"] == artist_mbid));
            same_title && credited
        })
    })
}

/// Candidates whose name or an alias matches ours exactly (ignoring case and spacing).
pub fn name_matches(name: &str, candidates: &[ArtistCandidate]) -> Vec<ArtistCandidate> {
    let want = names::normalize(name);
    candidates
        .iter()
        .filter(|c| c.score >= 90)
        .filter(|c| {
            names::normalize(&c.name) == want || c.aliases.iter().any(|a| names::normalize(a) == want)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, name: &str, score: u32, aliases: &[&str]) -> ArtistCandidate {
        ArtistCandidate {
            id: id.into(),
            name: name.into(),
            score,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn escapes_lucene_phrases() {
        assert_eq!(lucene_phrase("Say \"Hi\""), "\"Say \\\"Hi\\\"\"");
        assert_eq!(lucene_phrase("#myhealth"), "\"#myhealth\"");
    }

    #[test]
    fn matches_names_and_aliases_only() {
        let c = vec![
            cand("1", "Kanye West", 100, &["Ye"]),
            cand("2", "Kanye West Tribute", 95, &[]),
            cand("3", "Ye", 60, &[]),
        ];
        assert_eq!(name_matches("kanye  west", &c).len(), 1);
        assert_eq!(name_matches("Ye", &c)[0].id, "1");
    }

    #[test]
    fn verifies_recordings_by_credit_and_title() {
        let v: Value = serde_json::from_str(
            r#"{"count": 3, "recordings": [
                {"title": "Preacher Man", "artist-credit": [{"artist": {"id": "other"}}]},
                {"title": "PREACHER MAN (Live)", "artist-credit": [{"artist": {"id": "other"}}, {"artist": {"id": "ye"}}]}
            ]}"#,
        )
        .unwrap();
        assert!(has_credited_recording(&v, "ye", "PREACHER MAN"));
        assert!(!has_credited_recording(&v, "ye", "BULLY"));
        assert!(!has_credited_recording(&v, "nobody", "PREACHER MAN"));
        assert!(!has_credited_recording(&serde_json::json!({"count": 7}), "ye", "PREACHER MAN"));
    }

    #[test]
    fn parses_search_results() {
        let v: Value = serde_json::from_str(
            r#"{"id":"abc","name":"Frank Ocean","score":100,"aliases":[{"name":"Christopher Breaux"}]}"#,
        )
        .unwrap();
        let c = parse_candidate(&v).unwrap();
        assert_eq!(c, cand("abc", "Frank Ocean", 100, &["Christopher Breaux"]));
    }
}
