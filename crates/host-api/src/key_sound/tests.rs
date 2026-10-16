//! Tests for the parts of the player that need no audio device: settings, the melody, pack files, bounded decoding and the sample table swap. Nothing here opens an output stream.

use super::*;
use msime_client_core::preferences::{KeySoundMode, PluginPreferences};

fn builtin_sounds() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/sound-packs")
}

fn builtin_roots() -> PluginRoots {
    PluginRoots {
        installed: None,
        builtin_sounds: Some(builtin_sounds()),
        builtin_packs: None,
    }
}

#[test]
fn key_classes_follow_the_abi_numbering() {
    let classes: Vec<_> = (0..5).map(KeyClass::from_code).collect();
    assert_eq!(
        classes,
        [
            Some(KeyClass::Default),
            Some(KeyClass::Space),
            Some(KeyClass::Enter),
            Some(KeyClass::Backspace),
            None
        ]
    );
    assert_eq!(
        KeyClass::ALL.map(KeyClass::name),
        ["default", "space", "enter", "backspace"]
    );
}

#[test]
fn melody_steps_through_the_tune_and_starts_over() {
    let tune = [0, 4, 7];
    let start = Instant::now();
    let mut melody = Melody::default();
    let at = |millis| start + Duration::from_millis(millis);
    let notes: Vec<_> = (0..5)
        .map(|index| melody.step(&tune, at(index * 100)))
        .collect();
    assert_eq!(notes, [Some(0), Some(4), Some(7), Some(0), Some(4)]);
    // Just under the idle reset carries on; a pause of the full length starts again.
    assert_eq!(melody.step(&tune, at(400 + 2_999)), Some(7));
    assert_eq!(melody.step(&tune, at(3_399 + 3_000)), Some(0));
    assert_eq!(melody.step(&[], at(7_000)), None);
}

#[test]
fn volume_scales_amplitude_down_to_silence() {
    assert_eq!(decibels(100), 0.0);
    assert!((decibels(50) + 6.0206).abs() < 0.001);
    assert!((decibels(1) + 40.0).abs() < 0.001);
    assert_eq!(decibels(0), -60.0);
    assert_eq!(decibels(250), 0.0);
}

#[test]
fn settings_follow_the_plugin_preferences() {
    let roots = builtin_roots();
    let defaults = SoundSettings::new(&PluginPreferences::default(), &roots);
    assert!(!defaults.wanted(), "everything is off in a fresh profile");
    assert_eq!(defaults.pack, "default");
    assert_eq!(defaults.melody_pack, "twinkle");
    assert_eq!(defaults.volume, 50);

    let mut preferences = PluginPreferences::default();
    preferences.music.enabled = true;
    let settings = SoundSettings::new(&preferences, &roots);
    assert!(!settings.music, "music without a chosen pack plays nothing");
    assert!(!settings.wanted());

    preferences.music.pack = "rain".into();
    preferences.key_sound.enabled = true;
    preferences.key_sound.mode = KeySoundMode::Melody;
    let settings = SoundSettings::new(&preferences, &roots);
    assert!(settings.music && settings.key && settings.melody && settings.wanted());
    assert!(settings.commit_sounds(), "a melody may advance on commits");

    preferences.key_sound.enabled = false;
    preferences.music.enabled = false;
    preferences.achievements.enabled = true;
    let settings = SoundSettings::new(&preferences, &roots);
    assert!(settings.wanted() && !settings.commit_sounds());
}

#[test]
fn a_session_moves_its_generation_only_when_its_settings_change() {
    let roots = builtin_roots();
    let mut preferences = PluginPreferences::default();
    let mut sound = SessionSound::new(SoundSettings::new(&preferences, &roots));
    let first = sound.generation;
    sound.update(SoundSettings::new(&preferences, &roots));
    assert_eq!(sound.generation, first);
    preferences.key_sound.volume = 80;
    sound.update(SoundSettings::new(&preferences, &roots));
    assert_ne!(sound.generation, first);
    assert_eq!(sound.settings.volume, 80);
}

