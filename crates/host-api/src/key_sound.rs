//! Key sounds, the typing melody, commit and achievement sounds, and background music.
//!
//! One player serves the whole process, however many sessions it holds: there is one audio device, and a host with several fields open types into one of them at a time. It starts on the first call that has something switched on, not when the library loads, so the Windows TSF DLL, which links this library into every process it is injected into and never calls these functions, opens no audio device and starts no thread.
//!
//! The key path only posts a request. A session compares a generation number rather than its settings, the request goes into a bounded queue with `try_send`, so a full queue drops a sound instead of holding up a keystroke, and everything else - resolving packs, decoding, mixing - happens on the player's own thread. Samples are decoded on a thread of their own and the whole table is swapped in when it is ready, so changing packs never plays half of one and half of the other.
//!
//! Only macOS, Windows and Linux play anything here. On iOS, Android and HarmonyOS the entry points report that nothing was queued; HarmonyOS plays packs itself from the files `pack_files` and `music_pack_files` resolve.
//!
//! A failure of the audio device, or a panic anywhere in the player, turns sound off for the rest of the process with one line on standard error. A pack that does not decode is reported once and stays silent.

// Without the player the melody, the volume curve and most of the settings go unread on the platforms that do not play.
#![cfg_attr(
    any(target_os = "ios", target_os = "android", target_env = "ohos"),
    allow(dead_code)
)]

use msime_client_core::preferences::{KeySoundMode, PluginPreferences};
use msime_client_services::plugins::effect_pack::TypingEffect;
use msime_client_services::plugins::{
    self, music_pack, sound_pack, BuiltinDirs, EffectStyle, PluginContent, PluginKind,
    PluginSummary, MANIFEST_FILE,
};
use serde_json::{json, Value};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
mod decode;
mod effect;
#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
mod player;

use effect::Combo;
pub(crate) use effect::{typing_effect, EVENT_COMMIT, FLAG_AUTO_REPEAT, FLAG_MUTED};

/// The key classes a host reports, numbered as `msime_client_key_sound` takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyClass {
    Default = 0,
    Space = 1,
    Enter = 2,
    Backspace = 3,
}

impl KeyClass {
    pub(crate) const ALL: [Self; 4] = [Self::Default, Self::Space, Self::Enter, Self::Backspace];

    pub(crate) fn from_code(code: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|class| *class as u32 == code)
    }

    /// The class name `SoundPack::key_sample` takes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Space => "space",
            Self::Enter => "enter",
            Self::Backspace => "backspace",
        }
    }
}

/// What a session asks the player for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    Key(KeyClass),
    Commit,
    Achievement,
    /// A combo reached the tier with this index into `plugins::COMBO_MILESTONES`: the key pack's commit sample, pitched up by the tier.
    TierUp(u8),
    /// Whether the input method is the active one in a field that may hear music: false when it loses activation or the focused field is a secure one.
    Music(bool),
}

/// The directories a session reads plugin packs from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PluginRoots {
    /// `<state root>/plugins`: installed packs and `mentions.json`. `None` when the host named no state root.
    pub installed: Option<PathBuf>,
    /// The built-in sound packs of the host's bundle.
    pub builtin_sounds: Option<PathBuf>,
    /// 桌面安装包随带的官方插件包（`<kind>/<id>/`）。
    pub builtin_packs: Option<PathBuf>,
}

impl PluginRoots {
    /// `state_root` is HostOptions' `preferences_directory`, `sound_packs` its `sound_packs`. Without `sound_packs` the built-in packs are looked for in `sound-packs` beside `resources`, where the macOS bundle and the Linux install put them; a relative path names nothing.
    ///
    /// `plugin_packs` 是宿主选项里的同名项。没给时取内置音效包目录旁边的 `plugin-packs`（存在才用），macOS 的 bundle 和 Linux 的安装都把官方插件包放在那里，这两个宿主已经在传 `sound_packs`，不用另外传。Windows 的 Server 和设置应用总是显式给出（在 Program Files 里）：它的内置音效包在普通用户可写的数据目录里，旁边的目录不能当作安装包的东西。
    pub(crate) fn new(
        state_root: Option<&str>,
        sound_packs: Option<&str>,
        plugin_packs: Option<&str>,
        resources: &str,
    ) -> Self {
        let absolute = |path: &str| {
            let path = Path::new(path);
            path.is_absolute().then(|| path.to_path_buf())
        };
        let builtin_sounds = match sound_packs {
            Some(path) => absolute(path),
            None => absolute(resources)
                .and_then(|resources| Some(resources.parent()?.join("sound-packs")))
                .filter(|path| path.is_dir()),
        };
        let builtin_packs = match plugin_packs {
            Some(path) => absolute(path),
            None => builtin_sounds
                .as_deref()
                .and_then(Path::parent)
                .map(|parent| parent.join("plugin-packs"))
                .filter(|path| path.is_dir()),
        };
        Self {
            installed: state_root
                .and_then(absolute)
                .map(|root| root.join("plugins")),
            builtin_sounds,
            builtin_packs,
        }
    }

