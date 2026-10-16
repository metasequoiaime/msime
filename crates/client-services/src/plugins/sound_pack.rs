//! `kind = "sound"`: short samples a host plays on keys, on commit and on an achievement, or one sample it plays a melody on.
//!
//! ```toml
//! mode = "keys"               # "keys" | "sequence"
//! [sounds]
//! default = "key.wav"         # required in keys mode
//! space = "space.wav"
//! enter = "enter.wav"
//! backspace = "backspace.wav"
//! commit = "commit.wav"
//! achievement = "achievement.wav"
//! [sequence]                  # sequence mode only
//! sample = "tone.wav"
//! semitones = [0, 2, 4, 5, 7]
//! advance = "key"             # "key" | "commit"
//! ```
//!
//! Every sample is a `.wav` file (RIFF/WAVE); see `super::is_wav` for why a sound pack takes no Ogg.

use serde::Serialize;
use toml::Value;

use super::{is_wav, only_keys, valid_file_name, AudioLimits};

pub(crate) const MANIFEST_KEYS: [&str; 3] = ["mode", "sounds", "sequence"];

/// Bytes of one sample.
pub const MAX_SAMPLE_BYTES: u64 = 512 * 1024;
/// Distinct samples in one pack.
pub const MAX_SAMPLES: usize = 8;
/// Bytes of every sample of one pack together.
pub const MAX_PACK_BYTES: u64 = 4 * 1024 * 1024;
/// Length of one decoded sample. The file size alone does not bound it - a low-rate sample runs far longer in the same bytes - so a host checks the frame count the header declares before decoding, and the frames it got after, against `sample_frames_allowed`.
pub const MAX_SAMPLE_MILLIS: u64 = 1_500;
/// Notes in a melody.
pub const MAX_SEMITONES: usize = 128;
/// Two octaves either way of the sample's own pitch; playback rate stays between a quarter and four times.
pub const SEMITONE_RANGE: std::ops::RangeInclusive<i64> = -24..=24;
/// A melody returns to its first note after the keyboard has been quiet this long.
pub const MELODY_IDLE_RESET_MILLIS: u64 = 3_000;

/// The lowest and highest sample rates a decoded sample may have.
pub const SAMPLE_RATES: std::ops::RangeInclusive<u32> = 8_000..=192_000;

pub(crate) const LIMITS: AudioLimits = AudioLimits {
    files: MAX_SAMPLES,
    file_bytes: MAX_SAMPLE_BYTES,
    total_bytes: MAX_PACK_BYTES,
};

/// Whether `frames` at `sample_rate` fit in `MAX_SAMPLE_MILLIS`, for a decoder to ask before decoding (from the header) and again after (from what it produced).
pub fn sample_frames_allowed(sample_rate: u32, frames: u64) -> bool {
    SAMPLE_RATES.contains(&sample_rate)
        && frames > 0
        && frames.saturating_mul(1_000) <= MAX_SAMPLE_MILLIS * u64::from(sample_rate)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundMode {
    /// A sample per key class, falling back to `default`.
    Keys,
    /// One sample, pitched through `semitones` note by note.
    Sequence,
}

/// What moves a melody on to its next note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SequenceAdvance {
    Key,
    Commit,
}

/// The key classes a host sounds, and the two events. Every entry is a file name in the pack directory.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct SoundFiles {
    pub default: Option<String>,
    pub space: Option<String>,
    pub enter: Option<String>,
    pub backspace: Option<String>,
    pub commit: Option<String>,
    pub achievement: Option<String>,
}

