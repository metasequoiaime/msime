use super::*;
use msime_client_core::preferences::Preferences;

/// 合成的数据类清单：通用字段加上 `body`。
fn manifest(kind: &str, id: &str, name: &str, body: &str) -> String {
    format!("schema_version = 1\nkind = \"{kind}\"\nid = \"{id}\"\nname = \"{name}\"\nversion = \"1.0.0\"\nlicense = \"CC0-1.0\"\n{body}")
}

const PHRASES: &str = "[[phrases]]\nkey = \"qm\"\ntext = \"合成签名 someone@example.com\"\n\n[[phrases]]\nkey = \"dz\"\ntext = \"合成地址\"\n";

fn phrase_table(id: &str, name: &str, replace: bool) -> CreatePluginRequest {
    CreatePluginRequest {
        manifest: manifest("phrase_table", id, name, PHRASES),
        files: BTreeMap::new(),
        texts: BTreeMap::new(),
        replace,
    }
}

fn data_pack(manifest: String) -> CreatePluginRequest {
    CreatePluginRequest {
        manifest,
        files: BTreeMap::new(),
        texts: BTreeMap::new(),
        replace: false,
    }
}

/// 只有文件头的 WAV：导入只核对 RIFF/WAVE 文件头，时长由宿主播放前检查。
fn wav() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&36u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&22_050u32.to_le_bytes());
    bytes.extend_from_slice(&44_100u32.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes
}

fn defaults() -> Preferences {
    Preferences::default()
}

fn listed<'a>(list: &'a PluginList, kind: &str, id: &str) -> &'a PluginView {
    list.plugins
        .iter()
        .find(|plugin| plugin.kind == kind && plugin.id == id)
        .unwrap_or_else(|| panic!("{kind}/{id} is not listed: {list:?}"))
}

#[test]
fn every_data_kind_is_created_and_listed() {
    let state = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let sample = files.path().join("key.wav");
    std::fs::write(&sample, wav()).unwrap();
    let sample_path = sample.to_str().unwrap().to_owned();
    let preferences = defaults();
    let mut helpcode = data_pack(manifest(
        "helpcode",
        "my-codes",
        "我的辅助码",
        "[helpcode]\ntable = \"table.txt\"\n",
    ));
    helpcode.texts = BTreeMap::from([("table.txt".into(), "# 合成\n啊=a\n吧=ba\n".into())]);
    let mut wordbook = data_pack(manifest(
        "wordbook",
        "my-words",
        "我的单词",
        "[wordbook]\nfile = \"words.tsv\"\n",
    ));
    wordbook.texts = BTreeMap::from([(
        "words.tsv".into(),
        "cache\tn. 高速缓存\ncompiler\t/kəmˈpaɪlər/\tn. 编译器\n".into(),
    )]);
    let mut sound = data_pack(manifest(
        "sound",
        "clicks",
        "咔哒",
        "[sounds]\ndefault = \"key.wav\"\n",
    ));
    sound.files = BTreeMap::from([("key.wav".into(), sample_path.clone())]);
    sound.texts = BTreeMap::from([("LICENSE.txt".into(), "CC0-1.0\n".into())]);
    let mut music = data_pack(manifest(
        "music",
        "calm",
        "安静",
        "[music]\ntracks = [\"calm.wav\"]\n",
    ));
    music.files = BTreeMap::from([("calm.wav".into(), sample_path)]);
    let requests = [
        phrase_table("signature", "邮件签名", false),
        data_pack(manifest(
            "command_table",
            "today",
            "今天",
            "[[commands]]\ntrigger = \"today\"\ntitle = \"今天\"\ntemplate = \"{date:%Y-%m-%d} {weekday}\"\n",
        )),
        data_pack(manifest(
            "symbol_set",
            "arrows",
            "箭头",
            "[[groups]]\ntab = \"symbols\"\ntitle = \"箭头\"\nkeywords = \"jiantou arrow\"\nitems = [\"→\", \"←\", \"⇒\"]\n",
        )),
        data_pack(manifest(
            "effect",
            "sparkle",
            "火花",
            "[effect]\nstyle = \"sparks\"\nintensity = 70\n",
        )),
        helpcode,
        wordbook,
        sound,
        music,
    ];
    for request in &requests {
        let installed = create(state.path(), &preferences, request).unwrap();
        assert!(!installed.replaced);
        assert!(!installed.next_step.is_empty(), "{installed:?}");
        crate::assert_required_fields_present::<InstalledPlugin>(
            &serde_json::to_value(&installed).unwrap(),
        );
    }
    // 文件只被读，不被移动。
    assert_eq!(std::fs::read(&sample).unwrap(), wav());

    let list = list(state.path(), &BuiltinPaths::default(), &preferences);
    assert!(list.issues.is_empty(), "{:?}", list.issues);
    assert_eq!(list.plugins.len(), requests.len());
    let phrases = listed(&list, "phrase_table", "signature");
    assert_eq!(phrases.name, "邮件签名");
    assert_eq!(phrases.license, "CC0-1.0");
    assert!(!phrases.builtin && !phrases.official);
    assert_eq!(phrases.summary, "2 K-mode phrases under keys qm, dz");
    assert_eq!(phrases.active, Some(false));
    assert_eq!(
        listed(&list, "command_table", "today").summary,
        "1 / commands: /today"
    );
    assert_eq!(
        listed(&list, "symbol_set", "arrows").summary,
        "1 symbol groups: 箭头 (symbols, 3 items)"
    );
    assert_eq!(
        listed(&list, "effect", "sparkle").summary,
        "sparks typing effect at intensity 70"
    );
    assert_eq!(
        listed(&list, "helpcode", "my-codes").summary,
        "2 helpcode entries in table.txt"
    );
    let words = listed(&list, "wordbook", "my-words");
    assert_eq!(words.summary, "2 words in words.tsv");
    // 单词本装上就能用。
    assert_eq!(words.active, Some(true));
    assert_eq!(
        listed(&list, "sound", "clicks").summary,
        "key sounds from 1 sample files"
    );
    assert_eq!(listed(&list, "music", "calm").summary, "1 music tracks");
    assert!(state
        .path()
        .join("plugins/sound/clicks/LICENSE.txt")
        .is_file());

    let value = serde_json::to_value(&list).unwrap();
    crate::assert_required_fields_present::<PluginList>(&value);
    crate::assert_required_fields_present::<PluginView>(&value["plugins"][0]);
    crate::assert_required_fields_present::<PluginSelection>(&value["selection"]);
}