    /// 两个内置目录，交给 client-services 的载入和列目录。
    pub(crate) fn builtin(&self) -> BuiltinDirs<'_> {
        BuiltinDirs {
            sounds: self.builtin_sounds.as_deref(),
            packs: self.builtin_packs.as_deref(),
        }
    }

    /// The manifest a pack of `kind` would be loaded from, without checking anything: a built-in id names the bundle's copy, any other id the installed one. `None` where `load` would refuse for want of a root.
    fn manifest(&self, kind: PluginKind, id: &str) -> Option<PathBuf> {
        let directory = if plugins::is_builtin(kind, id) {
            // 内置 id 不看插件根目录。
            plugins::pack_directory(Path::new(""), self.builtin(), kind, id)?
        } else {
            plugins::kind_directory(self.installed.as_deref()?, kind).join(id)
        };
        Some(directory.join(MANIFEST_FILE))
    }

    /// 按 client-services 的规则校验过的一个 `kind` 类包。只有内置音效包或音乐包不需要状态根目录就能解析：其他包都只会装在状态根目录下，没有它时去查，就会相对工作目录读 `<kind>/<id>`，交回相对路径。
    pub(crate) fn load(&self, kind: PluginKind, id: &str) -> Result<PluginSummary, String> {
        let builtin = plugins::is_builtin(kind, id);
        let installed = match self.installed.as_deref() {
            Some(installed) => installed,
            None if builtin => Path::new(""),
            None => return Err(format!("{} packs need a state root", kind.as_str())),
        };
        plugins::load_package(installed, self.builtin(), kind, id)
    }
}

/// What a pack's manifest looked like when its settings were taken: its modification time, length and (on Unix) inode, or `None` when there was none. Importing a pack again writes a new manifest and removing it takes the manifest away, so a changed stamp is how a session notices either without reading the pack.
pub(crate) type PackStamp = Option<(Option<SystemTime>, u64, u64)>;

pub(crate) fn stamp(path: Option<PathBuf>) -> PackStamp {
    let metadata = std::fs::symlink_metadata(path?).ok()?;
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&metadata);
    #[cfg(not(unix))]
    let inode = 0;
    Some((metadata.modified().ok(), metadata.len(), inode))
}

/// The stamps of the packs the settings play from, taken only for a pack in use, so changing a pack nobody plays reloads nothing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PackStamps {
    pub pack: PackStamp,
    pub melody_pack: PackStamp,
    pub music_pack: PackStamp,
    pub effect_pack: PackStamp,
}

/// The effect and music settings of one preference document, with where their packs are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SoundSettings {
    pub roots: PluginRoots,
    pub key: bool,
    /// Keys play the melody pack's next note instead of their own sample.
    pub melody: bool,
    pub commit: bool,
    pub achievements: bool,
    pub pack: String,
    pub melody_pack: String,
    /// 0-100, for every effect sound.
    pub volume: u8,
    pub music: bool,
    pub music_pack: String,
    pub music_volume: u8,
    /// The preferences' `effect_style`, `effect_intensity` and `effect_pack`, kept so that a restamp can resolve the pack again.
    pub effect_style: EffectStyle,
    pub effect_intensity: u8,
    pub effect_pack: String,
    /// The typing effect once `effect_pack` is resolved, which is what the session draws: `Off` with `combo_counter` off answers every typing-effect call with 0.
    pub effect: TypingEffect,
    pub combo_counter: bool,
    pub combo_tier_sound: bool,
    pub stamps: PackStamps,
}