#[test]
fn nothing_is_queued_while_every_sound_is_off() {
    let sound = SessionSound::new(SoundSettings::new(
        &PluginPreferences::default(),
        &builtin_roots(),
    ));
    assert!(!key(&sound, KeyClass::Default));
    assert!(!commit(&sound));
    assert!(!music_active(&sound, true));
}

#[test]
fn plugin_roots_come_from_the_state_root_and_the_bundle() {
    let directory = tempfile::tempdir().unwrap();
    let resources = directory.path().join("EngineResources");
    let state = directory.path().join("state");
    let roots = PluginRoots::new(state.to_str(), None, None, resources.to_str().unwrap());
    assert_eq!(roots.installed, Some(state.join("plugins")));
    assert_eq!(
        roots.builtin_sounds, None,
        "no sound-packs beside resources"
    );

    std::fs::create_dir_all(directory.path().join("sound-packs")).unwrap();
    let roots = PluginRoots::new(None, None, None, resources.to_str().unwrap());
    assert_eq!(roots.installed, None);
    assert_eq!(
        roots.builtin_sounds,
        Some(directory.path().join("sound-packs"))
    );

    let named = directory.path().join("data/sound-packs");
    let roots = PluginRoots::new(Some("relative"), named.to_str(), None, "relative");
    assert_eq!(roots.installed, None, "a relative state root names nothing");
    assert_eq!(roots.builtin_sounds, Some(named));
}

#[test]
fn official_plugin_packs_are_named_or_found_beside_the_sound_packs() {
    let directory = tempfile::tempdir().unwrap();
    let sounds = directory.path().join("Resources/sound-packs");
    std::fs::create_dir_all(&sounds).unwrap();
    // 移动端和不带官方包的开发构建：音效包旁边没有 plugin-packs，什么都不认。
    let roots = PluginRoots::new(None, sounds.to_str(), None, "");
    assert_eq!(roots.builtin_packs, None);
    assert_eq!(roots.builtin().packs, None);

    // macOS 的 bundle 和 Linux 的安装：只传 sound_packs，旁边的 plugin-packs 就是官方包。
    let packs = directory.path().join("Resources/plugin-packs");
    std::fs::create_dir_all(&packs).unwrap();
    let roots = PluginRoots::new(None, sounds.to_str(), None, "");
    assert_eq!(roots.builtin_packs, Some(packs.clone()));
    assert_eq!(roots.builtin().sounds, Some(sounds.as_path()));
    assert_eq!(roots.builtin().packs, Some(packs.as_path()));

    // 不传 sound_packs 时音效包按 resources 推出，官方包再从音效包推出。
    let resources = directory.path().join("Resources/EngineResources");
    let roots = PluginRoots::new(None, None, None, resources.to_str().unwrap());
    assert_eq!(roots.builtin_packs, Some(packs.clone()));

    // 显式给出的（Windows 的 Server 给 Program Files 下的目录）优先，不存在也照用，不再去音效包旁边找；相对路径什么都不认。
    let named = directory.path().join("server/plugin-packs");
    let roots = PluginRoots::new(None, sounds.to_str(), named.to_str(), "");
    assert_eq!(roots.builtin_packs, Some(named));
    let roots = PluginRoots::new(None, sounds.to_str(), Some("plugin-packs"), "");
    assert_eq!(roots.builtin_packs, None);
}

#[test]
fn pack_files_resolves_validated_files_to_absolute_paths() {
    let roots = builtin_roots();
    let keys = pack_files(&roots, "default").unwrap();
    assert_eq!(keys["mode"], "keys");
    assert_eq!(keys["builtin"], true);
    assert_eq!(keys["license"], "CC0-1.0");
    assert_eq!(keys["sequence"], Value::Null);
    for class in [
        "default",
        "space",
        "enter",
        "backspace",
        "commit",
        "achievement",
    ] {
        let path = PathBuf::from(keys["sounds"][class].as_str().unwrap());
        assert!(path.is_absolute() && path.is_file(), "{class}");
    }

    let melody = pack_files(&roots, "twinkle").unwrap();
    assert_eq!(melody["mode"], "sequence");
    assert_eq!(melody["sounds"]["default"], Value::Null);
    assert_eq!(melody["sequence"]["advance"], "key");
    assert_eq!(
        melody["sequence"]["semitones"].as_array().unwrap().len(),
        42
    );
    assert!(PathBuf::from(melody["sequence"]["sample"].as_str().unwrap()).is_file());
    assert_eq!(melody["max_sample_millis"], 1_500);

    assert!(
        pack_files(&roots, "typewriter").is_err(),
        "nothing installed"
    );
    assert!(pack_files(&roots, "../default").is_err());
    let no_bundle = PluginRoots::default();
    assert!(pack_files(&no_bundle, "default").is_err());
}