#[test]
fn the_next_step_says_how_to_turn_a_pack_on() {
    let state = tempfile::tempdir().unwrap();
    let installed = create(
        state.path(),
        &defaults(),
        &phrase_table("signature", "邮件签名", false),
    )
    .unwrap();
    assert!(installed.next_step.contains("启用"), "{installed:?}");
    assert!(installed.next_step.contains("Shift+K"), "{installed:?}");

    let mut off = defaults();
    off.local_modes.command = false;
    let installed = create(
        state.path(),
        &off,
        &data_pack(manifest(
            "command_table",
            "today",
            "今天",
            "[[commands]]\ntrigger = \"today\"\ntitle = \"今天\"\ntemplate = \"{date}\"\n",
        )),
    )
    .unwrap();
    assert!(
        installed.next_step.contains("指令(/ 模式)"),
        "{installed:?}"
    );

    // 已经启用的短语表被替换时，说改动几秒内生效，而不是让用户再去打开。
    let mut enabled = defaults();
    enabled.plugins.phrase_tables = vec!["signature".into()];
    let replaced = create(
        state.path(),
        &enabled,
        &phrase_table("signature", "新签名", true),
    )
    .unwrap();
    assert!(replaced.replaced);
    assert!(
        replaced.next_step.contains("already in use"),
        "{replaced:?}"
    );
}

#[test]
fn the_selection_reports_the_plugin_preferences() {
    let state = tempfile::tempdir().unwrap();
    create(
        state.path(),
        &defaults(),
        &phrase_table("signature", "邮件签名", false),
    )
    .unwrap();
    let mut preferences = defaults();
    preferences.plugins.phrase_tables = vec!["signature".into()];
    preferences.plugins.helpcode_pack_quanpin = "my-codes".into();
    preferences.local_modes.command = true;
    let list = list(state.path(), &BuiltinPaths::default(), &preferences);
    assert_eq!(list.selection.phrase_tables, ["signature"]);
    assert_eq!(
        list.selection.helpcode_pack_quanpin.as_deref(),
        Some("my-codes")
    );
    assert_eq!(list.selection.helpcode_pack_shuangpin, None);
    assert_eq!(list.selection.effect_pack, None);
    assert!(list.selection.command_mode);
    assert_eq!(
        listed(&list, "phrase_table", "signature").active,
        Some(true)
    );
}