impl SoundSettings {
    pub(crate) fn new(preferences: &PluginPreferences, roots: &PluginRoots) -> Self {
        let mut settings = Self {
            roots: roots.clone(),
            key: preferences.key_sound.enabled,
            melody: preferences.key_sound.mode == KeySoundMode::Melody,
            commit: preferences.commit_sound.enabled,
            achievements: preferences.achievements.enabled,
            pack: preferences.key_sound.pack.clone(),
            melody_pack: preferences.melody.pack.clone(),
            volume: preferences.key_sound.volume,
            music: preferences.music.enabled && !preferences.music.pack.is_empty(),
            music_pack: preferences.music.pack.clone(),
            music_volume: preferences.music.volume,
            effect_style: preferences.effect_style,
            // 本库编译到的平台固定了强度（`HostPlatform::fixed_effect_intensity`，macOS、Windows、鸿蒙）时不读偏好里存的值。鸿蒙键盘的效果由 ArkTS 的 `TypingEffectPolicy` 画，它同样用固定的 `DEFAULT_INTENSITY`，不读存下的值。
            effect_intensity: crate::ffi::host::compiled_platform()
                .fixed_effect_intensity()
                .unwrap_or(preferences.effect_intensity),
            effect_pack: preferences.effect_pack.clone(),
            effect: TypingEffect::default(),
            combo_counter: preferences.combo_counter,
            combo_tier_sound: preferences.combo_tier_sound,
            stamps: PackStamps::default(),
        };
        // Stamped before the effect pack is read, so a pack replaced in between is read again on the next restamp rather than never.
        settings.stamps = settings.stamp_packs();
        settings.effect = settings.resolve_effect();
        settings
    }

    /// Load the selected effect pack, or take the preferences' style without one. Reads one small manifest; never called on the key path.
    pub(crate) fn resolve_effect(&self) -> TypingEffect {
        TypingEffect::resolve(
            self.roots.installed.as_deref(),
            &self.effect_pack,
            self.effect_style,
            self.effect_intensity,
        )
    }

    /// Stamp the manifests of the packs in use. A few `stat`s; never called on the key path.
    pub(crate) fn stamp_packs(&self) -> PackStamps {
        let manifest = |used: bool, kind: PluginKind, id: &str| {
            stamp(used.then(|| self.roots.manifest(kind, id)).flatten())
        };
        PackStamps {
            pack: manifest(self.uses_key_pack(), PluginKind::Sound, &self.pack),
            melody_pack: manifest(
                self.key && self.melody,
                PluginKind::Sound,
                &self.melody_pack,
            ),
            music_pack: manifest(self.music, PluginKind::Music, &self.music_pack),
            effect_pack: manifest(
                !self.effect_pack.is_empty(),
                PluginKind::Effect,
                &self.effect_pack,
            ),
        }
    }

    /// Whether anything plays from the key pack: its key samples, its commit sample (the commit sound and the combo's tier-up), or its achievement jingle.
    pub(crate) fn uses_key_pack(&self) -> bool {
        (self.key && !self.melody) || self.commit || self.achievements || self.tier_sound()
    }

    /// Whether a combo reaching a new tier plays the tier-up sound.
    pub(crate) fn tier_sound(&self) -> bool {
        self.combo_counter && self.combo_tier_sound
    }

    /// Whether the player has anything to do. Nothing starts it until this holds, and it lets the audio device go once this stops holding.
    pub(crate) fn wanted(&self) -> bool {
        self.key || self.commit || self.achievements || self.music || self.tier_sound()
    }

    /// Whether a commit makes a sound: its own sample, or the next note of a melody that advances on commits (only the pack knows which, so any melody counts here).
    fn commit_sounds(&self) -> bool {
        self.commit || (self.key && self.melody)
    }
}

/// Numbers every distinct settings value a session publishes, so the key path compares one integer.
static GENERATIONS: AtomicU64 = AtomicU64::new(1);
/// The settings the process's sessions published last, for the calls that come without a session: an achievement is noticed where typing statistics are recorded.
static LATEST: Mutex<Option<(Arc<SoundSettings>, u64)>> = Mutex::new(None);
/// `LATEST`'s achievement switch, read without the lock by every statistics record.
static ACHIEVEMENTS: AtomicBool = AtomicBool::new(false);

