//! Resizing artwork into small cached JPEGs and extracting color palettes.

use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};

/// Card backgrounds: covers a ~700px-tall card at 1x with a little headroom.
pub const COVER_SIZE: u32 = 768;
pub const THUMB_SIZE: u32 = 160;
pub const HEADSHOT_SIZE: u32 = 320;
const JPEG_QUALITY: u8 = 85;

pub struct SavedCover {
    pub cover: PathBuf,
    pub thumb: PathBuf,
    pub palette: [u8; 15],
}

fn short_hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex()[..10].to_owned()
}

fn save_jpeg(img: &RgbImage, path: &Path) -> Result<(), String> {
    let mut out = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
        .encode_image(img)
        .map_err(|e| e.to_string())
}

/// Scales so the longer side is at most `max`, never upscaling.
fn fit(img: &DynamicImage, max: u32) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    if w <= max && h <= max {
        return img.to_rgb8();
    }
    img.resize(max, max, FilterType::Triangle).to_rgb8()
}

/// Square crop biased toward the top, where faces usually are in portraits.
fn square_portrait(img: &DynamicImage, size: u32) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = ((h - side) as f32 * 0.25) as u32;
    img.crop_imm(x, y, side, side).resize_exact(size, size, FilterType::Triangle).to_rgb8()
}

/// Names files by content hash so the webview never shows a stale cached image.
pub fn save_cover(bytes: &[u8], release_id: i64, dir: &Path) -> Result<SavedCover, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tag = short_hash(bytes);
    let cover = dir.join(format!("{release_id}-{tag}.jpg"));
    let thumb = dir.join(format!("{release_id}-{tag}-s.jpg"));
    let large = fit(&img, COVER_SIZE);
    save_jpeg(&large, &cover)?;
    let small = image::imageops::resize(&large, THUMB_SIZE, THUMB_SIZE, FilterType::Triangle);
    save_jpeg(&small, &thumb)?;
    Ok(SavedCover { cover, thumb, palette: palette(&small) })
}

pub fn save_headshot(bytes: &[u8], artist_id: i64, dir: &Path) -> Result<PathBuf, String> {
    let img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{artist_id}-{}.jpg", short_hash(bytes)));
    save_jpeg(&square_portrait(&img, HEADSHOT_SIZE), &path)?;
    Ok(path)
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Five representative colors ordered dark to light, via k-means on a downsampled image.
pub fn palette(img: &RgbImage) -> [u8; 15] {
    const K: usize = 5;
    let small = image::imageops::resize(img, 48, 48, FilterType::Triangle);
    let pixels: Vec<[f32; 3]> = small
        .pixels()
        .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
        .collect();

    // Seed centers across the brightness range for stable, spread-out results.
    let mut sorted = pixels.clone();
    sorted.sort_by(|a, b| luma(*a).total_cmp(&luma(*b)));
    let mut centers: Vec<[f32; 3]> =
        (0..K).map(|i| sorted[(i * 2 + 1) * sorted.len() / (K * 2)]).collect();

    let mut assignment = vec![0usize; pixels.len()];
    for _ in 0..12 {
        for (i, p) in pixels.iter().enumerate() {
            assignment[i] = (0..K)
                .min_by(|&a, &b| dist(p, &centers[a]).total_cmp(&dist(p, &centers[b])))
                .unwrap_or(0);
        }
        let mut sums = [[0f32; 4]; K];
        for (p, &k) in pixels.iter().zip(&assignment) {
            sums[k][0] += p[0];
            sums[k][1] += p[1];
            sums[k][2] += p[2];
            sums[k][3] += 1.0;
        }
        for k in 0..K {
            if sums[k][3] > 0.0 {
                centers[k] = [sums[k][0] / sums[k][3], sums[k][1] / sums[k][3], sums[k][2] / sums[k][3]];
            }
        }
    }

    centers.sort_by(|a, b| luma(*a).total_cmp(&luma(*b)));
    let mut out = [0u8; 15];
    for (i, c) in centers.iter().enumerate() {
        for ch in 0..3 {
            out[i * 3 + ch] = c[ch].round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

fn dist(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_tone() -> RgbImage {
        RgbImage::from_fn(64, 64, |x, _| {
            if x < 32 { image::Rgb([200, 30, 30]) } else { image::Rgb([20, 20, 120]) }
        })
    }

    #[test]
    fn palette_is_ordered_dark_to_light_and_finds_dominant_colors() {
        let p = palette(&two_tone());
        let l = |i: usize| luma([p[i * 3] as f32, p[i * 3 + 1] as f32, p[i * 3 + 2] as f32]);
        for i in 0..4 {
            assert!(l(i) <= l(i + 1));
        }
        let has = |r: u8, b: u8| (0..5).any(|i| p[i * 3].abs_diff(r) < 30 && p[i * 3 + 2].abs_diff(b) < 30);
        assert!(has(200, 30) && has(20, 120));
    }

    #[test]
    fn saves_cover_and_thumb() {
        let dir = std::env::temp_dir().join(format!("lm-art-{}", std::process::id()));
        let mut png = Vec::new();
        DynamicImage::ImageRgb8(RgbImage::from_pixel(1200, 1200, image::Rgb([90, 140, 60])))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let saved = save_cover(&png, 7, &dir).unwrap();
        let cover = image::open(&saved.cover).unwrap();
        let thumb = image::open(&saved.thumb).unwrap();
        assert_eq!((cover.width(), cover.height()), (COVER_SIZE, COVER_SIZE));
        assert_eq!(thumb.width(), THUMB_SIZE);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn crops_portraits_square() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(400, 900));
        let out = square_portrait(&img, HEADSHOT_SIZE);
        assert_eq!((out.width(), out.height()), (HEADSHOT_SIZE, HEADSHOT_SIZE));
    }
}
