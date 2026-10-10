//! The process's player: one thread that owns the kira mixer, and the queue sessions post to.

use super::effect::TIER_SEMITONES;
use super::{
    decibels, decode, Event, KeyClass, Melody, PackStamp, PluginRoots, SessionSound, SoundSettings,
};
use kira::backend::Backend;
use kira::sound::static_sound::StaticSoundData;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::PlaybackState;
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Semitones, Tween};
use msime_client_core::plugins::sound_pack::{SequenceAdvance, SoundPack};
use msime_client_core::plugins::{PluginContent, PluginKind, PluginSummary};
use std::collections::HashMap;
use std::convert::Infallible;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

/// Requests the queue holds before `try_send` starts dropping sounds. A burst of keys well past what anyone types.
const QUEUE: usize = 64;
/// A key or commit sound older than this when the player gets to it is dropped: late is worse than silent.
const STALE: Duration = Duration::from_millis(200);
/// The audio device is let go after this long without a sound and without music playing. An open output stream keeps the machine from idling (CoreAudio holds a power assertion for it), so a user who switched key sounds on and walked away must not pay for that.
const IDLE_CLOSE: Duration = Duration::from_secs(30);
/// How often the player looks at a playing track, to start the next one when it ends.
const MUSIC_TICK: Duration = Duration::from_millis(250);
/// After the device fails to open, how long the player waits before trying again. A failure is usually passing (the default output switching to a headset, the sound server restarting), so it must not silence the rest of the process.
const OPEN_RETRY: Duration = Duration::from_secs(5);
/// With no music playing, a device open this long is let go at the next quiet moment and opened afresh by the next sound. kira gives no sign when its stream thread has died on a device change, and steady typing would otherwise keep a silent manager until the idle close.
const MANAGER_LIFETIME: Duration = Duration::from_secs(60);
/// The quiet moment a worn device waits for, longer than any sample a pack may have, so letting it go never cuts a sound off.
const WORN_QUIET: Duration = Duration::from_secs(2);
/// How long a closing device waits for its stopped tracks to be reported stopped. A renderer that never reports it is gone already, and the device is let go anyway.
const STOP_WAIT: Duration = Duration::from_secs(2);

type LoadResult = thread::Result<Result<Samples, String>>;

pub(super) enum Request {
    Configure(Arc<SoundSettings>),
    Event(Event, Instant),
    Loaded(u64, Box<LoadResult>),
}

struct Player {
    sender: SyncSender<Request>,
    /// The generation of the settings last sent, so the key path sends settings only when they changed.
    configured: AtomicU64,
}

static PLAYER: OnceLock<Option<Player>> = OnceLock::new();
static DISABLED: AtomicBool = AtomicBool::new(false);

/// Turn sound off for the rest of the process, saying why once.
fn disable(reason: &str) {
    if !DISABLED.swap(true, Ordering::AcqRel) {
        crate::diagnostics::report("sound is off for the rest of this process", reason);
    }
}

/// The running player, started first when `start` is set. `None` once sound has been turned off.
fn player(start: bool) -> Option<&'static Player> {
    if DISABLED.load(Ordering::Acquire) {
        return None;
    }
    let player = if start {
        PLAYER.get_or_init(spawn)
    } else {
        PLAYER.get()?
    };
    player.as_ref()
}

fn spawn() -> Option<Player> {
    let (sender, receiver) = sync_channel(QUEUE);
    let loads = sender.clone();
    match thread::Builder::new()
        .name("msime-sound".into())
        .spawn(move || run(receiver, loads))
    {
        Ok(_) => Some(Player {
            sender,
            configured: AtomicU64::new(0),
        }),
        Err(error) => {
            disable(&format!("the sound thread did not start: {error}"));
            None
        }
    }
}

/// Send the session's settings when the player last saw another generation. False when the queue refused them.
fn configure(player: &Player, sound: &SessionSound) -> bool {
    if player.configured.swap(sound.generation, Ordering::AcqRel) == sound.generation {
        return true;
    }
    let sent = player
        .sender
        .try_send(Request::Configure(Arc::clone(&sound.settings)))
        .is_ok();
    if !sent {
        player.configured.store(0, Ordering::Release);
    }
    sent
}