#[test]
fn a_refused_manifest_says_what_is_wrong() {
    let state = tempfile::tempdir().unwrap();
    let preferences = defaults();
    let refused =
        |request: &CreatePluginRequest| create(state.path(), &preferences, request).unwrap_err();

    let mut unknown = phrase_table("signature", "邮件签名", false);
    unknown.manifest = unknown
        .manifest
        .replace("license", "colour = \"red\"\nlicense");
    let error = refused(&unknown);
    assert!(error.contains("未知的键 colour"), "{error}");

    let missing = data_pack(manifest(
        "helpcode",
        "my-codes",
        "辅助码",
        "[helpcode]\ntable = \"table.txt\"\n",
    ));
    let error = refused(&missing);
    assert!(error.contains("缺少数据文件 table.txt"), "{error}");

    let mut uppercase = phrase_table("signature", "邮件签名", false);
    uppercase.manifest = uppercase.manifest.replace("key = \"qm\"", "key = \"QM\"");
    let error = refused(&uppercase);
    assert!(error.starts_with("the pack was refused: "), "{error}");

    let error = refused(&phrase_table("latex-symbols", "LaTeX", false));
    assert!(error.contains("ships with the input method"), "{error}");

    let mut permissions = phrase_table("signature", "邮件签名", false);
    permissions.manifest = permissions
        .manifest
        .replace("license", "permissions = [\"read_commit\"]\nlicense");
    let error = refused(&permissions);
    assert!(error.contains("permissions"), "{error}");

    assert!(list(state.path(), &BuiltinPaths::default(), &preferences)
        .plugins
        .is_empty());
}

#[test]
fn an_installed_pack_is_replaced_only_when_asked() {
    let state = tempfile::tempdir().unwrap();
    let preferences = defaults();
    create(
        state.path(),
        &preferences,
        &phrase_table("signature", "邮件签名", false),
    )
    .unwrap();
    let error = create(
        state.path(),
        &preferences,
        &phrase_table("signature", "新签名", false),
    )
    .unwrap_err();
    assert!(error.contains("replace: true"), "{error}");
    let name = || {
        list(state.path(), &BuiltinPaths::default(), &preferences).plugins[0]
            .name
            .clone()
    };
    assert_eq!(name(), "邮件签名");
    let replaced = create(
        state.path(),
        &preferences,
        &phrase_table("signature", "新签名", true),
    )
    .unwrap();
    assert!(replaced.replaced);
    assert_eq!(name(), "新签名");
    // replace 不要求已经装着。
    let fresh = create(
        state.path(),
        &preferences,
        &phrase_table("other", "另一个", true),
    )
    .unwrap();
    assert!(!fresh.replaced);
}