/// A session's settings and the generation that names them, with the session's combo.
#[derive(Clone, Debug)]
pub(crate) struct SessionSound {
    settings: Arc<SoundSettings>,
    generation: u64,
    /// In a `Cell` because the key path reaches the session through a shared borrow (`with_sound`); a session lives on one thread, so this is neither a lock nor shared with any other session.
    combo: Cell<Combo>,
}

impl SessionSound {
    pub(crate) fn new(settings: SoundSettings) -> Self {
        let sound = Self {
            settings: Arc::new(settings),
            generation: GENERATIONS.fetch_add(1, Ordering::Relaxed),
            combo: Cell::default(),
        };
        sound.publish();
        sound
    }

    /// Take the settings of a newer preference document. The generation only moves when they differ, so a change to an unrelated preference never reloads a pack. The combo carries over.
    pub(crate) fn update(&mut self, settings: SoundSettings) {
        if *self.settings != settings {
            let combo = self.combo.get();
            *self = Self::new(settings);
            self.combo.set(combo);
        }
    }

    /// Stamp the packs in use again, for a field that just gained focus: the settings page may have imported a pack again or removed it since. When a stamp moved the settings take a new generation and a running player is told at once, so it decodes the new files or falls silent instead of playing what it cached.
    pub(crate) fn restamp(&mut self) {
        let stamps = self.settings.stamp_packs();
        if stamps != self.settings.stamps {
            let mut settings = SoundSettings {
                stamps,
                ..(*self.settings).clone()
            };
            settings.effect = settings.resolve_effect();
            self.update(settings);
            sync(self);
        }
    }

    fn publish(&self) {
        ACHIEVEMENTS.store(self.settings.achievements, Ordering::Relaxed);
        *LATEST.lock().unwrap_or_else(PoisonError::into_inner) =
            Some((Arc::clone(&self.settings), self.generation));
    }
}

/// The session's resolved typing effect for `msime_client_typing_effect_settings`: the style and parameters the host draws with, and whether it shows the combo count. Not for the key path: the host reads it when the preferences change or a field gains focus, and draws each key from `msime_client_typing_effect`'s answer.
pub(crate) fn effect_settings(sound: &SessionSound) -> Value {
    let settings = &sound.settings;
    let mut value = serde_json::to_value(&settings.effect).unwrap_or(Value::Null);
    if let Some(fields) = value.as_object_mut() {
        fields.insert("combo_counter".into(), settings.combo_counter.into());
    }
    value
}

/// Queue the sound of one key. False when nothing was queued: the key sound is off, this platform does not play, sound failed earlier in this process, or the queue is full.
pub(crate) fn key(sound: &SessionSound, class: KeyClass) -> bool {
    if !sound.settings.key {
        sync(sound);
        return false;
    }
    deliver(sound, Event::Key(class))
}

/// Queue the sound of a commit: the pack's commit sample, the melody's next note, or both.
pub(crate) fn commit(sound: &SessionSound) -> bool {
    if !sound.settings.commit_sounds() {
        sync(sound);
        return false;
    }
    deliver(sound, Event::Commit)
}

/// Queue the tier-up sound of a combo that reached tier `tier`.
pub(crate) fn tier_up(sound: &SessionSound, tier: u8) -> bool {
    if !sound.settings.tier_sound() {
        return false;
    }
    deliver(sound, Event::TierUp(tier))
}

/// Tell the player whether music may play now. A running player hears it even while music is off, so music switched on later starts only once the host says the input method is active; with nothing switched on, no player is started for it.
pub(crate) fn music_active(sound: &SessionSound, active: bool) -> bool {
    deliver(sound, Event::Music(active))
}

/// Whether the process's sessions last asked for achievement sounds; the statistics recorder only reads the count before a commit when they did.
pub(crate) fn achievements_armed() -> bool {
    ACHIEVEMENTS.load(Ordering::Relaxed)
}

