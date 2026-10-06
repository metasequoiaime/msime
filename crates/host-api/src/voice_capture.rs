//! Microphone capture for platform hosts: bounded mono 16 kHz PCM from the default input device, and the capture endpoints a host can offer for selection.
//!
//! This replaces the Engine's miniaudio `AudioCapture` and keeps what hosts relied on: the same bounds, a partial buffer when the time runs out first, and device identities in the backend-qualified hex form (`wasapi:`, `coreaudio:`, `alsa:`) that saved preferences already hold and the Windows TSF host still opens. Identities and labels may identify a user's hardware, so neither belongs in logs.

// iOS and HarmonyOS have no capture path here, and Android's AAudio endpoints have no identity a host can select by, so the shared bounds and encoders go unused there.
#![cfg_attr(
    any(target_os = "ios", target_os = "android", target_env = "ohos"),
    allow(dead_code)
)]

/// The rate every consumer of captured audio expects.
pub(crate) const SAMPLE_RATE: u32 = 16_000;
/// The longest capture one call may ask for.
pub(crate) const MAX_MILLISECONDS: u32 = 60_000;
/// How many endpoints a listing reports at most.
const MAX_DEVICES: usize = 128;

/// Samples `milliseconds` of capture produce at `SAMPLE_RATE`.
fn capacity(milliseconds: u32) -> usize {
    SAMPLE_RATE as usize * milliseconds as usize / 1000
}

/// A backend device identifier as the opaque ASCII id hosts persist: the prefix, then every code unit of the identifier as `digits` lowercase hex digits. `None` for an empty identifier or one the native field could not hold with its terminator (`capacity` units), which the reference could not report either and the Windows host refuses to open.
fn encoded_device_id(
    prefix: &str,
    units: impl ExactSizeIterator<Item = u32>,
    digits: usize,
    capacity: usize,
) -> Option<String> {
    if units.len() == 0 || units.len() >= capacity {
        return None;
    }
    let mut id = String::with_capacity(prefix.len() + units.len() * digits);
    id.push_str(prefix);
    for unit in units {
        id.push_str(&format!("{unit:0digits$x}"));
    }
    Some(id)
}

/// WASAPI endpoint ids are UTF-16 in a 64-unit field; the other backends carry UTF-8 bytes in a 256-byte one.
#[cfg_attr(not(windows), allow(dead_code))]
fn wasapi_device_id(endpoint: &str) -> Option<String> {
    let units = endpoint.encode_utf16().map(u32::from).collect::<Vec<_>>();
    encoded_device_id("wasapi:", units.into_iter(), 4, 64)
}

#[cfg_attr(
    not(any(
        target_vendor = "apple",
        all(target_os = "linux", not(target_env = "ohos"))
    )),
    allow(dead_code)
)]
fn byte_device_id(prefix: &str, native: &str) -> Option<String> {
    encoded_device_id(prefix, native.bytes().map(u32::from), 2, 256)
}

fn bounded_collect<T, I>(iter: I, limit: usize) -> Vec<T>
where
    I: IntoIterator<Item = T>,
{
    let mut values = Vec::with_capacity(limit);
    values.extend(iter.into_iter().take(limit));
    values
}

#[cfg(not(any(target_os = "ios", target_env = "ohos")))]
mod backend {
    use super::{bounded_collect, capacity, MAX_DEVICES, MAX_MILLISECONDS, SAMPLE_RATE};
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
    use rubato::audioadapter_buffers::direct::InterleavedSlice;
    use rubato::{Fft, FixedSync, Resampler};
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::Duration;

    /// What the capture thread fills and the caller waits on.
    struct Capture {
        samples: Vec<f32>,
        /// The native-rate mono frames that make up the requested duration.
        wanted: usize,
    }

    type Shared = Arc<(Mutex<Capture>, Condvar)>;

