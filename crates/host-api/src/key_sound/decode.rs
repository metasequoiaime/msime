//! Pack audio decoded by symphonia under the pack bounds.
//!
//! client-core checked each file's size, extension and header bytes when it validated the pack; the length is only known here. A sample is refused when its header declares more than `MAX_SAMPLE_MILLIS`, when decoding runs past that bound whatever the header said, and when what came out is empty or out of range, so a file that lies about its length costs at most one sample's worth of memory. A track has to declare a length within `MAX_TRACK_SECONDS` before it plays, and stops at that length while it streams.

use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use kira::sound::streaming::Decoder;
use kira::Frame;
use msime_client_core::plugins::{music_pack, sound_pack};
use std::convert::Infallible;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;

/// Frames of silence a finished track hands the mixer until the player stops it.
const SILENCE_FRAMES: usize = 1024;

fn sample_frame_capacity(declared: Option<u64>, limit: u64) -> usize {
    declared
        .filter(|frames| *frames <= limit)
        .and_then(|frames| usize::try_from(frames).ok())
        .unwrap_or(0)
}

/// An opened file's audio track.
struct Source {
    reader: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    sample_rate: u32,
    channels: usize,
    /// The length the header declares, when it declares one.
    frames: Option<u64>,
    interleaved: Vec<f32>,
}

fn open(path: &Path, maximum_bytes: u64) -> Result<Source, String> {
    let file = crate::bounded_file::open_private(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > maximum_bytes {
        return Err("not a regular file of the allowed size".into());
    }
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let reader = symphonia::default::get_probe()
        .probe(
            &Hint::default(),
            stream,
            Default::default(),
            Default::default(),
        )
        .map_err(|error| error.to_string())?;
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or("no audio track")?;
    let Some(CodecParameters::Audio(parameters)) = track.codec_params.as_ref() else {
        return Err("no audio track".into());
    };
    let sample_rate = parameters
        .sample_rate
        .filter(|rate| sound_pack::SAMPLE_RATES.contains(rate))
        .ok_or("unsupported sample rate")?;
    let channels = parameters
        .channels
        .as_ref()
        .map(|channels| channels.count())
        .filter(|count| matches!(count, 1 | 2))
        .ok_or("only mono and stereo audio is played")?;
    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(parameters, &Default::default())
        .map_err(|error| error.to_string())?;
    let (track, frames) = (track.id, track.num_frames);
    Ok(Source {
        reader,
        decoder,
        track,
        sample_rate,
        channels,
        frames,
        interleaved: Vec::new(),
    })
}

impl Source {
    /// The frames of the track's next packet; `None` at its end.
    fn next(&mut self) -> Result<Option<Vec<Frame>>, String> {
        let packet = loop {
            match self.reader.next_packet() {
                Ok(Some(packet)) if packet.track_id == self.track => break packet,
                Ok(Some(_)) => {}
                Ok(None) => return Ok(None),
                Err(error) => return Err(error.to_string()),
            }
        };
        let buffer = self
            .decoder
            .decode(&packet)
            .map_err(|error| error.to_string())?;
        if buffer.num_planes() != self.channels {
            return Err("the channel layout changed".into());
        }
        buffer.copy_to_vec_interleaved(&mut self.interleaved);
        Ok(Some(match self.channels {
            1 => self
                .interleaved
                .iter()
                .map(|sample| Frame::from_mono(*sample))
                .collect(),
            _ => self
                .interleaved
                .chunks_exact(2)
                .map(|pair| Frame::new(pair[0], pair[1]))
                .collect(),
        }))
    }
}

/// One sample of a sound pack, decoded whole.
pub(super) fn sample(path: &Path) -> Result<StaticSoundData, String> {
    let mut source = open(path, sound_pack::MAX_SAMPLE_BYTES)?;
    let rate = source.sample_rate;
    if let Some(declared) = source.frames {
        if !sound_pack::sample_frames_allowed(rate, declared) {
            return Err("the sample is longer than a pack allows".into());
        }
    }
    let limit = sound_pack::MAX_SAMPLE_MILLIS * u64::from(rate) / 1_000;
    let mut frames: Vec<Frame> = Vec::with_capacity(sample_frame_capacity(source.frames, limit));
    while let Some(chunk) = source.next()? {
        // A header that understates the length is stopped here, not at the end of the file.
        if (frames.len() + chunk.len()) as u64 > limit {
            return Err("the sample is longer than a pack allows".into());
        }
        frames.extend(chunk);
    }
    if !sound_pack::sample_frames_allowed(rate, frames.len() as u64) {
        return Err("the sample is empty or longer than a pack allows".into());
    }
    Ok(StaticSoundData {
        sample_rate: rate,
        frames: frames.into(),
        settings: StaticSoundSettings::default(),
        slice: None,
    })
}

/// A music track streamed through kira. It never reports an error: kira's decode thread retries a failing decoder without pause, so the end of the file, a decode error or a decoder panic instead raise `ended` and hand out silence until the player, which watches that flag, stops the sound.
pub(super) struct TrackDecoder {
    source: Source,
    frames: usize,
    produced: usize,
    ended: Arc<AtomicBool>,
}

pub(super) fn track(path: &Path, ended: Arc<AtomicBool>) -> Result<TrackDecoder, String> {
    let source = open(path, music_pack::MAX_TRACK_BYTES)?;
    let frames = source
        .frames
        .filter(|frames| music_pack::track_frames_allowed(source.sample_rate, *frames))
        .and_then(|frames| usize::try_from(frames).ok())
        .ok_or("the track does not declare a length a pack allows")?;
    Ok(TrackDecoder {
        source,
        frames,
        produced: 0,
        ended,
    })
}

impl Decoder for TrackDecoder {
    type Error = Infallible;

    fn sample_rate(&self) -> u32 {
        self.source.sample_rate
    }

    fn num_frames(&self) -> usize {
        self.frames
    }

    fn decode(&mut self) -> Result<Vec<Frame>, Self::Error> {
        if !self.ended.load(Ordering::Acquire) {
            let source = &mut self.source;
            match catch_unwind(AssertUnwindSafe(|| source.next())) {
                Ok(Ok(Some(mut chunk))) => {
                    let room = self.frames - self.produced;
                    if chunk.len() >= room {
                        chunk.truncate(room);
                        self.ended.store(true, Ordering::Release);
                    }
                    self.produced += chunk.len();
                    // A packet may decode to no frames (a Vorbis stream's first one does); kira asks again, and silence here would shift the rest of the track.
                    if !chunk.is_empty() || !self.ended.load(Ordering::Acquire) {
                        return Ok(chunk);
                    }
                }
                _ => self.ended.store(true, Ordering::Release),
            }
        }
        Ok(vec![Frame::ZERO; SILENCE_FRAMES])
    }

    /// kira seeks to the start position when it opens a stream, which is where the decoder already is. Music never loops or seeks otherwise, so any other seek comes from something unexpected; the track ends there rather than trusting the demuxer to land where asked.
    fn seek(&mut self, index: usize) -> Result<usize, Self::Error> {
        if index != self.produced {
            self.ended.store(true, Ordering::Release);
        }
        Ok(index)
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;

    #[test]
    fn sample_frame_capacity_uses_a_valid_declared_length() {
        assert_eq!(sample_frame_capacity(Some(123), 1_000), 123);
        assert_eq!(sample_frame_capacity(None, 1_000), 0);
        assert_eq!(sample_frame_capacity(Some(1_001), 1_000), 0);
    }
}