/// Queue the achievement sample under the settings sessions published last.
pub(crate) fn achievement() -> bool {
    let latest = LATEST
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let Some((settings, generation)) = latest else {
        return false;
    };
    if !settings.achievements {
        return false;
    }
    deliver(
        &SessionSound {
            settings,
            generation,
            combo: Cell::default(),
        },
        Event::Achievement,
    )
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
fn deliver(sound: &SessionSound, event: Event) -> bool {
    player::deliver(sound, event)
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
fn sync(sound: &SessionSound) {
    player::sync(sound);
}

#[cfg(any(target_os = "ios", target_os = "android", target_env = "ohos"))]
fn deliver(_sound: &SessionSound, _event: Event) -> bool {
    false
}

#[cfg(any(target_os = "ios", target_os = "android", target_env = "ohos"))]
fn sync(_sound: &SessionSound) {}

/// Steps through a melody note by note.
#[derive(Debug, Default)]
pub(crate) struct Melody {
    next: usize,
    last: Option<Instant>,
}

impl Melody {
    /// The semitone of the next note. The tune starts over at its end, and after `MELODY_IDLE_RESET_MILLIS` without a note, so a pause in typing begins it again.
    pub(crate) fn step(&mut self, semitones: &[i8], now: Instant) -> Option<i8> {
        if semitones.is_empty() {
            return None;
        }
        let idle = Duration::from_millis(sound_pack::MELODY_IDLE_RESET_MILLIS);
        if self.next >= semitones.len()
            || self
                .last
                .is_some_and(|last| now.saturating_duration_since(last) >= idle)
        {
            self.next = 0;
        }
        self.last = Some(now);
        let note = semitones[self.next];
        self.next += 1;
        Some(note)
    }
}

/// A 0-100 volume in decibels. The setting scales amplitude, so 50 is -6 dB; 0 and anything at or below kira's silence floor of -60 dB is silent.
pub(crate) fn decibels(volume: u8) -> f32 {
    const SILENCE: f32 = -60.0;
    if volume == 0 {
        return SILENCE;
    }
    (20.0 * (f32::from(volume.min(100)) / 100.0).log10()).max(SILENCE)
}

/// 一个校验过的音效包的文件，以绝对路径给出，供自己播放音效包的宿主使用。校验用的是 client-services 的，也就是 `scan` 列出这个包时用的同一套，所以宿主不会打开清单没有列出的文件。
pub(crate) fn pack_files(roots: &PluginRoots, id: &str) -> Result<Value, String> {
    let package = roots.load(PluginKind::Sound, id)?;
    let PluginContent::Sound(pack) = &package.content else {
        return Err("not a sound pack".into());
    };
    let path = |name: &Option<String>| {
        name.as_ref()
            .map(|name| package.directory.join(name).to_string_lossy().into_owned())
    };
    let sounds = &pack.sounds;
    Ok(json!({
        "id": package.id,
        "name": package.name,
        "license": package.license,
        "builtin": package.builtin,
        "mode": pack.mode,
        "sounds": {
            "default": path(&sounds.default),
            "space": path(&sounds.space),
            "enter": path(&sounds.enter),
            "backspace": path(&sounds.backspace),
            "commit": path(&sounds.commit),
            "achievement": path(&sounds.achievement),
        },
        "sequence": pack.sequence.as_ref().map(|sequence| json!({
            "sample": path(&Some(sequence.sample.clone())),
            "semitones": sequence.semitones,
            "advance": sequence.advance,
        })),
        "max_sample_millis": sound_pack::MAX_SAMPLE_MILLIS,
        "melody_idle_reset_millis": sound_pack::MELODY_IDLE_RESET_MILLIS,
    }))
}

/// 一个校验过的音乐包的曲目，以绝对路径按播放顺序给出，供自己流式播放音乐的宿主使用。这个包按 `scan` 列出它时用的同一套 client-services 规则检查；一首曲目最多能播多久是 `music_pack::MAX_TRACK_SECONDS`，宿主在播放每首曲目之前用它核对曲目时长，就像桌面解码器核对帧数那样。
pub(crate) fn music_pack_files(roots: &PluginRoots, id: &str) -> Result<Value, String> {
    let package = roots.load(PluginKind::Music, id)?;
    let PluginContent::Music(pack) = &package.content else {
        return Err("not a music pack".into());
    };
    let tracks: Vec<String> = pack
        .tracks
        .iter()
        .map(|track| package.directory.join(track).to_string_lossy().into_owned())
        .collect();
    Ok(json!({
        "id": package.id,
        "name": package.name,
        "license": package.license,
        "tracks": tracks,
        "max_track_seconds": music_pack::MAX_TRACK_SECONDS,
    }))
}

#[cfg(test)]
mod tests;