#[test]
fn an_installed_pack_is_read_back_and_recreated_from_its_own_files() {
    let state = tempfile::tempdir().unwrap();
    let preferences = defaults();
    let table = "啊=a\n吧=ba\n";
    let body = "[helpcode]\ntable = \"table.txt\"\n";
    let mut request = data_pack(manifest("helpcode", "my-codes", "辅助码", body));
    request.texts = BTreeMap::from([("table.txt".into(), table.into())]);
    create(state.path(), &preferences, &request).unwrap();
    let pack = PluginRequest {
        kind: "helpcode".into(),
        id: "my-codes".into(),
    };
    let files = get(state.path(), &BuiltinPaths::default(), &pack).unwrap();
    // 逐段拼接：Windows 上读回的路径用反斜杠分隔。
    let directory = state
        .path()
        .join("plugins")
        .join("helpcode")
        .join("my-codes");
    assert_eq!(
        files,
        PluginFiles {
            kind: "helpcode".into(),
            id: "my-codes".into(),
            builtin: false,
            manifest: manifest("helpcode", "my-codes", "辅助码", body),
            directory: directory.display().to_string(),
            files: vec![PluginFileView {
                name: "table.txt".into(),
                file: directory.join("table.txt").display().to_string(),
                bytes: table.len() as u64,
            }],
        }
    );
    let value = serde_json::to_value(&files).unwrap();
    crate::assert_required_fields_present::<PluginFiles>(&value);
    crate::assert_required_fields_present::<PluginFileView>(&value["files"][0]);

    // 改了清单，文件原样指回已安装的那份，整包替换：替换之前每个文件都已读进来。
    let mut edited = data_pack(files.manifest.replace("辅助码", "我的辅助码"));
    edited.files = files
        .files
        .iter()
        .map(|file| (file.name.clone(), file.file.clone()))
        .collect();
    edited.replace = true;
    assert!(
        create(state.path(), &preferences, &edited)
            .unwrap()
            .replaced
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("table.txt")).unwrap(),
        table
    );
    assert_eq!(
        list(state.path(), &BuiltinPaths::default(), &preferences).plugins[0].name,
        "我的辅助码"
    );

    let get_error = |kind: &str, id: &str| {
        get(
            state.path(),
            &BuiltinPaths::default(),
            &PluginRequest {
                kind: kind.into(),
                id: id.into(),
            },
        )
        .unwrap_err()
    };
    let error = get_error("helpcode", "missing");
    assert!(
        error.contains("no helpcode pack missing is installed"),
        "{error}"
    );
    let error = get_error("theme", "my-codes");
    assert!(error.contains("is not a plugin kind"), "{error}");
    std::fs::create_dir_all(state.path().join("plugins/helpcode/broken")).unwrap();
    let error = get_error("helpcode", "broken");
    assert!(error.contains("under issues"), "{error}");
}

#[test]
fn unsafe_files_are_refused_before_anything_is_installed() {
    let state = tempfile::tempdir().unwrap();
    let files = tempfile::tempdir().unwrap();
    let preferences = defaults();
    let sound = |name: &str, file: &str| {
        let mut request = data_pack(manifest(
            "sound",
            "clicks",
            "咔哒",
            "[sounds]\ndefault = \"key.wav\"\n",
        ));
        request.files = BTreeMap::from([(name.into(), file.into())]);
        request
    };
    let refused =
        |request: &CreatePluginRequest| create(state.path(), &preferences, request).unwrap_err();

    let error = refused(&sound("key.wav", "key.wav"));
    assert!(error.contains("is not an absolute path"), "{error}");
    let missing = files.path().join("missing.wav");
    let error = refused(&sound("key.wav", missing.to_str().unwrap()));
    assert!(error.contains("does not exist"), "{error}");
    let error = refused(&sound("key.wav", files.path().to_str().unwrap()));
    assert!(error.contains("is not a regular file"), "{error}");

    // 超过上限的文件在读之前就拒绝；稀疏文件不占盘。
    let huge = files.path().join("huge.wav");
    std::fs::File::create(&huge)
        .unwrap()
        .set_len(MAX_FILE_BYTES + 1)
        .unwrap();
    let error = refused(&sound("key.wav", huge.to_str().unwrap()));
    assert!(error.contains("larger than 16 MiB"), "{error}");

    let real = files.path().join("key.wav");
    std::fs::write(&real, wav()).unwrap();
    let real = real.to_str().unwrap();
    for name in ["../key.wav", "sub/key.wav", ".hidden.wav", "plugin.toml"] {
        let error = refused(&sound(name, real));
        assert!(
            error.contains("is not a file name") || error.contains("given as manifest"),
            "{name}: {error}"
        );
    }
    let mut both = sound("key.wav", real);
    both.texts = BTreeMap::from([("key.wav".into(), "not audio".into())]);
    let error = refused(&both);
    assert!(error.contains("only one of the two"), "{error}");

    #[cfg(unix)]
    {
        let link = files.path().join("link.wav");
        std::os::unix::fs::symlink(real, &link).unwrap();
        let error = refused(&sound("key.wav", link.to_str().unwrap()));
        assert!(error.contains("is a symbolic link"), "{error}");
        // 符号链接在上一级目录时也拒绝。
        let linked = files.path().join("linked");
        std::os::unix::fs::symlink(files.path(), &linked).unwrap();
        // root 在只有 root 能写的目录里建的链接，Linux 上算系统链接（`msime_path_trust::is_root_only_link`），以 root 运行的测试容器里跳过这一项。
        use std::os::unix::fs::MetadataExt;
        if std::fs::symlink_metadata(&linked).unwrap().uid() != 0 {
            let through = linked.join("key.wav");
            let error = refused(&sound("key.wav", through.to_str().unwrap()));
            assert!(error.contains("symbolic link on the way"), "{error}");
        }
    }
    assert!(list(state.path(), &BuiltinPaths::default(), &preferences)
        .plugins
        .is_empty());

    // 同一个文件经正常路径给出就能装。
    create(state.path(), &preferences, &sound("key.wav", real)).unwrap();
}