#[test]
fn pack_files_reads_installed_packs_from_the_state_root() {
    let state = tempfile::tempdir().unwrap();
    let pack = state.path().join("plugins/sound/typewriter");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::copy(
        builtin_sounds().join("default/key.wav"),
        pack.join("key.wav"),
    )
    .unwrap();
    std::fs::write(
        pack.join("plugin.toml"),
        "schema_version = 1\nkind = \"sound\"\nid = \"typewriter\"\nname = \"Typewriter\"\nversion = \"1\"\nlicense = \"CC0-1.0\"\n[sounds]\ndefault = \"key.wav\"\n",
    )
    .unwrap();
    let roots = PluginRoots::new(state.path().to_str(), None, None, "");
    let files = pack_files(&roots, "typewriter").unwrap();
    assert_eq!(files["builtin"], false);
    assert_eq!(
        files["sounds"]["default"],
        pack.join("key.wav").to_string_lossy().as_ref()
    );
    assert_eq!(files["sounds"]["space"], Value::Null);
}

fn install_effect(state: &Path, style: &str) {
    let pack = state.join("plugins/effect/neon");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("plugin.toml"),
        format!("schema_version = 1\nkind = \"effect\"\nid = \"neon\"\nname = \"Neon\"\nversion = \"1\"\nlicense = \"CC0-1.0\"\n[effect]\nstyle = \"{style}\"\nintensity = 80\ncolors = [\"#00FFCC\"]\nparticles = 12\n"),
    )
    .unwrap();
}

#[test]
fn the_selected_effect_pack_replaces_the_preferences_style_and_follows_a_reimport() {
    use msime_client_services::plugins::EffectStyle;

    let state = tempfile::tempdir().unwrap();
    let roots = PluginRoots::new(state.path().to_str(), None, None, "");
    let mut preferences = PluginPreferences {
        effect_style: EffectStyle::Flash,
        effect_intensity: 30,
        combo_counter: true,
        ..PluginPreferences::default()
    };

    // 没有特效包时使用偏好的样式和强度，其他参数取宿主默认值；固定强度的平台（macOS、Windows、鸿蒙）不读存下的 30，使用固定值。
    let intensity = crate::ffi::host::compiled_platform()
        .fixed_effect_intensity()
        .unwrap_or(30);
    let settings = SoundSettings::new(&preferences, &roots);
    assert!(settings.stamps.effect_pack.is_none());
    let value = effect_settings(&SessionSound::new(settings));
    assert_eq!(
        value,
        serde_json::json!({"pack": null, "issue": null, "style": "flash", "intensity": intensity, "colors": [], "duration_ms": null, "particles": null, "combo_counter": true})
    );

    // A pack that is not installed draws nothing and says why.
    preferences.effect_pack = "neon".into();
    let mut sound = SessionSound::new(SoundSettings::new(&preferences, &roots));
    let value = effect_settings(&sound);
    assert_eq!(value["pack"], "neon");
    assert_eq!(value["style"], "off");
    assert!(value["issue"].is_string());

    // Installed after the session started: the next focus-in picks it up, and its parameters replace the preferences'.
    install_effect(state.path(), "sparks");
    sound.restamp();
    let value = effect_settings(&sound);
    assert_eq!(value["issue"], Value::Null);
    assert_eq!(value["style"], "sparks");
    assert_eq!(value["intensity"], 80);
    assert_eq!(value["colors"], serde_json::json!(["#00FFCC"]));
    assert_eq!(value["particles"], 12);
    assert_eq!(value["duration_ms"], Value::Null);

    // Imported again with another style (a manifest of another length, so the stamp moves without waiting on the clock): the style follows.
    install_effect(state.path(), "power_mode");
    sound.restamp();
    assert_eq!(effect_settings(&sound)["style"], "power_mode");
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_env = "ohos")))]
mod playback {
    use super::super::player::{load, Request, Selection, Worker};
    use super::super::*;
    use super::{builtin_roots, builtin_sounds};
    use kira::{Decibels, PlaybackRate, Semitones, Value as Parameter};
    use std::io::Write;
    use std::sync::mpsc::{sync_channel, Receiver};

