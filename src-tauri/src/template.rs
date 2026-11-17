//! Library path template:
//! `{AlbumArtist}/{Year} - {Album}/{Disc}{Track:02} {Title}.{ext}`

use std::path::PathBuf;

pub const MAX_PATH_CHARS: usize = 240;
const MIN_SEGMENT_CHARS: usize = 16;

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[derive(Debug, Clone)]
pub struct PathFields<'a> {
    pub album_artist: &'a str,
    pub album: &'a str,
    pub year: Option<i32>,
    pub disc: Option<u32>,
    /// Digits to pad the disc number to; 0 means a single-disc release (no disc prefix).
    pub disc_width: usize,
    pub track: Option<u32>,
    pub title: &'a str,
    pub ext: &'a str,
}

/// Makes one path segment valid on Windows.
pub fn sanitize_segment(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = s.trim_start().trim_end_matches(['.', ' ']);
    s = trimmed.to_owned();
    if s.is_empty() {
        s.push('_');
    }
    let stem = s.split('.').next().unwrap_or("");
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        s.insert(stem.len(), '_');
    }
    s
}

fn truncate_chars(s: &mut String, max: usize) {
    if let Some((i, _)) = s.char_indices().nth(max) {
        s.truncate(i);
        let t = s.trim_end_matches(['.', ' ']).len();
        s.truncate(t);
    }
}

/// Renders the library-relative path, shortening segments (title first) so that
/// `root_chars` plus the result stays within [`MAX_PATH_CHARS`].
pub fn render(fields: &PathFields, root_chars: usize) -> PathBuf {
    let artist = sanitize_segment(fields.album_artist);
    let album = sanitize_segment(&match fields.year {
        Some(y) if y > 0 => format!("{y} - {}", fields.album),
        _ => fields.album.to_owned(),
    });
    let prefix = match (fields.disc, fields.track) {
        (Some(d), Some(t)) if fields.disc_width > 0 => format!("{d:0w$}-{t:02} ", w = fields.disc_width),
        (_, Some(t)) => format!("{t:02} "),
        _ => String::new(),
    };
    let ext = sanitize_segment(&fields.ext.to_lowercase());
    let title = sanitize_segment(fields.title);

    // [artist, album, title]; shortened in the order title, album, artist.
    let mut segs = [artist, album, title];
    let fixed = root_chars + 3 + prefix.chars().count() + 1 + ext.chars().count();
    for idx in [2, 1, 0] {
        let total = fixed + segs.iter().map(|s| s.chars().count()).sum::<usize>();
        let over = total.saturating_sub(MAX_PATH_CHARS);
        if over == 0 {
            break;
        }
        let keep = segs[idx].chars().count().saturating_sub(over).max(MIN_SEGMENT_CHARS);
        truncate_chars(&mut segs[idx], keep);
    }

    let [artist, album, title] = segs;
    let file = format!("{prefix}{title}.{ext}");
    [artist, album, file].iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn fields<'a>() -> PathFields<'a> {
        PathFields {
            album_artist: "Frank Ocean",
            album: "bear WITNESS",
            year: Some(2021),
            disc: Some(1),
            disc_width: 0,
            track: Some(3),
            title: "Bad Religion",
            ext: "MP3",
        }
    }

    #[test]
    fn renders_default_template() {
        assert_eq!(
            render(&fields(), 30),
            Path::new("Frank Ocean/2021 - bear WITNESS/03 Bad Religion.mp3")
        );
    }

    #[test]
    fn adds_padded_disc_prefix_for_multi_disc() {
        let f = PathFields { disc: Some(3), disc_width: 2, ..fields() };
        assert_eq!(
            render(&f, 30),
            Path::new("Frank Ocean/2021 - bear WITNESS/03-03 Bad Religion.mp3")
        );
    }

    #[test]
    fn omits_missing_year_and_track() {
        let f = PathFields { year: None, track: None, ..fields() };
        assert_eq!(render(&f, 30), Path::new("Frank Ocean/bear WITNESS/Bad Religion.mp3"));
    }

    #[test]
    fn replaces_invalid_characters() {
        assert_eq!(sanitize_segment("AC/DC: Live?"), "AC_DC_ Live_");
        assert_eq!(sanitize_segment("Girlfriend's \"Best\" Friend"), "Girlfriend's _Best_ Friend");
        assert_eq!(sanitize_segment("tab\there"), "tab_here");
    }

    #[test]
    fn strips_trailing_dots_and_spaces() {
        assert_eq!(sanitize_segment("Mr. Jones... "), "Mr. Jones");
        assert_eq!(sanitize_segment(" ..."), "_");
    }

    #[test]
    fn suffixes_reserved_names() {
        assert_eq!(sanitize_segment("CON"), "CON_");
        assert_eq!(sanitize_segment("nul.txt"), "nul_.txt");
        assert_eq!(sanitize_segment("Console"), "Console");
    }

    #[test]
    fn truncates_title_first_for_long_paths() {
        let long = "x".repeat(300);
        let f = PathFields { title: &long, ..fields() };
        let p = render(&f, 40);
        assert!(40 + 1 + p.to_string_lossy().chars().count() <= MAX_PATH_CHARS);
        assert!(p.starts_with("Frank Ocean/2021 - bear WITNESS"));
    }

    #[test]
    fn truncates_other_segments_when_title_is_not_enough() {
        let long = "y".repeat(300);
        let f = PathFields { album_artist: &long, album: &long, title: &long, ..fields() };
        let p = render(&f, 40);
        assert!(40 + 1 + p.to_string_lossy().chars().count() <= MAX_PATH_CHARS);
    }
}