#[test]
fn a_script_plugin_is_refused_by_create_plugin() {
    let state = tempfile::tempdir().unwrap();
    let error = create(
        state.path(),
        &defaults(),
        &data_pack("schema_version = 2\nkind = 'script'\nid = 'pangu'\nname = '盘古'\nversion = '1.0.0'\nlicense = 'MIT'\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['commit']\n".into()),
    )
    .unwrap_err();
    assert!(error.contains("install_plugin_folder"), "{error}");
    assert!(!state.path().join("plugins").exists());
}

/// 最小的 ABI v1 模块：一页内存导出为 `memory`，`msime_abi` 返回 1，`msime_on_event` 返回 0，不导入任何函数，与 client-services 的 `script_tests::module(&[])` 字节相同。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
const MODULE: [u8; 84] = [
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0a, 0x02, 0x60, 0x00, 0x01, 0x7f, 0x60,
    0x01, 0x7f, 0x01, 0x7f, 0x03, 0x03, 0x02, 0x00, 0x01, 0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x27,
    0x03, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00, 0x09, 0x6d, 0x73, 0x69, 0x6d, 0x65,
    0x5f, 0x61, 0x62, 0x69, 0x00, 0x00, 0x0e, 0x6d, 0x73, 0x69, 0x6d, 0x65, 0x5f, 0x6f, 0x6e, 0x5f,
    0x65, 0x76, 0x65, 0x6e, 0x74, 0x00, 0x01, 0x0a, 0x0b, 0x02, 0x04, 0x00, 0x41, 0x01, 0x0b, 0x04,
    0x00, 0x41, 0x00, 0x0b,
];

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
#[test]
fn a_script_plugin_folder_is_installed_but_not_granted() {
    let state = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let folder = source.path().join("pangu");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(
        folder.join("plugin.toml"),
        "schema_version = 2\nkind = 'script'\nid = 'pangu'\nname = '盘古'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = []\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['commit']\n",
    )
    .unwrap();
    std::fs::write(folder.join("main.wasm"), MODULE).unwrap();
    let preferences = defaults();
    let request = |replace: bool| InstallPluginRequest {
        path: folder.to_str().unwrap().into(),
        replace,
    };
    let installed = install_folder(state.path(), &preferences, &request(false)).unwrap();
    assert_eq!(installed.kind, "script");
    assert_eq!(installed.id, "pangu");
    assert!(
        installed.next_step.contains("脚本插件（实验）"),
        "{installed:?}"
    );
    assert!(
        installed.next_step.contains("never grants"),
        "{installed:?}"
    );
    let list = list(state.path(), &BuiltinPaths::default(), &preferences);
    let script = listed(&list, "script", "pangu");
    assert_eq!(script.active, None);
    assert!(script.summary.starts_with("script plugin"), "{script:?}");

    let error = install_folder(state.path(), &preferences, &request(false)).unwrap_err();
    assert!(error.contains("replace: true"), "{error}");
    assert!(
        install_folder(state.path(), &preferences, &request(true))
            .unwrap()
            .replaced
    );

    // 删除脚本插件时它的授权条目一起去掉，由 client-services 的 `remove` 负责。
    delete(
        state.path(),
        &preferences,
        &PluginRequest {
            kind: "script".into(),
            id: "pangu".into(),
        },
    )
    .unwrap();
    assert!(!state.path().join("plugins/script/pangu").exists());
}