const SOUND_KEYS: [&str; 6] = [
    "default",
    "space",
    "enter",
    "backspace",
    "commit",
    "achievement",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sequence {
    pub sample: String,
    pub semitones: Vec<i8>,
    pub advance: SequenceAdvance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoundPack {
    pub mode: SoundMode,
    pub sounds: SoundFiles,
    pub sequence: Option<Sequence>,
}

impl SoundPack {
    /// Every audio file the manifest names, repeats included.
    pub fn files(&self) -> Vec<String> {
        let sounds = &self.sounds;
        [
            &sounds.default,
            &sounds.space,
            &sounds.enter,
            &sounds.backspace,
            &sounds.commit,
            &sounds.achievement,
        ]
        .into_iter()
        .flatten()
        .chain(self.sequence.iter().map(|sequence| &sequence.sample))
        .cloned()
        .collect()
    }

    /// The sample for a key class (`default`, `space`, `enter`, `backspace`), falling back to `default`. `None` in sequence mode, where the melody plays instead.
    pub fn key_sample(&self, class: &str) -> Option<&str> {
        if self.mode == SoundMode::Sequence {
            return None;
        }
        let sounds = &self.sounds;
        match class {
            "space" => sounds.space.as_deref(),
            "enter" => sounds.enter.as_deref(),
            "backspace" => sounds.backspace.as_deref(),
            _ => None,
        }
        .or(sounds.default.as_deref())
    }
}

pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<SoundPack, String> {
    let mode = match table.get("mode") {
        None => SoundMode::Keys,
        Some(value) => match value.as_str() {
            Some("keys") => SoundMode::Keys,
            Some("sequence") => SoundMode::Sequence,
            _ => return Err("mode 只能是 keys 或 sequence".into()),
        },
    };
    let sounds = match table.get("sounds") {
        None => SoundFiles::default(),
        Some(value) => {
            let sounds = value.as_table().ok_or("sounds 必须是一个表")?;
            only_keys(sounds, &SOUND_KEYS, "sounds")?;
            let file = |key: &str| sounds.get(key).map(audio_file).transpose();
            SoundFiles {
                default: file("default")?,
                space: file("space")?,
                enter: file("enter")?,
                backspace: file("backspace")?,
                commit: file("commit")?,
                achievement: file("achievement")?,
            }
        }
    };
    let sequence = match table.get("sequence") {
        None => None,
        Some(value) => Some(parse_sequence(
            value.as_table().ok_or("sequence 必须是一个表")?,
        )?),
    };
    match mode {
        SoundMode::Keys if sounds.default.is_none() => {
            return Err("keys 模式需要 sounds.default".into())
        }
        SoundMode::Keys if sequence.is_some() => {
            return Err("只有 sequence 模式才能有 sequence".into())
        }
        SoundMode::Sequence if sequence.is_none() => {
            return Err("sequence 模式需要 sequence 表".into())
        }
        _ => {}
    }
    Ok(SoundPack {
        mode,
        sounds,
        sequence,
    })
}

fn parse_sequence(table: &toml::map::Map<String, Value>) -> Result<Sequence, String> {
    only_keys(table, &["sample", "semitones", "advance"], "sequence")?;
    let sample = audio_file(table.get("sample").ok_or("sequence 缺少 sample")?)?;
    let items = table
        .get("semitones")
        .and_then(Value::as_array)
        .ok_or("sequence 缺少 semitones")?;
    if items.is_empty() || items.len() > MAX_SEMITONES {
        return Err("旋律的音符数量不在允许范围内".into());
    }
    let mut semitones = Vec::with_capacity(items.len());
    for item in items {
        let semitone = item
            .as_integer()
            .filter(|semitone| SEMITONE_RANGE.contains(semitone))
            .ok_or("semitones 必须是 -24 到 24 之间的整数")?;
        semitones.push(semitone as i8);
    }
    let advance = match table.get("advance") {
        None => SequenceAdvance::Key,
        Some(value) => match value.as_str() {
            Some("key") => SequenceAdvance::Key,
            Some("commit") => SequenceAdvance::Commit,
            _ => return Err("advance 只能是 key 或 commit".into()),
        },
    };
    Ok(Sequence {
        sample,
        semitones,
        advance,
    })
}

/// A manifest value naming a sample in the pack directory: a `.wav` file, never Ogg (`super::is_wav`).
fn audio_file(value: &Value) -> Result<String, String> {
    let name = value.as_str().ok_or("音频文件名必须是字符串")?;
    if !valid_file_name(name) || !is_wav(name) {
        return Err(format!("{name} 不是 .wav 文件名，音效包只接受 WAV 音频"));
    }
    Ok(name.to_owned())
}
