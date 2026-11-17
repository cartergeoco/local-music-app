//! Text rules for artist names, titles, and file names.

/// Artist names that contain a separator we would otherwise split on.
const COMMA_NAMES: &[&str] = &[
    "tyler, the creator",
    "earth, wind & fire",
    "crosby, stills, nash & young",
    "crosby, stills & nash",
];

const FEATURE_MARKERS: &[&str] = &[" feat. ", " feat ", " ft. ", " ft ", " featuring ", " with "];

/// Case-folded, whitespace-collapsed form used to group names that differ only cosmetically.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for word in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        for c in word.chars() {
            match c {
                '\u{2018}' | '\u{2019}' | '`' => out.push('\''),
                '\u{201C}' | '\u{201D}' => out.push('"'),
                _ => out.extend(c.to_lowercase()),
            }
        }
    }
    out
}

/// The lead artist of a credit such as "Daniel Caesar, Ellie Goulding" or "A feat. B".
pub fn primary_artist(credit: &str) -> &str {
    let credit = credit.trim();
    let lower = credit.to_lowercase();
    for name in COMMA_NAMES {
        if lower.starts_with(name) {
            return &credit[..name.len()];
        }
    }
    let mut end = credit.len();
    for sep in [", ", "; ", " / "].iter().chain(FEATURE_MARKERS) {
        if let Some(i) = lower.find(sep) {
            end = end.min(i);
        }
    }
    credit[..end].trim()
}

/// Title without featured-artist credits or bracketed qualifiers, for fuzzy matching.
pub fn clean_title(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut depth = 0usize;
    for c in title.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let lower = out.to_lowercase();
    let cut = FEATURE_MARKERS
        .iter()
        .filter_map(|m| lower.find(m))
        .min()
        .unwrap_or(out.len());
    out[..cut].split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn monogram(name: &str) -> String {
    let words: Vec<&str> = name
        .split_whitespace()
        .filter(|w| !w.eq_ignore_ascii_case("the"))
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .collect();
    let initials: String = match words.as_slice() {
        [] => String::new(),
        [only] => only.chars().filter(|c| c.is_alphanumeric()).take(2).collect(),
        [first, .., last] => {
            let pick = |w: &str| w.chars().find(|c| c.is_alphanumeric());
            pick(first).into_iter().chain(pick(last)).collect()
        }
    };
    initials.to_uppercase()
}

/// Sort key that ignores a leading "The".
pub fn sort_name(name: &str) -> String {
    let n = normalize(name);
    n.strip_prefix("the ").map(str::to_owned).unwrap_or(n)
}

/// Leading track number in a file stem such as "07. Title" or "1003 - Title".
pub fn leading_number(stem: &str) -> Option<u32> {
    let digits: String = stem.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let rest = &stem[digits.len()..];
    if rest.starts_with('.') || rest.starts_with(" - ") || rest.starts_with(' ') || rest.starts_with('_') {
        digits.parse().ok()
    } else {
        None
    }
}

/// Best-effort title from a file stem: drops a leading number and an "Artist - " prefix.
pub fn title_from_stem(stem: &str) -> String {
    let mut s = stem.trim();
    if leading_number(s).is_some() {
        s = s.trim_start_matches(|c: char| c.is_ascii_digit());
        s = s.trim_start_matches(['.', '-', '_', ' ']);
    }
    if let Some((_, after)) = s.split_once(" - ") {
        s = after;
    }
    let s = s.trim();
    if s.is_empty() { stem.trim().to_owned() } else { s.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_space_and_quotes() {
        assert_eq!(normalize("  Can\u{2019}t   Stop "), "can't stop");
        assert_eq!(normalize("GIVĒON"), "givēon");
    }

    #[test]
    fn finds_primary_artist() {
        assert_eq!(primary_artist("Daniel Caesar, Ellie Goulding"), "Daniel Caesar");
        assert_eq!(primary_artist("Kanye West, Miley Cyrus, Travis Scott"), "Kanye West");
        assert_eq!(primary_artist("Skyte feat. Lil Tecca"), "Skyte");
        assert_eq!(primary_artist("Tyler, The Creator, Frank Ocean"), "Tyler, The Creator");
        assert_eq!(primary_artist("Simon & Garfunkel"), "Simon & Garfunkel");
        assert_eq!(primary_artist("  Frank Ocean "), "Frank Ocean");
    }

    #[test]
    fn cleans_titles() {
        assert_eq!(clean_title("Bad Religion (feat. The Roots)"), "Bad Religion");
        assert_eq!(clean_title("Srk [Bowery Ballroom, Ny]"), "Srk");
        assert_eq!(clean_title("Let Me ft. Kyle Christopher"), "Let Me");
        assert_eq!(clean_title("Nights"), "Nights");
    }

    #[test]
    fn monograms() {
        assert_eq!(monogram("The Echos"), "EC");
        assert_eq!(monogram("Daniel Caesar"), "DC");
        assert_eq!(monogram("GIVĒON"), "GI");
        assert_eq!(monogram("#myhealth"), "MY");
        assert_eq!(monogram(""), "");
    }

    #[test]
    fn sort_names_ignore_leading_the() {
        assert_eq!(sort_name("The Weeknd"), "weeknd");
        assert_eq!(sort_name("Theo"), "theo");
    }

    #[test]
    fn parses_leading_numbers() {
        assert_eq!(leading_number("07. GHOST TOWN"), Some(7));
        assert_eq!(leading_number("1003. Bad Religion"), Some(1003));
        assert_eq!(leading_number("01 - Intro"), Some(1));
        assert_eq!(leading_number("48"), None);
        assert_eq!(leading_number("2Pac - Song"), None);
        assert_eq!(leading_number("Daniel Caesar - Provider"), None);
    }

    #[test]
    fn derives_titles_from_stems() {
        assert_eq!(title_from_stem("301. Life Goes On (ft. Blue Whale)"), "Life Goes On (ft. Blue Whale)");
        assert_eq!(title_from_stem("Daniel Caesar - Provider"), "Provider");
        assert_eq!(title_from_stem("Song"), "Song");
    }
}