#[test]
fn a_data_pack_folder_is_installed_by_path() {
    let state = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    let folder = source.path().join("arrows");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(
        folder.join("plugin.toml"),
        manifest(
            "symbol_set",
            "arrows",
            "箭头",
            "[[groups]]\ntab = \"symbols\"\ntitle = \"箭头\"\nitems = [\"→\"]\n",
        ),
    )
    .unwrap();
    let install = |path: &str| {
        install_folder(
            state.path(),
            &defaults(),
            &InstallPluginRequest {
                path: path.into(),
                replace: false,
            },
        )
    };
    let installed = install(folder.to_str().unwrap()).unwrap();
    assert_eq!(installed.kind, "symbol_set");
    assert!(installed.next_step.contains("ready"), "{installed:?}");

    let error = install("arrows").unwrap_err();
    assert!(error.contains("not an absolute path"), "{error}");
    let missing = source.path().join("missing");
    let error = install(missing.to_str().unwrap()).unwrap_err();
    assert!(
        error.contains("the path must be a pack folder or a .zip file"),
        "{error}"
    );
}

#[test]
fn deleting_refuses_packs_that_ship_with_the_input_method() {
    let state = tempfile::tempdir().unwrap();
    let preferences = defaults();
    create(
        state.path(),
        &preferences,
        &phrase_table("signature", "邮件签名", false),
    )
    .unwrap();
    let delete = |kind: &str, id: &str, preferences: &Preferences| {
        delete(
            state.path(),
            preferences,
            &PluginRequest {
                kind: kind.into(),
                id: id.into(),
            },
        )
    };
    for (kind, id, expected) in [
        ("sound", "default", "ships with the input method"),
        ("music", "msime-music-lofi", "ships with the input method"),
        (
            "phrase_table",
            "latex-symbols",
            "ships with the input method",
        ),
        (
            "phrase_table",
            "missing",
            "no phrase_table pack missing is installed",
        ),
        ("phrase_table", "../signature", "is not a plugin id"),
        ("theme", "signature", "is not a plugin kind"),
    ] {
        let error = delete(kind, id, &preferences).unwrap_err();
        assert!(error.contains(expected), "{kind}/{id}: {error}");
    }
    assert!(state.path().join("plugins/phrase_table/signature").is_dir());

    // 偏好里还启用着它时照删，结果说出是哪一项，由输入法回到默认。
    let mut enabled = defaults();
    enabled.plugins.phrase_tables = vec!["signature".into()];
    let deleted = delete("phrase_table", "signature", &enabled).unwrap();
    assert_eq!(
        deleted,
        DeletedPlugin {
            kind: "phrase_table".into(),
            id: "signature".into(),
            still_selected_in: vec!["plugins.phrase_tables".into()],
        }
    );
    crate::assert_required_fields_present::<DeletedPlugin>(
        &serde_json::to_value(&deleted).unwrap(),
    );
    assert!(!state.path().join("plugins/phrase_table/signature").exists());
    let error = delete("phrase_table", "signature", &enabled).unwrap_err();
    assert!(error.contains("is installed"), "{error}");
}

#[test]
fn official_packs_from_the_runtime_options_are_listed_read_only() {
    let state = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let official = packs.path().join("phrase_table/latex-symbols");
    std::fs::create_dir_all(&official).unwrap();
    std::fs::write(
        official.join("plugin.toml"),
        manifest(
            "phrase_table",
            "latex-symbols",
            "LaTeX 符号",
            "[[phrases]]\nkey = \"alpha\"\ntext = \"α\"\n",
        ),
    )
    .unwrap();
    let builtin = BuiltinPaths::from_options(&serde_json::json!({
        "resources": "/nonexistent/resources",
        "plugin_packs": packs.path(),
    }));
    let list = list(state.path(), &builtin, &defaults());
    let latex = listed(&list, "phrase_table", "latex-symbols");
    assert!(latex.builtin && latex.official);
    let files = get(
        state.path(),
        &builtin,
        &PluginRequest {
            kind: "phrase_table".into(),
            id: "latex-symbols".into(),
        },
    )
    .unwrap();
    assert!(files.builtin);
    assert!(files.manifest.contains("alpha"));
    // 运行时选项里的相对路径不算数，没有内置目录时只列用户的包。
    assert_eq!(
        BuiltinPaths::from_options(&serde_json::json!({ "plugin_packs": "plugin-packs" })),
        BuiltinPaths::default()
    );
}