    /// A PCM16 WAV of `frames` frames of a quiet ramp.
    fn wav(path: &Path, channels: u16, rate: u32, frames: u32) {
        let data = frames * u32::from(channels) * 2;
        let mut bytes = Vec::with_capacity(44 + data as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data.to_le_bytes());
        for index in 0..frames * u32::from(channels) {
            bytes.extend_from_slice(&((index % 64) as i16 * 16).to_le_bytes());
        }
        std::fs::File::create(path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();
    }

    #[test]
    fn samples_decode_within_the_pack_bounds() {
        let key = decode::sample(&builtin_sounds().join("default/key.wav")).unwrap();
        assert_eq!((key.sample_rate, key.frames.len()), (44_100, 2_425));

        let directory = tempfile::tempdir().unwrap();
        let path = |name: &str| directory.path().join(name);
        wav(&path("stereo.wav"), 2, 48_000, 48_000);
        assert_eq!(
            decode::sample(&path("stereo.wav")).unwrap().frames.len(),
            48_000
        );
        // 1.5 s is the longest sample; at 8 kHz that is 12,000 frames in 24 KB, well inside the byte limit.
        wav(&path("longest.wav"), 1, 8_000, 12_000);
        assert!(decode::sample(&path("longest.wav")).is_ok());
        wav(&path("long.wav"), 1, 8_000, 12_001);
        assert!(decode::sample(&path("long.wav"))
            .unwrap_err()
            .contains("longer"));
        wav(&path("empty.wav"), 1, 44_100, 0);
        assert!(decode::sample(&path("empty.wav")).is_err());
        wav(&path("slow.wav"), 1, 4_000, 100);
        assert!(
            decode::sample(&path("slow.wav")).is_err(),
            "rate below 8 kHz"
        );
        wav(&path("surround.wav"), 3, 44_100, 100);
        assert!(decode::sample(&path("surround.wav")).is_err());
        std::fs::write(path("garbage.wav"), b"RIFF\0\0\0\0WAVEnothing here").unwrap();
        assert!(decode::sample(&path("garbage.wav")).is_err());
        let large = vec![0u8; sound_pack::MAX_SAMPLE_BYTES as usize + 1];
        std::fs::write(path("large.wav"), large).unwrap();
        assert!(decode::sample(&path("large.wav")).is_err());
    }

    #[test]
    fn tracks_must_declare_a_length_within_the_music_bound() {
        let directory = tempfile::tempdir().unwrap();
        let track = directory.path().join("track.wav");
        wav(&track, 1, 8_000, 4_000);
        let ended = Arc::new(AtomicBool::new(false));
        let mut decoder = decode::track(&track, Arc::clone(&ended)).unwrap();
        use kira::sound::streaming::Decoder as _;
        assert_eq!(
            (decoder.sample_rate(), decoder.num_frames()),
            (8_000, 4_000)
        );
        let mut decoded = 0;
        while !ended.load(Ordering::Acquire) {
            decoded += decoder.decode().unwrap().len();
            assert!(
                decoded <= 4_000 + 1_024 * 2,
                "silence after the end is bounded by the player"
            );
        }
        assert!(decoded >= 4_000);
        // A finished decoder hands out silence rather than an error kira would retry at full speed.
        let silence = decoder.decode().unwrap();
        assert!(!silence.is_empty() && silence.iter().all(|frame| *frame == kira::Frame::ZERO));
    }

    // kira seeks every stream to its start position when it opens it; that seek must not end the track.
    #[test]
    fn a_track_survives_the_seek_that_opens_it() {
        use kira::sound::streaming::{Decoder as _, StreamingSoundData};
        use kira::sound::PlaybackState;
        use kira::{AudioManager, AudioManagerSettings, Tween};
        let directory = tempfile::tempdir().unwrap();
        let track = directory.path().join("track.wav");
        // Longer than kira's stream buffer, so its decode thread cannot reach the end on its own.
        wav(&track, 1, 8_000, 8_000 * 30);
        let ended = Arc::new(AtomicBool::new(false));
        let mut decoder = decode::track(&track, Arc::clone(&ended)).unwrap();
        assert_eq!(decoder.seek(0).unwrap(), 0);
        assert!(!ended.load(Ordering::Acquire));
        assert!(!decoder.decode().unwrap().is_empty());
        assert!(!ended.load(Ordering::Acquire));

        let ended = Arc::new(AtomicBool::new(false));
        let decoder = decode::track(&track, Arc::clone(&ended)).unwrap();
        let mut manager =
            AudioManager::<kira::backend::mock::MockBackend>::new(AudioManagerSettings::default())
                .unwrap();
        let mut handle = manager
            .play(StreamingSoundData::from_decoder(decoder))
            .unwrap();
        assert!(
            !ended.load(Ordering::Acquire),
            "opening the stream ended it"
        );
        handle.stop(Tween::default());
        manager.backend_mut().on_start_processing();
        manager.backend_mut().process();
        assert_eq!(handle.state(), PlaybackState::Stopped);

        // Any other seek is unexpected and ends the track.
        let ended = Arc::new(AtomicBool::new(false));
        let mut decoder = decode::track(&track, Arc::clone(&ended)).unwrap();
        decoder.seek(100).unwrap();
        assert!(ended.load(Ordering::Acquire));
    }

    #[test]
    fn built_in_packs_load_into_a_sample_table() {
        assert!(load(&Selection::of(&keys_settings()).unwrap()).is_ok());
        let mut settings = keys_settings();
        settings.melody = true;
        settings.commit = false;
        let selection = Selection::of(&settings).unwrap();
        assert!(load(&selection).is_ok());
        settings.melody_pack = "default".into();
        let error = load(&Selection::of(&settings).unwrap()).err().unwrap();
        assert!(error.contains("not a melody"), "{error}");
        settings.melody_pack = "missing".into();
        assert!(load(&Selection::of(&settings).unwrap()).is_err());
        settings.key = false;
        settings.achievements = false;
        assert!(Selection::of(&settings).is_none(), "nothing to decode");
    }

    fn keys_settings() -> SoundSettings {
        SoundSettings {
            roots: builtin_roots(),
            key: true,
            melody: false,
            commit: true,
            achievements: true,
            pack: "default".into(),
            melody_pack: "twinkle".into(),
            volume: 50,
            music: false,
            music_pack: String::new(),
            music_volume: 30,
            ..SoundSettings::default()
        }
    }

    /// Configure the worker and hand it the loads that configuration started, newest last or first as asked.
    fn settle(worker: &mut Worker, receiver: &Receiver<Request>, loads: usize, newest_first: bool) {
        let mut results: Vec<Request> = (0..loads)
            .map(|_| receiver.recv_timeout(Duration::from_secs(20)).unwrap())
            .collect();
        results.sort_by_key(|request| match request {
            Request::Loaded(load, _) => *load,
            _ => 0,
        });
        if newest_first {
            results.reverse();
        }
        for request in results {
            worker.handle(request);
        }
    }

    fn frames(sounds: &[kira::sound::static_sound::StaticSoundData]) -> Vec<usize> {
        sounds.iter().map(|sound| sound.frames.len()).collect()
    }

    #[test]
    fn key_classes_and_events_pick_their_samples() {
        let (sender, receiver) = sync_channel(8);
        let mut worker = Worker::new(sender);
        let now = Instant::now();
        assert!(
            worker
                .sounds_for(Event::Key(KeyClass::Space), now)
                .is_empty(),
            "nothing loaded yet"
        );
        worker.configure(Arc::new(keys_settings()));
        settle(&mut worker, &receiver, 1, false);
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Default), now)),
            [2_425]
        );
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Space), now)),
            [3_969]
        );
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Enter), now)),
            [4_851]
        );
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Backspace), now)),
            [2_205]
        );
        assert_eq!(frames(&worker.sounds_for(Event::Commit, now)), [14_112]);
        let jingle = worker.sounds_for(Event::Achievement, now);
        assert_eq!(frames(&jingle), [55_125]);
        assert_eq!(
            jingle[0].settings.volume,
            Parameter::from(Decibels(decibels(50)))
        );

        // A volume change needs no reload; switching the commit sound off silences commits only.
        let mut quieter = keys_settings();
        quieter.volume = 10;
        quieter.commit = false;
        quieter.achievements = false;
        worker.configure(Arc::new(quieter));
        let key = worker.sounds_for(Event::Key(KeyClass::Default), now);
        assert_eq!(
            key[0].settings.volume,
            Parameter::from(Decibels(decibels(10)))
        );
        assert!(worker.sounds_for(Event::Commit, now).is_empty());
        assert!(worker.sounds_for(Event::Achievement, now).is_empty());
        assert!(
            receiver.try_recv().is_err(),
            "the same packs were not decoded again"
        );
    }

    #[test]
    fn keys_play_the_melody_note_by_note() {
        let (sender, receiver) = sync_channel(8);
        let mut worker = Worker::new(sender);
        let mut settings = keys_settings();
        settings.melody = true;
        worker.configure(Arc::new(settings));
        settle(&mut worker, &receiver, 1, false);
        let start = Instant::now();
        let rates: Vec<_> = (0..3)
            .map(|index| {
                let sounds = worker.sounds_for(
                    Event::Key(KeyClass::Default),
                    start + Duration::from_millis(index * 100),
                );
                assert_eq!(frames(&sounds), [35_280]);
                sounds[0].settings.playback_rate
            })
            .collect();
        let rate = |semitones: f64| Parameter::<PlaybackRate>::from(Semitones(semitones));
        assert_eq!(rates, [rate(0.0), rate(0.0), rate(7.0)]);
        // The tune advances on keys, so a commit plays only the commit sample.
        assert_eq!(
            frames(&worker.sounds_for(Event::Commit, start + Duration::from_millis(300))),
            [14_112]
        );
        let later = start + Duration::from_millis(300 + 3_000);
        let note = worker.sounds_for(Event::Key(KeyClass::Space), later);
        assert_eq!(
            note[0].settings.playback_rate,
            rate(0.0),
            "a pause starts the tune again"
        );
    }

    /// Install `typewriter` under `state` with `sample`, one of the default pack's files, as its key and commit sample.
    fn install_typewriter(state: &Path, sample: &str, name: &str) {
        let pack = state.join("plugins/sound/typewriter");
        if pack.exists() {
            std::fs::remove_dir_all(&pack).unwrap();
        }
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::copy(
            builtin_sounds().join("default").join(sample),
            pack.join("key.wav"),
        )
        .unwrap();
        std::fs::write(
            pack.join("plugin.toml"),
            format!("schema_version = 1\nkind = \"sound\"\nid = \"typewriter\"\nname = \"{name}\"\nversion = \"1\"\nlicense = \"CC0-1.0\"\n[sounds]\ndefault = \"key.wav\"\ncommit = \"key.wav\"\n"),
        )
        .unwrap();
    }

    #[test]
    fn a_pack_imported_again_or_removed_is_not_played_from_the_cache() {
        let state = tempfile::tempdir().unwrap();
        install_typewriter(state.path(), "key.wav", "Typewriter");
        let (sender, receiver) = sync_channel(8);
        let mut worker = Worker::new(sender);
        let mut settings = SoundSettings {
            roots: PluginRoots::new(state.path().to_str(), None, None, ""),
            key: true,
            pack: "typewriter".into(),
            volume: 50,
            ..SoundSettings::default()
        };
        settings.stamps = settings.stamp_packs();
        assert!(settings.stamps.pack.is_some());
        assert!(settings.stamps.melody_pack.is_none() && settings.stamps.music_pack.is_none());
        worker.configure(Arc::new(settings.clone()));
        settle(&mut worker, &receiver, 1, false);
        let now = Instant::now();
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Default), now)),
            [2_425]
        );

        // Nothing moved: the same settings decode nothing again.
        let restamped = SoundSettings {
            stamps: settings.stamp_packs(),
            ..settings.clone()
        };
        assert_eq!(restamped, settings);

        // Imported again under the same id with another sample: the next stamp differs and the new file is decoded.
        install_typewriter(state.path(), "space.wav", "Typewriter 2");
        let mut settings = SoundSettings {
            stamps: settings.stamp_packs(),
            ..settings
        };
        worker.configure(Arc::new(settings.clone()));
        settle(&mut worker, &receiver, 1, false);
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Default), now)),
            [3_969]
        );

        // Removed: the cached samples are dropped and keys fall silent rather than playing a pack that is gone.
        std::fs::remove_dir_all(state.path().join("plugins/sound/typewriter")).unwrap();
        settings.stamps = settings.stamp_packs();
        assert!(settings.stamps.pack.is_none());
        worker.configure(Arc::new(settings));
        settle(&mut worker, &receiver, 1, false);
        assert!(worker
            .sounds_for(Event::Key(KeyClass::Default), now)
            .is_empty());
    }

    #[test]
    fn a_tier_up_plays_the_commit_sample_higher_each_tier() {
        let (sender, receiver) = sync_channel(8);
        let mut worker = Worker::new(sender);
        // Only the combo uses the key pack, which is still loaded for it.
        let settings = SoundSettings {
            roots: builtin_roots(),
            pack: "default".into(),
            volume: 50,
            combo_counter: true,
            combo_tier_sound: true,
            ..SoundSettings::default()
        };
        assert!(settings.wanted() && settings.uses_key_pack());
        worker.configure(Arc::new(settings.clone()));
        settle(&mut worker, &receiver, 1, false);
        let now = Instant::now();
        assert!(
            worker.sounds_for(Event::Commit, now).is_empty(),
            "the commit sound is off"
        );
        let rate = |semitones: f64| Parameter::<PlaybackRate>::from(Semitones(semitones));
        for (tier, semitones) in [(1, 3.0), (2, 6.0), (3, 9.0), (4, 12.0)] {
            let sounds = worker.sounds_for(Event::TierUp(tier), now);
            assert_eq!(frames(&sounds), [14_112]);
            assert_eq!(sounds[0].settings.playback_rate, rate(semitones));
            assert_eq!(
                sounds[0].settings.volume,
                Parameter::from(Decibels(decibels(50)))
            );
        }
        let quiet = SoundSettings {
            combo_tier_sound: false,
            commit: true,
            ..settings
        };
        assert!(!quiet.tier_sound());
        worker.configure(Arc::new(quiet));
        assert!(worker.sounds_for(Event::TierUp(1), now).is_empty());
    }

    #[test]
    fn only_the_newest_load_is_swapped_in() {
        let (sender, receiver) = sync_channel(8);
        let mut worker = Worker::new(sender);
        worker.configure(Arc::new(keys_settings()));
        let mut melody = keys_settings();
        melody.melody = true;
        worker.configure(Arc::new(melody));
        // The older keys-mode table arrives last and must not replace the melody.
        settle(&mut worker, &receiver, 2, true);
        let sounds = worker.sounds_for(Event::Key(KeyClass::Default), Instant::now());
        assert_eq!(frames(&sounds), [35_280]);

        // Switching everything off forgets the table.
        let mut off = keys_settings();
        off.key = false;
        off.commit = false;
        off.achievements = false;
        worker.configure(Arc::new(off));
        worker.configure(Arc::new(keys_settings()));
        assert!(worker
            .sounds_for(Event::Key(KeyClass::Default), Instant::now())
            .is_empty());
        settle(&mut worker, &receiver, 1, false);
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Default), Instant::now())),
            [2_425]
        );
    }

    /// 在 `state` 下装一个名为 `id` 的包，`manifest` 是 `[sounds]` 或 `mode`/`[sequence]` 那几行。
    fn install_pack(state: &Path, id: &str, manifest: &str) -> PathBuf {
        let pack = state.join("plugins/sound").join(id);
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(
            pack.join("plugin.toml"),
            format!("schema_version = 1\nkind = \"sound\"\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1\"\nlicense = \"CC0-1.0\"\n{manifest}"),
        )
        .unwrap();
        pack
    }

    // 解码失败只让用到那个样本的键或声音不响（docs/plugins.md「超出的样本会被跳过，那个键不出声」），同一个包的其他样本和另选的旋律包照常播放；整个包解析不了时仍然全部静音。
    #[test]
    fn a_sample_that_does_not_decode_silences_only_its_own_sound() {
        let state = tempfile::tempdir().unwrap();
        let broken = install_pack(
            state.path(),
            "broken",
            "[sounds]\ndefault = \"long.wav\"\nspace = \"space.wav\"\ncommit = \"space.wav\"\nachievement = \"long.wav\"\n",
        );
        // client-services 导入时能过（文件头正确），解码器拒绝（8 kHz 下 2 秒，超过 1.5 秒上限）。
        wav(&broken.join("long.wav"), 1, 8_000, 16_000);
        std::fs::copy(
            builtin_sounds().join("default/space.wav"),
            broken.join("space.wav"),
        )
        .unwrap();
        let tune = install_pack(
            state.path(),
            "broken-tune",
            "mode = \"sequence\"\n[sequence]\nsample = \"tone.wav\"\nsemitones = [0, 7]\n",
        );
        wav(&tune.join("tone.wav"), 1, 8_000, 16_000);

        let (sender, receiver) = sync_channel(8);
        let mut worker = Worker::new(sender);
        let mut settings = keys_settings();
        settings.roots = PluginRoots::new(state.path().to_str(), None, None, "");
        settings.roots.builtin_sounds = Some(builtin_sounds());
        settings.pack = "broken".into();
        worker.configure(Arc::new(settings.clone()));
        settle(&mut worker, &receiver, 1, false);
        let now = Instant::now();
        for class in [KeyClass::Default, KeyClass::Enter, KeyClass::Backspace] {
            assert!(
                worker.sounds_for(Event::Key(class), now).is_empty(),
                "{class:?} falls back to the sample that does not decode"
            );
        }
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Space), now)),
            [3_969]
        );
        assert_eq!(frames(&worker.sounds_for(Event::Commit, now)), [3_969]);
        assert!(worker.sounds_for(Event::Achievement, now).is_empty());
        // 坏样本被三类键和成就音共用，只解码一次、只记一条。
        let skipped = load(&Selection::of(&settings).unwrap()).unwrap().skipped;
        assert_eq!(skipped.len(), 1, "{skipped:?}");
        assert!(skipped[0].starts_with("broken: long.wav: "), "{skipped:?}");

        // 按键包里坏掉的样本不连累另选的旋律包。
        settings.melody = true;
        worker.configure(Arc::new(settings.clone()));
        settle(&mut worker, &receiver, 1, false);
        assert_eq!(
            frames(&worker.sounds_for(Event::Key(KeyClass::Default), now)),
            [35_280]
        );
        assert_eq!(frames(&worker.sounds_for(Event::Commit, now)), [3_969]);

        // 旋律包的样本解码失败只让旋律不响，按键包的上屏音照常。
        settings.pack = "default".into();
        settings.melody_pack = "broken-tune".into();
        worker.configure(Arc::new(settings.clone()));
        settle(&mut worker, &receiver, 1, false);
        assert!(worker
            .sounds_for(Event::Key(KeyClass::Default), now)
            .is_empty());
        assert_eq!(frames(&worker.sounds_for(Event::Commit, now)), [14_112]);

        // 解析不了的包仍然整包静音。
        settings.melody = false;
        settings.pack = "missing".into();
        worker.configure(Arc::new(settings));
        settle(&mut worker, &receiver, 1, false);
        assert!(worker
            .sounds_for(Event::Key(KeyClass::Space), now)
            .is_empty());
        assert!(worker.sounds_for(Event::Commit, now).is_empty());
    }
}
