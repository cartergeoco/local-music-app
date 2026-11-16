//! Deterministic placeholder artists for the gray-box prototype.

use serde::Serialize;

pub const DEFAULT_COUNT: u32 = 500;
pub const MAX_COUNT: u32 = 20_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ArtistCard {
    pub id: u32,
    pub name: String,
    pub monogram: String,
    /// Five RGB colors, flattened, ordered dark to light.
    pub palette: [u8; 15],
}

const SYLLABLES: &[&str] = &[
    "ka", "lo", "mi", "ra", "ve", "no", "sa", "tu", "zel", "ari", "on", "ly", "dex", "mo", "rin",
    "ta", "vi", "sol", "en", "qua", "bel", "ko", "ny", "ash", "ju", "mar", "el", "fi", "or", "sky",
];
const NOUNS: &[&str] = &[
    "Echo", "Static", "Velvet", "Harbor", "Neon", "Satellite", "Mirage", "Atlas", "Ember", "Tide",
    "Signal", "Orchid", "Cascade", "Prism", "Monarch", "Drift", "Voltage", "Saffron", "Lumen",
    "Crescent",
];
const SUFFIXES: &[&str] = &["Club", "Society", "Collective", "Union", "Theory", "Machine"];

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

pub fn generate(count: u32) -> Vec<ArtistCard> {
    let count = count.min(MAX_COUNT);
    let mut rng = Rng(0x5EED_CAFE_F00D_1234);
    (0..count)
        .map(|id| {
            let name = artist_name(&mut rng);
            let monogram = monogram(&name);
            let palette = palette(&mut rng);
            ArtistCard { id, name, monogram, palette }
        })
        .collect()
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

fn word(rng: &mut Rng) -> String {
    let n = 2 + rng.below(2);
    let raw: String = (0..n).map(|_| SYLLABLES[rng.below(SYLLABLES.len())]).collect();
    capitalize(&raw)
}

fn artist_name(rng: &mut Rng) -> String {
    match rng.below(5) {
        0 => format!("{} {}", word(rng), word(rng)),
        1 => format!("The {}s", NOUNS[rng.below(NOUNS.len())]),
        2 => format!(
            "{} {}",
            NOUNS[rng.below(NOUNS.len())],
            SUFFIXES[rng.below(SUFFIXES.len())]
        ),
        3 => word(rng).to_uppercase(),
        _ => word(rng),
    }
}

pub fn monogram(name: &str) -> String {
    let words: Vec<&str> = name
        .split_whitespace()
        .filter(|w| !w.eq_ignore_ascii_case("the"))
        .collect();
    let initials: String = match words.as_slice() {
        [] => String::new(),
        [only] => only.chars().take(2).collect(),
        [first, .., last] => first.chars().take(1).chain(last.chars().take(1)).collect(),
    };
    initials.to_uppercase()
}

fn palette(rng: &mut Rng) -> [u8; 15] {
    let base = rng.unit() * 360.0;
    let spread = 20.0 + rng.unit() * 140.0;
    let sat = 0.45 + rng.unit() * 0.4;
    // Hue offsets and lightness ramp from deep shadow to highlight.
    let stops = [
        (0.0, 0.10),
        (spread * 0.35, 0.26),
        (spread * 0.7, 0.44),
        (spread, 0.60),
        (spread * 1.25, 0.78),
    ];
    let mut out = [0u8; 15];
    for (i, (offset, light)) in stops.iter().enumerate() {
        let [r, g, b] = hsl_to_rgb((base + offset) % 360.0, sat, *light);
        out[i * 3] = r;
        out[i * 3 + 1] = g;
        out[i * 3 + 2] = b;
    }
    out
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to_u8 = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    [to_u8(r), to_u8(g), to_u8(b)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_requested_count_with_sequential_ids() {
        let artists = generate(500);
        assert_eq!(artists.len(), 500);
        assert!(artists.iter().enumerate().all(|(i, a)| a.id == i as u32));
    }

    #[test]
    fn is_deterministic() {
        assert_eq!(generate(50), generate(50));
    }

    #[test]
    fn caps_count() {
        assert_eq!(generate(MAX_COUNT + 10).len(), MAX_COUNT as usize);
    }

    #[test]
    fn names_and_monograms_are_non_empty() {
        for a in generate(500) {
            assert!(!a.name.trim().is_empty());
            assert!(!a.monogram.is_empty() && a.monogram.chars().count() <= 2);
        }
    }

    #[test]
    fn monogram_rules() {
        assert_eq!(monogram("The Echos"), "EC");
        assert_eq!(monogram("Neon Society"), "NS");
        assert_eq!(monogram("Kalo Vemi Ra"), "KR");
        assert_eq!(monogram("SOL"), "SO");
    }

    #[test]
    fn palette_runs_dark_to_light() {
        let luma = |p: &[u8; 15], i: usize| {
            0.2126 * p[i * 3] as f32 + 0.7152 * p[i * 3 + 1] as f32 + 0.0722 * p[i * 3 + 2] as f32
        };
        for a in generate(200) {
            assert!(luma(&a.palette, 0) < luma(&a.palette, 4));
        }
    }
}