    pub(crate) fn capture_audio(milliseconds: u32) -> Vec<f32> {
        if milliseconds == 0 || milliseconds > MAX_MILLISECONDS {
            return Vec::new();
        }
        let maximum = capacity(milliseconds);
        let Some(device) = cpal::default_host().default_input_device() else {
            return Vec::new();
        };
        // The device's own format, never a requested 16 kHz one: CoreAudio would switch the device's nominal rate for every application to honour it, and a WASAPI capture client accepts only the shared mix format. Anything other than 16 kHz is resampled here instead, as miniaudio did.
        let Ok(supported) = device.default_input_config() else {
            return Vec::new();
        };
        let format = supported.sample_format();
        let config = supported.config();
        let rate = config.sample_rate;
        if rate == 0 || config.channels == 0 {
            return Vec::new();
        }
        let wanted = (maximum as u64 * u64::from(rate)).div_ceil(u64::from(SAMPLE_RATE)) as usize;
        let shared: Shared = Arc::new((
            Mutex::new(Capture {
                samples: Vec::with_capacity(wanted),
                wanted,
            }),
            Condvar::new(),
        ));
        let stream = match format {
            SampleFormat::I8 => build::<i8>(&device, config, &shared),
            SampleFormat::I16 => build::<i16>(&device, config, &shared),
            SampleFormat::I24 => build::<cpal::I24>(&device, config, &shared),
            SampleFormat::I32 => build::<i32>(&device, config, &shared),
            SampleFormat::I64 => build::<i64>(&device, config, &shared),
            SampleFormat::U8 => build::<u8>(&device, config, &shared),
            SampleFormat::U16 => build::<u16>(&device, config, &shared),
            SampleFormat::U24 => build::<cpal::U24>(&device, config, &shared),
            SampleFormat::U32 => build::<u32>(&device, config, &shared),
            SampleFormat::U64 => build::<u64>(&device, config, &shared),
            SampleFormat::F32 => build::<f32>(&device, config, &shared),
            SampleFormat::F64 => build::<f64>(&device, config, &shared),
            _ => None,
        };
        let Some(stream) = stream else {
            return Vec::new();
        };
        if stream.play().is_err() {
            return Vec::new();
        }
        let (lock, done) = &*shared;
        let Ok(guard) = lock.lock() else {
            return Vec::new();
        };
        let Ok((guard, _)) = done.wait_timeout_while(
            guard,
            Duration::from_millis(u64::from(milliseconds)),
            |capture| capture.samples.len() < capture.wanted,
        ) else {
            return Vec::new();
        };
        // Dropping the stream waits for the capture thread, which may be waiting on this lock to deliver its last buffer, so the lock is released first.
        drop(guard);
        drop(stream);
        let Ok(mut capture) = lock.lock() else {
            return Vec::new();
        };
        let native = std::mem::take(&mut capture.samples);
        drop(capture);
        let mut samples = if rate == SAMPLE_RATE {
            native
        } else {
            resample(&native, rate)
        };
        samples.truncate(maximum);
        samples
    }

    /// An input stream that downmixes each frame to its mean and keeps the first `wanted` of them.
    fn build<T>(
        device: &cpal::Device,
        config: StreamConfig,
        shared: &Shared,
    ) -> Option<cpal::Stream>
    where
        T: SizedSample,
        f32: FromSample<T>,
    {
        let channels = usize::from(config.channels);
        let data = Arc::clone(shared);
        device
            .build_input_stream::<T, _, _>(
                config,
                move |input: &[T], _| {
                    let (lock, done) = &*data;
                    let Ok(mut capture) = lock.lock() else {
                        return;
                    };
                    let room = capture.wanted.saturating_sub(capture.samples.len());
                    capture
                        .samples
                        .extend(input.chunks_exact(channels).take(room).map(|frame| {
                            frame
                                .iter()
                                .map(|&sample| sample.to_sample::<f32>())
                                .sum::<f32>()
                                / channels as f32
                        }));
                    if capture.samples.len() >= capture.wanted {
                        done.notify_one();
                    }
                },
                // The reference ignored backend errors too: a device that stops delivering leaves the samples it already gave, returned when the time runs out. Only a callback that threw discarded a capture, and this one cannot fail.
                |_| {},
                None,
            )
            .ok()
    }

