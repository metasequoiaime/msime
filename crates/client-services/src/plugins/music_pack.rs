//! `kind = "music"`: background tracks a host streams while it is the active input method.
//!
//! ```toml
//! [music]
//! tracks = ["rain.ogg", "piano.ogg"]
//! ```
//!
//! Tracks may be WAV or Ogg, unlike a sound pack's samples: they are streamed rather than decoded up front, so their length is checked as they play. Tracks are longer than samples by design, so a host streams them from disk instead of decoding them whole, and only while music is switched on; it pauses them whenever the input method is not active or the focused field is a secure one.

use serde::Serialize;
use toml::Value;

use super::{is_audio, only_keys, valid_file_name, AudioLimits};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["music"];

/// Bytes of one track.
pub const MAX_TRACK_BYTES: u64 = 16 * 1024 * 1024;
/// Tracks in one pack.
pub const MAX_TRACKS: usize = 8;
/// Bytes of every track of one pack together.
pub const MAX_PACK_BYTES: u64 = 64 * 1024 * 1024;
/// Length of one track, for the streaming decoder to check against the header before playing and against its running frame count while playing, the counterpart of `sound_pack::sample_frames_allowed`.
pub const MAX_TRACK_SECONDS: u64 = 15 * 60;

pub(crate) const LIMITS: AudioLimits = AudioLimits {
    files: MAX_TRACKS,
    file_bytes: MAX_TRACK_BYTES,
    total_bytes: MAX_PACK_BYTES,
};

/// Whether `frames` at `sample_rate` fit in `MAX_TRACK_SECONDS`.
pub fn track_frames_allowed(sample_rate: u32, frames: u64) -> bool {
    super::sound_pack::SAMPLE_RATES.contains(&sample_rate)
        && frames > 0
        && frames <= MAX_TRACK_SECONDS * u64::from(sample_rate)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MusicPack {
    /// Played in this order, then from the start again. File names in the pack directory, no repeats.
    pub tracks: Vec<String>,
}

pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<MusicPack, String> {
    let music = table
        .get("music")
        .and_then(Value::as_table)
        .ok_or("音乐包缺少 music 表")?;
    only_keys(music, &["tracks"], "music")?;
    let items = music
        .get("tracks")
        .and_then(Value::as_array)
        .ok_or("music 缺少 tracks")?;
    if items.is_empty() || items.len() > MAX_TRACKS {
        return Err("曲目数量不在允许范围内".into());
    }
    let mut tracks: Vec<String> = Vec::with_capacity(items.len());
    for item in items {
        let track = audio_file(item)?;
        if tracks.contains(&track) {
            return Err(format!("{track} 重复了"));
        }
        tracks.push(track);
    }
    Ok(MusicPack { tracks })
}

/// A manifest value naming a track in the pack directory: WAV or Ogg.
fn audio_file(value: &Value) -> Result<String, String> {
    let name = value.as_str().ok_or("音频文件名必须是字符串")?;
    if !valid_file_name(name) || !is_audio(name) {
        return Err(format!("{name} 不是 .wav 或 .ogg 文件名"));
    }
    Ok(name.to_owned())
}