pub(super) fn deliver(sound: &SessionSound, event: Event) -> bool {
    let Some(player) = player(sound.settings.wanted()) else {
        return false;
    };
    configure(player, sound)
        && player
            .sender
            .try_send(Request::Event(event, Instant::now()))
            .is_ok()
}

/// Bring a running player up to the session's settings without asking for a sound, so switching everything off lets the device go and stops the music.
pub(super) fn sync(sound: &SessionSound) {
    if let Some(player) = player(false) {
        configure(player, sound);
    }
}

fn run(receiver: Receiver<Request>, loads: SyncSender<Request>) {
    let mut worker: Worker = Worker::new(loads);
    loop {
        let request = match worker.timeout(Instant::now()) {
            None => match receiver.recv() {
                Ok(request) => Some(request),
                Err(_) => return,
            },
            Some(timeout) => match receiver.recv_timeout(timeout) {
                Ok(request) => Some(request),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        let handled = catch_unwind(AssertUnwindSafe(|| {
            if let Some(request) = request {
                worker.handle(request);
            }
            worker.tick(Instant::now());
        }));
        if handled.is_err() {
            disable("the sound thread panicked");
        }
        if DISABLED.load(Ordering::Acquire) {
            return;
        }
    }
}

/// The decoded samples of the selected packs.
#[derive(Default)]
pub(super) struct Samples {
    keys: [Option<StaticSoundData>; 4],
    commit: Option<StaticSoundData>,
    achievement: Option<StaticSoundData>,
    melody: Option<MelodySamples>,
}

struct MelodySamples {
    sample: StaticSoundData,
    semitones: Vec<i8>,
    advance: SequenceAdvance,
}

/// What `Samples` are decoded from: the key pack when a key, commit, achievement or tier-up sound uses it, and the melody pack when keys play the melody. Each carries its manifest's stamp, so a pack imported again under the same id is decoded again and a removed one is dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    roots: PluginRoots,
    pack: Option<(String, PackStamp)>,
    melody_pack: Option<(String, PackStamp)>,
}

impl Selection {
    pub(super) fn of(settings: &SoundSettings) -> Option<Self> {
        let pack = settings
            .uses_key_pack()
            .then(|| (settings.pack.clone(), settings.stamps.pack));
        let melody_pack = (settings.key && settings.melody)
            .then(|| (settings.melody_pack.clone(), settings.stamps.melody_pack));
        (pack.is_some() || melody_pack.is_some()).then(|| Self {
            roots: settings.roots.clone(),
            pack,
            melody_pack,
        })
    }
}

/// A validated pack of `kind`, installed or (for sound packs) built in.
fn resolve(roots: &PluginRoots, kind: PluginKind, id: &str) -> Result<PluginSummary, String> {
    roots
        .load(kind, id)
        .map_err(|reason| format!("{} pack {id}: {reason}", kind.as_str()))
}

fn sound_pack(roots: &PluginRoots, id: &str) -> Result<(PathBuf, SoundPack), String> {
    let package = resolve(roots, PluginKind::Sound, id)?;
    match package.content {
        PluginContent::Sound(pack) => Ok((package.directory, pack)),
        _ => Err(format!("sound pack {id}: not a sound pack")),
    }
}

/// Decode every sample `selection` needs, each file once.
pub(super) fn load(selection: &Selection) -> Result<Samples, String> {
    let mut decoded: HashMap<PathBuf, StaticSoundData> = HashMap::new();
    let mut sample = |directory: &Path, name: &str| -> Result<StaticSoundData, String> {
        let path = directory.join(name);
        if let Some(data) = decoded.get(&path) {
            return Ok(data.clone());
        }
        let data = decode::sample(&path).map_err(|reason| format!("{name}: {reason}"))?;
        decoded.insert(path, data.clone());
        Ok(data)
    };
    let mut samples = Samples::default();
    if let Some((id, _)) = &selection.pack {
        let (directory, pack) = sound_pack(&selection.roots, id)?;
        for class in KeyClass::ALL {
            if let Some(name) = pack.key_sample(class.name()) {
                samples.keys[class as usize] = Some(sample(&directory, name)?);
            }
        }
        if let Some(name) = &pack.sounds.commit {
            samples.commit = Some(sample(&directory, name)?);
        }
        if let Some(name) = &pack.sounds.achievement {
            samples.achievement = Some(sample(&directory, name)?);
        }
    }
    if let Some((id, _)) = &selection.melody_pack {
        let (directory, pack) = sound_pack(&selection.roots, id)?;
        let sequence = pack
            .sequence
            .ok_or_else(|| format!("sound pack {id}: not a melody"))?;
        samples.melody = Some(MelodySamples {
            sample: sample(&directory, &sequence.sample)?,
            semitones: sequence.semitones,
            advance: sequence.advance,
        });
    }
    Ok(samples)
}

/// The music pack being played and where in it.
#[derive(Default)]
struct Music {
    /// Whether the host says music may play now.
    active: bool,
    /// The pack's directory and tracks, once resolved for the current settings.
    tracks: Option<(PathBuf, Vec<String>)>,
    resolved: bool,
    track: usize,
    handle: Option<StreamingSoundHandle<Infallible>>,
    /// Raised by the playing track's decoder when it has nothing more to give.
    ended: Arc<AtomicBool>,
}

impl Music {
    /// Forget the pack, for a change of pack or a closed device, handing back its track for the worker to stop.
    fn reset(&mut self) -> Option<StreamingSoundHandle<Infallible>> {
        self.tracks = None;
        self.resolved = false;
        self.track = 0;
        self.handle.take()
    }

    fn playing(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| handle.state().is_advancing())
    }
}