    /// The whole clip at once: the rates are fixed, so the synchronous FFT resampler is both the fastest and the most accurate choice, and `process_all` trims its delay so the output lines up with the input.
    fn resample(samples: &[f32], rate: u32) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }
        let Ok(mut resampler) = Fft::<f32>::new(
            rate as usize,
            SAMPLE_RATE as usize,
            1024,
            1,
            FixedSync::Both,
        ) else {
            return Vec::new();
        };
        let Ok(input) = InterleavedSlice::new(samples, 1, samples.len()) else {
            return Vec::new();
        };
        resampler
            .process_all(&input, samples.len(), None)
            .map(|output| output.take_data())
            .unwrap_or_default()
    }

    pub(crate) fn capture_devices() -> Vec<(String, String)> {
        let host = cpal::default_host();
        let Ok(devices) = host.input_devices() else {
            return Vec::new();
        };
        bounded_collect(
            devices.filter_map(|device| {
                let native = device.id().ok()?;
                let id = match native.host() {
                    #[cfg(target_os = "windows")]
                    cpal::HostId::Wasapi => super::wasapi_device_id(native.id()),
                    #[cfg(target_vendor = "apple")]
                    cpal::HostId::CoreAudio => super::byte_device_id("coreaudio:", native.id()),
                    #[cfg(any(
                        target_os = "linux",
                        target_os = "dragonfly",
                        target_os = "freebsd",
                        target_os = "netbsd"
                    ))]
                    cpal::HostId::Alsa => super::byte_device_id("alsa:", native.id()),
                    // Other backends (AAudio, ASIO, JACK) had no identity the hosts could select by.
                    #[allow(unreachable_patterns)]
                    _ => None,
                }?;
                let label = device.description().ok()?.name().to_owned();
                (!label.is_empty()).then_some((id, label))
            }),
            MAX_DEVICES,
        )
    }
}

/// The iOS keyboard records through AVAudioEngine and HarmonyOS through its own audio kit; neither has a capture path here.
#[cfg(any(target_os = "ios", target_env = "ohos"))]
mod backend {
    pub(crate) fn capture_audio(_milliseconds: u32) -> Vec<f32> {
        Vec::new()
    }

    pub(crate) fn capture_devices() -> Vec<(String, String)> {
        Vec::new()
    }
}

/// Bounded mono 16 kHz samples from the default input device. Empty when the duration is outside 1..=60000 ms, no device could be opened, or nothing arrived in time.
pub(crate) fn capture_audio(milliseconds: u32) -> Vec<f32> {
    backend::capture_audio(milliseconds)
}

/// Capture endpoint identities paired with labels, at most 128.
pub(crate) fn capture_devices() -> Vec<(String, String)> {
    backend::capture_devices()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasapi_ids_are_utf16_code_units_in_four_hex_digits() {
        assert_eq!(wasapi_device_id("a").as_deref(), Some("wasapi:0061"));
        assert_eq!(
            wasapi_device_id("{0.0.1}").as_deref(),
            Some("wasapi:007b0030002e0030002e0031007d")
        );
        // A character outside the BMP is two code units, as the native wide string holds it.
        assert_eq!(
            wasapi_device_id("\u{1F3A4}").as_deref(),
            Some("wasapi:d83cdfa4")
        );
    }

    #[test]
    fn byte_ids_are_utf8_bytes_in_two_hex_digits() {
        assert_eq!(
            byte_device_id("coreaudio:", "BuiltIn").as_deref(),
            Some("coreaudio:4275696c74496e")
        );
        assert_eq!(
            byte_device_id("alsa:", "hw:0,0").as_deref(),
            Some("alsa:68773a302c30")
        );
        assert_eq!(
            byte_device_id("coreaudio:", "é").as_deref(),
            Some("coreaudio:c3a9")
        );
    }

    #[test]
    fn ids_the_native_field_could_not_hold_are_not_reported() {
        assert_eq!(wasapi_device_id(""), None);
        assert_eq!(byte_device_id("alsa:", ""), None);
        assert!(wasapi_device_id(&"a".repeat(63)).is_some());
        assert_eq!(wasapi_device_id(&"a".repeat(64)), None);
        assert!(byte_device_id("coreaudio:", &"a".repeat(255)).is_some());
        assert_eq!(byte_device_id("coreaudio:", &"a".repeat(256)), None);
    }

    #[test]
    fn out_of_range_durations_capture_nothing() {
        assert!(capture_audio(0).is_empty());
        assert!(capture_audio(MAX_MILLISECONDS + 1).is_empty());
        assert_eq!(capacity(1000), 16_000);
        assert_eq!(capacity(MAX_MILLISECONDS), 960_000);
    }

    #[test]
    fn bounded_device_collection_reserves_the_listing_limit() {
        let devices = bounded_collect([("id", "label")], MAX_DEVICES);

        assert_eq!(devices.len(), 1);
        assert!(devices.capacity() >= MAX_DEVICES);
    }
}
