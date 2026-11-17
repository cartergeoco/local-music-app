//! Reading and writing audio file tags.
//!
//! MP3 tags go through the `id3` crate, which keeps frames it can't decode as
//! raw bytes; lofty rejects whole files over a single malformed frame, which
//! is common in downloaded music. Lofty still handles audio properties and
//! every other format.

use std::path::Path;

use id3::TagLike;
use lofty::config::{ParseOptions, WriteOptions};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};

pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "aac", "wav", "ogg", "opus"];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

fn is_mp3(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub disc_total: Option<u32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub duration_ms: u64,
    pub bitrate_kbps: Option<u32>,
}

pub struct Cover {
    pub data: Vec<u8>,
}

fn clean(s: Option<&str>) -> Option<String> {
    s.map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned)
}

fn positive<T: Into<i64> + Copy>(v: Option<T>) -> Option<T> {
    v.filter(|&x| x.into() > 0)
}

fn fill<T>(slot: &mut Option<T>, value: impl FnOnce() -> Option<T>) {
    if slot.is_none() {
        *slot = value();
    }
}

/// Reads tags and audio properties. Cover art is only decoded when `with_cover` is set.
pub fn read(path: &Path, with_cover: bool) -> Result<(TrackTags, Option<Cover>), String> {
    let mp3 = is_mp3(path);
    let file = Probe::open(path)
        .map_err(|e| e.to_string())?
        .options(ParseOptions::new().read_tags(!mp3).read_cover_art(with_cover && !mp3))
        .read()
        .map_err(|e| e.to_string())?;

    let props = file.properties();
    let mut tags = TrackTags {
        duration_ms: props.duration().as_millis() as u64,
        bitrate_kbps: props.audio_bitrate(),
        ..Default::default()
    };
    let cover = if mp3 {
        read_id3(path, with_cover, &mut tags)?
    } else {
        file.primary_tag()
            .or_else(|| file.first_tag())
            .and_then(|tag| read_lofty(tag, with_cover, &mut tags))
    };
    Ok((tags, cover))
}

/// ID3v2 first, then any ID3v1 tag for fields the former lacks.
fn read_id3(path: &Path, with_cover: bool, tags: &mut TrackTags) -> Result<Option<Cover>, String> {
    let v2 = id3::no_tag_ok(id3::partial_tag_ok(id3::Tag::read_from_path(path))).map_err(|e| e.to_string())?;
    let v1 = id3::v1::Tag::read_from_path(path).ok().map(id3::Tag::from);
    for tag in [v2.as_ref(), v1.as_ref()].into_iter().flatten() {
        fill(&mut tags.title, || clean(tag.title()));
        fill(&mut tags.artist, || clean(tag.artist()));
        fill(&mut tags.album, || clean(tag.album()));
        fill(&mut tags.album_artist, || clean(tag.album_artist()));
        fill(&mut tags.genre, || tag.genre_parsed().and_then(|g| clean(Some(&g))));
        fill(&mut tags.track, || positive(tag.track()));
        fill(&mut tags.disc, || positive(tag.disc()));
        fill(&mut tags.disc_total, || positive(tag.total_discs()));
        fill(&mut tags.year, || {
            positive(tag.year().or_else(|| tag.date_recorded().or_else(|| tag.date_released()).map(|d| d.year)))
        });
    }
    if !with_cover {
        return Ok(None);
    }
    let pictures: Vec<_> = v2.iter().flat_map(|t| t.pictures()).collect();
    Ok(pictures
        .iter()
        .find(|p| p.picture_type == id3::frame::PictureType::CoverFront)
        .or_else(|| pictures.iter().max_by_key(|p| p.data.len()))
        .map(|p| Cover { data: p.data.clone() }))
}

fn read_lofty(tag: &Tag, with_cover: bool, tags: &mut TrackTags) -> Option<Cover> {
    tags.title = clean(tag.title().as_deref());
    tags.artist = clean(tag.artist().as_deref());
    tags.album = clean(tag.album().as_deref());
    tags.genre = clean(tag.genre().as_deref());
    tags.album_artist = clean(tag.get_string(ItemKey::AlbumArtist));
    tags.track = positive(tag.track());
    tags.disc = positive(tag.disk());
    tags.disc_total = positive(tag.disk_total());
    tags.year = positive(tag.date().map(|d| d.year as i32));
    if !with_cover {
        return None;
    }
    let pictures = tag.pictures();
    pictures
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.iter().max_by_key(|p| p.data().len()))
        .map(|p| Cover { data: p.data().to_vec() })
}

/// Fields to change in a file; `None` leaves a field untouched.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TagEdits {
    pub title: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub disc_total: Option<u32>,
}

impl TagEdits {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Applies edits in place. Pictures and every other frame are preserved; a tag
/// that only partially parses is never rewritten, since that would drop frames.
pub fn write(path: &Path, edits: &TagEdits) -> Result<(), String> {
    if edits.is_empty() {
        return Ok(());
    }
    if is_mp3(path) {
        write_id3(path, edits)
    } else {
        write_lofty(path, edits)
    }
}

fn write_id3(path: &Path, edits: &TagEdits) -> Result<(), String> {
    let mut tag = match id3::Tag::read_from_path(path) {
        Ok(tag) => tag,
        Err(id3::Error { kind: id3::ErrorKind::NoTag, .. }) => id3::Tag::new(),
        Err(e) => return Err(format!("tag not fully readable, left unchanged: {e}")),
    };
    if let Some(v) = &edits.title {
        tag.set_title(v.clone());
    }
    if let Some(v) = &edits.album_artist {
        tag.set_album_artist(v.clone());
    }
    if let Some(v) = edits.track {
        tag.set_track(v);
    }
    if let Some(v) = edits.disc {
        tag.set_disc(v);
    }
    if let Some(v) = edits.disc_total {
        tag.set_total_discs(v);
    }
    let version = tag.version();
    tag.write_to_path(path, version).map_err(|e| e.to_string())
}

fn write_lofty(path: &Path, edits: &TagEdits) -> Result<(), String> {
    let mut file = Probe::open(path)
        .map_err(|e| e.to_string())?
        .read()
        .map_err(|e| e.to_string())?;
    if file.primary_tag().is_none() {
        let tag_type = file.primary_tag_type();
        file.insert_tag(Tag::new(tag_type));
    }
    let tag = file.primary_tag_mut().ok_or("file has no writable tag")?;
    if let Some(v) = &edits.title {
        tag.set_title(v.clone());
    }
    if let Some(v) = &edits.album_artist {
        tag.insert_text(ItemKey::AlbumArtist, v.clone());
    }
    if let Some(v) = edits.track {
        tag.set_track(v);
    }
    if let Some(v) = edits.disc {
        tag.set_disk(v);
    }
    if let Some(v) = edits.disc_total {
        tag.set_disk_total(v);
    }
    file.save_to_path(path, WriteOptions::default()).map_err(|e| e.to_string())
}