/// The player's state. The backend is kira's default (cpal) in the host; tests drive the same code over kira's mock backend.
pub(super) struct Worker<B: Backend = DefaultBackend> {
    settings: Arc<SoundSettings>,
    loads: SyncSender<Request>,
    /// The load whose result is wanted; results of an earlier one are dropped.
    load: u64,
    selection: Option<Selection>,
    samples: Option<Samples>,
    melody: Melody,
    manager: Option<AudioManager<B>>,
    /// When the open manager was opened.
    opened: Option<Instant>,
    /// When the device may be tried again after it failed to open.
    open_retry: Option<Instant>,
    /// Whether the last open failed, so a failing device is reported once rather than every few seconds.
    open_failing: bool,
    last_sound: Option<Instant>,
    music: Music,
    /// Tracks told to stop that the renderer has not yet reported stopped. The manager is kept until they are: kira's decode thread for a stream only exits once the renderer marks the stream stopped, and a stream left paused or stopping when its manager is dropped keeps that thread waking every millisecond for the life of the process.
    stopping: Vec<StreamingSoundHandle<Infallible>>,
    /// When the device was asked to close while `stopping` was not yet empty.
    closing: Option<Instant>,
}

impl<B: Backend> Worker<B>
where
    B::Settings: Default,
    B::Error: std::fmt::Debug,
{
    pub(super) fn new(loads: SyncSender<Request>) -> Self {
        Self {
            settings: Arc::default(),
            loads,
            load: 0,
            selection: None,
            samples: None,
            melody: Melody::default(),
            manager: None,
            opened: None,
            open_retry: None,
            open_failing: false,
            last_sound: None,
            music: Music::default(),
            stopping: Vec::new(),
            closing: None,
        }
    }

    /// How long the thread may wait for the next request before `tick` has something to do: never while idle with the device closed.
    fn timeout(&self, now: Instant) -> Option<Duration> {
        if (self.music.handle.is_some() && self.music.active) || !self.stopping.is_empty() {
            return Some(MUSIC_TICK);
        }
        if self.manager.is_none() {
            // Music waiting for the device to open again is retried when the back-off ends.
            let retry = self
                .open_retry
                .filter(|_| self.settings.music && self.music.active)?;
            return Some(retry.saturating_duration_since(now) + MUSIC_TICK);
        }
        let since = self
            .last_sound
            .map_or(Duration::ZERO, |last| now.saturating_duration_since(last));
        let close = if self.worn(now) {
            WORN_QUIET
        } else {
            IDLE_CLOSE
        };
        Some(close.saturating_sub(since) + MUSIC_TICK)
    }

    pub(super) fn handle(&mut self, request: Request) {
        match request {
            Request::Configure(settings) => self.configure(settings),
            Request::Event(Event::Music(active), _) => self.music.active = active,
            Request::Event(event, at) => {
                let now = Instant::now();
                if matches!(event, Event::Key(_) | Event::Commit | Event::TierUp(_))
                    && now.saturating_duration_since(at) > STALE
                {
                    return;
                }
                for sound in self.sounds_for(event, now) {
                    self.play(sound);
                }
            }
            Request::Loaded(load, result) => self.loaded(load, result),
        }
    }

    pub(super) fn configure(&mut self, settings: Arc<SoundSettings>) {
        if *settings == *self.settings {
            return;
        }
        let selection = Selection::of(&settings);
        if selection != self.selection {
            self.samples = None;
            self.melody = Melody::default();
            self.load += 1;
            self.selection = selection.clone();
            if let Some(selection) = selection {
                let loads = self.loads.clone();
                let load = self.load;
                let spawned = thread::Builder::new()
                    .name("msime-sound-load".into())
                    .spawn(move || {
                        let result = catch_unwind(|| self::load(&selection));
                        // The player has stopped when this fails, and then nobody wants the samples.
                        let _ = loads.send(Request::Loaded(load, Box::new(result)));
                    });
                if let Err(error) = spawned {
                    disable(&format!("the sound pack loader did not start: {error}"));
                }
            }
        }
        let previous = std::mem::replace(&mut self.settings, settings);
        let settings = Arc::clone(&self.settings);
        if (
            previous.music,
            &previous.music_pack,
            &previous.roots,
            previous.stamps.music_pack,
        ) != (
            settings.music,
            &settings.music_pack,
            &settings.roots,
            settings.stamps.music_pack,
        ) {
            let track = self.music.reset();
            self.retire(track);
        } else if previous.music_volume != settings.music_volume {
            if let Some(handle) = &mut self.music.handle {
                handle.set_volume(decibels(settings.music_volume), Tween::default());
            }
        }
        if !settings.wanted() {
            self.close(Instant::now());
        }
    }

    pub(super) fn loaded(&mut self, load: u64, result: Box<LoadResult>) {
        if load != self.load {
            return;
        }
        match *result {
            Ok(Ok(samples)) => self.samples = Some(samples),
            Ok(Err(reason)) => crate::diagnostics::report("sound pack not loaded", &reason),
            Err(_) => disable("decoding a sound pack panicked"),
        }
    }

    /// What `event` plays under the current settings and samples. Stepping the melody is the only state it changes.
    pub(super) fn sounds_for(&mut self, event: Event, now: Instant) -> Vec<StaticSoundData> {
        let settings = &self.settings;
        let Some(samples) = &self.samples else {
            return Vec::new();
        };
        let volume = Decibels(decibels(settings.volume));
        let melody_advance = samples
            .melody
            .as_ref()
            .filter(|_| settings.key && settings.melody)
            .map(|melody| melody.advance);
        let mut sounds = Vec::with_capacity(2);
        let mut note = |melody: &MelodySamples| {
            self.melody
                .step(&melody.semitones, now)
                .map(|semitone| melody.sample.playback_rate(Semitones(f64::from(semitone))))
        };
        match event {
            Event::Key(class) if settings.key => {
                if settings.melody {
                    if melody_advance == Some(SequenceAdvance::Key) {
                        sounds.extend(samples.melody.as_ref().and_then(&mut note));
                    }
                } else {
                    sounds.extend(samples.keys[class as usize].clone());
                }
            }
            Event::Commit => {
                if settings.commit {
                    sounds.extend(samples.commit.clone());
                }
                if melody_advance == Some(SequenceAdvance::Commit) {
                    sounds.extend(samples.melody.as_ref().and_then(&mut note));
                }
            }
            Event::Achievement if settings.achievements => {
                sounds.extend(samples.achievement.clone());
            }
            Event::TierUp(tier) if settings.tier_sound() => {
                let semitones = f64::from(tier.saturating_mul(TIER_SEMITONES));
                sounds.extend(
                    samples
                        .commit
                        .clone()
                        .map(|sample| sample.playback_rate(Semitones(semitones))),
                );
            }
            _ => {}
        }
        sounds
            .into_iter()
            .map(|sound| sound.volume(volume))
            .collect()
    }

    /// The open manager, opening the device first when it is closed. A failed open is tried again after `OPEN_RETRY`, never given up on.
    fn manager(&mut self, now: Instant) -> Option<&mut AudioManager<B>> {
        if self.manager.is_none() {
            if !may_open(self.open_retry, now) {
                return None;
            }
            match AudioManager::<B>::new(AudioManagerSettings::default()) {
                Ok(manager) => {
                    self.manager = Some(manager);
                    self.opened = Some(now);
                    self.open_retry = None;
                    self.open_failing = false;
                }
                Err(error) => {
                    if !std::mem::replace(&mut self.open_failing, true) {
                        crate::diagnostics::report(
                            "no audio output, trying again later",
                            &format!("{error:?}"),
                        );
                    }
                    self.open_retry = Some(now + OPEN_RETRY);
                    return None;
                }
            }
        }
        // A sound wants the device, so a close waiting on stopped tracks is called off.
        self.closing = None;
        self.manager.as_mut()
    }

    fn play(&mut self, sound: StaticSoundData) {
        let now = Instant::now();
        let Some(manager) = self.manager(now) else {
            return;
        };
        // Past kira's limit of sounds at once, a key sound is simply not heard.
        let _ = manager.play(sound);
        self.last_sound = Some(now);
    }

    /// Stop a track and keep its handle until the renderer reports it stopped (see `stopping`).
    fn retire(&mut self, track: Option<StreamingSoundHandle<Infallible>>) {
        if let Some(mut handle) = track {
            handle.stop(Tween::default());
            self.stopping.push(handle);
        }
    }

    /// Let the audio device go, once the renderer has reported every stopped track stopped or `STOP_WAIT` has passed. Music starts its current track again when it next plays.
    fn close(&mut self, now: Instant) {
        let track = self.music.handle.take();
        self.retire(track);
        self.stopping
            .retain(|handle| handle.state() != PlaybackState::Stopped);
        let since = *self.closing.get_or_insert(now);
        if self.stopping.is_empty() || now.saturating_duration_since(since) >= STOP_WAIT {
            self.stopping.clear();
            self.closing = None;
            self.manager = None;
            self.opened = None;
        }
    }

    fn tick(&mut self, now: Instant) {
        self.tick_music(now);
        self.stopping
            .retain(|handle| handle.state() != PlaybackState::Stopped);
        if self.manager.is_none() || self.music.playing() {
            return;
        }
        let since = self
            .last_sound
            .map_or(Duration::MAX, |last| now.saturating_duration_since(last));
        if since >= IDLE_CLOSE
            || (self.worn(now) && since >= WORN_QUIET)
            || self.closing.is_some()
            || !self.settings.wanted()
        {
            self.close(now);
        }
    }

    /// Whether the open device has been open for `MANAGER_LIFETIME`.
    fn worn(&self, now: Instant) -> bool {
        self.opened
            .is_some_and(|opened| now.saturating_duration_since(opened) >= MANAGER_LIFETIME)
    }

    fn tick_music(&mut self, now: Instant) {
        if !(self.settings.music && self.music.active) {
            if let Some(handle) = &mut self.music.handle {
                if !matches!(
                    handle.state(),
                    PlaybackState::Paused | PlaybackState::Pausing | PlaybackState::Stopped
                ) {
                    handle.pause(Tween::default());
                    self.last_sound = Some(now);
                }
            }
            return;
        }
        if let Some(handle) = &mut self.music.handle {
            match handle.state() {
                PlaybackState::Stopped => {
                    self.music.handle = None;
                    if let Some((_, tracks)) = &self.music.tracks {
                        self.music.track = (self.music.track + 1) % tracks.len();
                    }
                }
                PlaybackState::Paused | PlaybackState::Pausing => {
                    handle.resume(Tween::default());
                }
                PlaybackState::Stopping => {}
                _ if self.music.ended.load(Ordering::Acquire) => handle.stop(Tween::default()),
                _ => {}
            }
            if self.music.handle.is_some() {
                self.last_sound = Some(now);
                return;
            }
        }
        if !self.music.resolved {
            self.music.resolved = true;
            let settings = Arc::clone(&self.settings);
            match resolve(&settings.roots, PluginKind::Music, &settings.music_pack) {
                Ok(PluginSummary {
                    directory,
                    content: PluginContent::Music(pack),
                    ..
                }) => self.music.tracks = Some((directory, pack.tracks)),
                Ok(_) => {}
                Err(reason) => crate::diagnostics::report("music not played", &reason),
            }
        }
        let Some((directory, tracks)) = self.music.tracks.clone() else {
            return;
        };
        let volume = decibels(self.settings.music_volume);
        for _ in 0..tracks.len() {
            let ended = Arc::new(AtomicBool::new(false));
            let path = directory.join(&tracks[self.music.track]);
            if let Ok(decoder) = decode::track(&path, Arc::clone(&ended)) {
                let data = StreamingSoundData::from_decoder(decoder).volume(volume);
                let Some(manager) = self.manager(now) else {
                    return;
                };
                if let Ok(handle) = manager.play(data) {
                    self.music.ended = ended;
                    self.music.handle = Some(handle);
                    self.last_sound = Some(now);
                    return;
                }
            }
            self.music.track = (self.music.track + 1) % tracks.len();
        }
        crate::diagnostics::report(
            "music not played",
            &format!(
                "music pack {}: no track could be played",
                self.settings.music_pack
            ),
        );
        self.music.tracks = None;
    }
}

/// Whether the device may be opened at `now`, given when a failed open said to try again.
fn may_open(retry: Option<Instant>, now: Instant) -> bool {
    retry.is_none_or(|retry| now >= retry)
}

#[cfg(test)]
mod tests {
    use super::super::PluginRoots;
    use super::*;
    use kira::backend::mock::MockBackend;

    /// A silent mono 16-bit WAV of `frames` frames at 8 kHz.
    fn silent_wav(path: &Path, frames: u32) {
        let data = frames * 2;
        let mut bytes = Vec::with_capacity(44 + data as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&8_000u32.to_le_bytes());
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data.to_le_bytes());
        bytes.resize(44 + data as usize, 0);
        std::fs::write(path, bytes).unwrap();
    }

    /// Settings playing a one-track music pack installed under `state`, and nothing else.
    fn music_settings(state: &Path) -> SoundSettings {
        let pack = state.join("plugins/music/calm");
        std::fs::create_dir_all(&pack).unwrap();
        // Thirty seconds: far more than kira's stream buffer, so the decoder never reaches the end during a test.
        silent_wav(&pack.join("calm.wav"), 8_000 * 30);
        std::fs::write(
            pack.join("plugin.toml"),
            "schema_version = 1\nkind = \"music\"\nid = \"calm\"\nname = \"Calm\"\nversion = \"1\"\nlicense = \"CC0-1.0\"\n[music]\ntracks = [\"calm.wav\"]\n",
        )
        .unwrap();
        SoundSettings {
            roots: PluginRoots::new(state.to_str(), None, ""),
            music: true,
            music_pack: "calm".into(),
            music_volume: 30,
            ..SoundSettings::default()
        }
    }

    fn render(worker: &mut Worker<MockBackend>) {
        let backend = worker.manager.as_mut().unwrap().backend_mut();
        backend.on_start_processing();
        backend.process();
    }

    fn playing_worker(settings: &SoundSettings, now: Instant) -> Worker<MockBackend> {
        let (sender, _receiver) = sync_channel(8);
        let mut worker: Worker<MockBackend> = Worker::new(sender);
        worker.configure(Arc::new(settings.clone()));
        worker.handle(Request::Event(Event::Music(true), now));
        worker.tick(now);
        assert!(worker.music.handle.is_some(), "the track started");
        assert!(!worker.music.ended.load(Ordering::Acquire));
        render(&mut worker);
        worker
    }

    // kira's decode thread for a stream exits only when the renderer marks the stream stopped, so dropping the manager first would leave it waking every millisecond for good.
    #[test]
    fn switching_music_off_keeps_the_device_until_the_track_is_stopped() {
        let state = tempfile::tempdir().unwrap();
        let settings = music_settings(state.path());
        let now = Instant::now();
        let mut worker = playing_worker(&settings, now);
        worker.configure(Arc::new(SoundSettings {
            music: false,
            ..settings.clone()
        }));
        assert!(worker.manager.is_some());
        assert_eq!(worker.stopping.len(), 1);
        assert_eq!(worker.timeout(now), Some(MUSIC_TICK));
        render(&mut worker);
        worker.tick(now);
        assert!(worker.manager.is_none());
        assert!(worker.stopping.is_empty());
        assert_eq!(worker.timeout(now), None);
    }

    #[test]
    fn paused_music_is_stopped_before_the_idle_close() {
        let state = tempfile::tempdir().unwrap();
        let settings = music_settings(state.path());
        let now = Instant::now();
        let mut worker = playing_worker(&settings, now);
        worker.handle(Request::Event(Event::Music(false), now));
        worker.tick(now);
        render(&mut worker);
        assert_eq!(
            worker.music.handle.as_ref().unwrap().state(),
            PlaybackState::Paused
        );
        let idle = now + IDLE_CLOSE;
        worker.tick(idle);
        assert!(worker.music.handle.is_none());
        assert!(worker.manager.is_some(), "waiting for the stop to render");
        render(&mut worker);
        worker.tick(idle);
        assert!(worker.manager.is_none());
    }

    // The stream's decode thread outlives this test's device, which is what a dead renderer leaves behind anyway; it ends with the test process.
    #[test]
    fn a_renderer_that_never_reports_the_stop_does_not_hold_the_device() {
        let state = tempfile::tempdir().unwrap();
        let settings = music_settings(state.path());
        let now = Instant::now();
        let mut worker = playing_worker(&settings, now);
        worker.configure(Arc::new(SoundSettings {
            music: false,
            ..settings.clone()
        }));
        let closed = Instant::now();
        worker.tick(closed + STOP_WAIT / 2);
        assert!(worker.manager.is_some());
        worker.tick(closed + STOP_WAIT);
        assert!(worker.manager.is_none());
        assert!(worker.stopping.is_empty());
    }

    #[test]
    fn a_failed_open_is_tried_again_after_the_back_off() {
        let (sender, _receiver) = sync_channel(8);
        let mut worker: Worker<MockBackend> = Worker::new(sender);
        let now = Instant::now();
        worker.open_retry = Some(now + OPEN_RETRY);
        worker.open_failing = true;
        assert!(worker.manager(now).is_none());
        assert!(!DISABLED.load(Ordering::Acquire));
        assert!(worker.manager(now + OPEN_RETRY).is_some());
        assert!(worker.open_retry.is_none() && !worker.open_failing);
        assert!(may_open(None, now));
        assert!(!may_open(Some(now + OPEN_RETRY), now));
        assert!(may_open(Some(now), now));
    }

    #[test]
    fn a_worn_device_is_let_go_at_the_next_quiet_moment() {
        let (sender, _receiver) = sync_channel(8);
        let mut worker: Worker<MockBackend> = Worker::new(sender);
        worker.settings = Arc::new(SoundSettings {
            key: true,
            ..SoundSettings::default()
        });
        let now = Instant::now();
        assert!(worker.manager(now).is_some());
        let worn = now + MANAGER_LIFETIME;
        worker.last_sound = Some(worn);
        worker.tick(worn);
        assert!(worker.manager.is_some(), "a sound may still be playing");
        assert_eq!(worker.timeout(worn), Some(WORN_QUIET + MUSIC_TICK));
        worker.tick(worn + WORN_QUIET);
        assert!(worker.manager.is_none());
    }
}
