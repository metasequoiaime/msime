//! 对共享状态目录没有写权限的宿主（iOS 键盘没有完全访问，#10945）：只读准备、只读租约、会话不写共享目录。
//!
//! 共享目录用权限位改成只读来模拟 App Group 的 `EPERM`（这里得到的是 `EACCES`，两者都是 `PermissionDenied`）。以 root 跑时权限位挡不住写，这几条用例什么也不测、直接返回。

use super::*;
use std::os::unix::fs::PermissionsExt;

fn set_tree_writable(path: &Path, writable: bool) {
    let metadata = std::fs::symlink_metadata(path).unwrap();
    if metadata.file_type().is_symlink() {
        return;
    }
    let mode = match (metadata.is_dir(), writable) {
        (true, true) => 0o755,
        (true, false) => 0o555,
        (false, true) => 0o644,
        (false, false) => 0o444,
    };
    if metadata.is_dir() && writable {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            set_tree_writable(&entry.unwrap().path(), writable);
        }
    }
    if !(metadata.is_dir() && writable) {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
}

/// 把 `state` 整棵改成只读，返回权限位是否真挡得住写（root 挡不住）。结束时由 [`ReadOnlyTree`] 改回来，好让临时目录删得掉。
struct ReadOnlyTree<'a>(&'a Path);

impl<'a> ReadOnlyTree<'a> {
    fn new(state: &'a Path) -> (Self, bool) {
        set_tree_writable(state, false);
        let enforced = std::fs::File::create(state.join("probe")).is_err();
        (Self(state), enforced)
    }
}

impl Drop for ReadOnlyTree<'_> {
    fn drop(&mut self) {
        set_tree_writable(self.0, true);
    }
}

/// 目录树里每个条目的路径、大小和修改时间，用来确认只读宿主一个字节也没写。
fn tree_listing(root: &Path) -> Vec<(PathBuf, u64, Option<std::time::SystemTime>)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                pending.push(entry.unwrap().path());
            }
        }
        entries.push((path, metadata.len(), metadata.modified().ok()));
    }
    entries.sort();
    entries
}

/// 文档里的路径与期望的路径按真实位置比较：macOS 的临时目录在 `/var` 下，规范化后在 `/private/var` 下。
fn canonical(path: &Value) -> PathBuf {
    let path = Path::new(path.as_str().unwrap());
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn prepare_read_only(resources: &Path, state: &Path, private: &Path) -> Value {
    let request = json!({
        "resources": resources,
        "state_root": state,
        "read_only_state": true,
        "private_root": private,
    })
    .to_string();
    read(unsafe { msime_client_prepare_host(request.as_ptr(), request.len()) })
}

fn create(options: &Value) -> Value {
    let text = options.to_string();
    read(unsafe { msime_client_create(text.as_ptr(), text.len()) })
}

/// 共享目录里有 App 准备好的代次时，只读宿主用它：文档指向共享的用户目录和代次，缓存和校验标记在私有目录，会话持着只读租约挡住维护，整个过程共享目录一个字节也没变。
#[test]
fn a_read_only_host_runs_on_the_generation_the_app_prepared_and_writes_nothing() {
    let root = tempfile::tempdir().unwrap();
    let app = root.path().join("app/EngineResources");
    let keyboard = root.path().join("keyboard/EngineResources");
    let specification = synthetic_desktop_lock(&app);
    synthetic_desktop_lock(&keyboard);
    let state = root.path().join("group/MSIME");
    let private = root.path().join("keyboard-container/MSIME");
    prepare_shipped_host_configuration(&app, &state, &specification, &[], Edition::full()).unwrap();
    let generation = state
        .join("user")
        .join("dictionaries")
        .join(specification.generation().unwrap());
    let before = tree_listing(&state);

    let (guard, enforced) = ReadOnlyTree::new(&state);
    if !enforced {
        return;
    }
    // 改前的准备在这里就失败：偏好锁、第一份偏好和日志回放都要写共享目录。
    assert!(prepare_host_configuration(&keyboard, &state).is_err());
    let document = crate::read_only_state::prepare_on(
        &keyboard,
        &state,
        &private,
        &specification,
        &[],
        Edition::full(),
    )
    .unwrap();
    assert_eq!(document["read_only_state"], true, "{document}");
    assert_eq!(
        canonical(&document["dictionaries"]),
        canonical(&json!(generation))
    );
    assert_eq!(
        canonical(&document["user_data"]),
        canonical(&json!(state.join("user")))
    );
    assert!(canonical(&document["cache"]).starts_with(canonical(&json!(private))));
    assert_eq!(
        canonical(&document["preferences_directory"]),
        canonical(&json!(state))
    );
    assert!(private.join("verified-resources.json").is_file());

    let created = create(&document);
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    for byte in b"nihao" {
        let typed = read(msime_client_character(handle, *byte, false));
        assert_eq!(typed["ok"], true, "{typed}");
    }
    // 只读打开的锁文件取到的是同一把共享锁：App 的维护（编辑个人词库、重置学习、删旧代次）取独占锁，被挡住。目录这时是只读的，维护本身打不开锁文件，这里直接对两把锁文件试独占 flock。
    let maintenance_possible = || {
        [state.join("user"), generation.clone()].iter().all(|root| {
            let file = msime_client_core::file_lock::open_existing_lock_file_read_only(
                root.join(msime_engine::access::LOCK_FILE),
            )
            .unwrap();
            msime_client_core::file_lock::ScopedFileLock::try_exclusive(file)
                .unwrap()
                .is_some()
        })
    };
    assert!(!maintenance_possible());
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    assert!(maintenance_possible());
    drop(guard);
    assert_eq!(tree_listing(&state), before);
}

/// App 还没为这个版本准备过代次（或者共享目录根本不在）时，只读宿主退回只读布局：词库就是自己随包的资源，用户目录和缓存在私有目录，照样建得出会话，共享目录不变。
#[test]
fn a_read_only_host_without_a_prepared_generation_uses_its_bundled_dictionaries() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard/EngineResources");
    let specification = synthetic_desktop_lock(&keyboard);
    let state = root.path().join("group/MSIME");
    std::fs::create_dir_all(&state).unwrap();
    // App 写过偏好，但没准备过代次。
    let preferences = Preferences {
        chinese_punctuation: false,
        ..Preferences::default()
    };
    PreferencesStore::new(&state).save(0, preferences).unwrap();
    let private = root.path().join("keyboard-container/MSIME");
    let before = tree_listing(&state);
    let (guard, enforced) = ReadOnlyTree::new(&state);
    if !enforced {
        return;
    }
    let document = crate::read_only_state::prepare_on(
        &keyboard,
        &state,
        &private,
        &specification,
        &[],
        Edition::full(),
    )
    .unwrap();
    let bundled = canonical(&json!(keyboard));
    assert_eq!(canonical(&document["dictionaries"]), bundled);
    assert_eq!(canonical(&document["resources"]), bundled);
    assert!(canonical(&document["user_data"]).starts_with(canonical(&json!(private))));
    // 偏好照样读的是 App 写的那份。
    assert_eq!(document["preferences"]["chinese_punctuation"], false);
    let created = create(&document);
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(
        read(msime_client_character(handle, b'n', false))["ok"],
        true
    );
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    drop(guard);
    assert_eq!(tree_listing(&state), before);

    // 共享目录根本不在（App 从没运行过）也一样。
    let absent = root.path().join("absent-group/MSIME");
    let document = crate::read_only_state::prepare_on(
        &keyboard,
        &absent,
        &private,
        &specification,
        &[],
        Edition::full(),
    )
    .unwrap();
    assert_eq!(canonical(&document["dictionaries"]), bundled);
    assert!(!absent.exists());
}

/// 代次在，但锁文件是旧版本的 App 留下的那种从没建过的状态时，只读宿主拿不到租约，不能不持锁去读一个可能被换掉的代次，退回随包词库。
#[test]
fn a_read_only_host_does_not_read_a_shared_generation_it_cannot_lease() {
    let root = tempfile::tempdir().unwrap();
    let app = root.path().join("app/EngineResources");
    let specification = synthetic_desktop_lock(&app);
    let state = root.path().join("group/MSIME");
    let private = root.path().join("keyboard-container/MSIME");
    prepare_shipped_host_configuration(&app, &state, &specification, &[], Edition::full()).unwrap();
    let generation = state
        .join("user")
        .join("dictionaries")
        .join(specification.generation().unwrap());
    // 可写的准备现在就建好两把锁文件。
    assert!(state
        .join("user")
        .join(msime_engine::access::LOCK_FILE)
        .is_file());
    assert!(generation.join(msime_engine::access::LOCK_FILE).is_file());
    std::fs::remove_file(generation.join(msime_engine::access::LOCK_FILE)).unwrap();
    let (_guard, enforced) = ReadOnlyTree::new(&state);
    if !enforced {
        return;
    }
    let document = crate::read_only_state::prepare_on(
        &app,
        &state,
        &private,
        &specification,
        &[],
        Edition::full(),
    )
    .unwrap();
    assert_eq!(canonical(&document["dictionaries"]), canonical(&json!(app)));
}

/// 只读的文档里学习和回车学英文词开着也不学：会话的 Engine 选项、偏好变了之后重建的 Engine 都不打开它们。
#[test]
fn a_read_only_session_never_learns() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard/EngineResources");
    let specification = synthetic_desktop_lock(&keyboard);
    let state = root.path().join("group/MSIME");
    let private = root.path().join("keyboard-container/MSIME");
    let mut document = crate::read_only_state::prepare_on(
        &keyboard,
        &state,
        &private,
        &specification,
        &[],
        Edition::full(),
    )
    .unwrap();
    document["preferences"]["learning"] = json!(true);
    document["preferences"]["enter_learns_english_word"] = json!(true);
    let created = create(&document);
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    // 学习和回车学英文词两个开关都关着：后者学的词写进代次里的英文词库，只读布局下就是随包资源。
    let learning = || {
        SESSIONS.with(|sessions| {
            let sessions = sessions.borrow();
            let options = &sessions.get(&handle).unwrap().options;
            options.learning || options.learn_english_on_enter
        })
    };
    assert!(!learning());
    let preferences = Preferences {
        learning: true,
        enter_learns_english_word: true,
        chinese_punctuation: !Preferences::default().chinese_punctuation,
        ..Preferences::default()
    };
    let update =
        json!({"format_version": 1, "revision": 1, "preferences": preferences}).to_string();
    let updated =
        read(unsafe { msime_client_update_preferences(handle, update.as_ptr(), update.len()) });
    assert_eq!(updated["ok"], true, "{updated}");
    assert!(!learning());
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}

/// 只读准备的请求：要有绝对路径的私有目录，不能同时发布 runtime-options.json；可写的准备不收私有目录。
#[test]
fn the_read_only_bootstrap_is_validated() {
    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    let state = root.path().join("state");
    let call = |request: Value| {
        let text = request.to_string();
        read(unsafe { msime_client_prepare_host(text.as_ptr(), text.len()) })
    };
    for request in [
        json!({"resources": resources, "state_root": state, "read_only_state": true}),
        json!({"resources": resources, "state_root": state, "read_only_state": true, "private_root": "relative"}),
        json!({"resources": resources, "state_root": state, "read_only_state": true, "private_root": root.path().join("private"), "runtime_options": root.path().join("runtime-options.json")}),
        json!({"resources": resources, "state_root": state, "private_root": root.path().join("private")}),
        json!({"resources": resources, "state_root": state, "read_only_state": true, "private_root": state.join("inside")}),
    ] {
        assert_eq!(call(request.clone())["ok"], false, "{request}");
    }
    assert!(!root.path().join("private").exists());
}

/// 真实词库（`MSIME_EVAL_RESOURCES` 指向暂存好的资源目录时才跑）：App 准备过、键盘扩展的共享目录只读时，键盘打 nihao 得到「你好」并上屏，共享目录一个字节也没变。
#[test]
fn a_read_only_keyboard_types_nihao_on_real_dictionaries() {
    let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES").map(PathBuf::from) else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let app = root.path().join("app/EngineResources");
    let keyboard = root.path().join("keyboard/EngineResources");
    for target in [&app, &keyboard] {
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        let copied = std::process::Command::new("cp")
            .arg("-R")
            .arg(&resources)
            .arg(target)
            .status()
            .unwrap();
        assert!(copied.success());
    }
    let state = root.path().join("group/MSIME");
    let private = root.path().join("keyboard-container/MSIME");
    let request = json!({"resources": app, "state_root": state}).to_string();
    let prepared = read(unsafe { msime_client_prepare_host(request.as_ptr(), request.len()) });
    assert_eq!(prepared["ok"], true, "{prepared}");
    let mut preferences = PreferencesStore::new(&state).load().unwrap();
    preferences.preferences.learning = true;
    PreferencesStore::new(&state)
        .save(preferences.revision, preferences.preferences)
        .unwrap();
    let before = tree_listing(&state);
    let (guard, enforced) = ReadOnlyTree::new(&state);
    if !enforced {
        return;
    }
    let document = prepare_read_only(&keyboard, &state, &private);
    assert_eq!(document["ok"], true, "{document}");
    let document = document["value"].clone();
    assert!(document["dictionaries"]
        .as_str()
        .unwrap()
        .contains("/group/MSIME/user/dictionaries/"));
    let mut options = document.clone();
    options["phrase_preedit"] = json!(true);
    let created = create(&options);
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    let mut view = Value::Null;
    for byte in b"nihao" {
        let typed = read(msime_client_character(handle, *byte, false));
        assert_eq!(typed["ok"], true, "{typed}");
        view = typed["value"]["view"].clone();
    }
    let texts: Vec<&str> = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|candidate| candidate["text"].as_str())
        .collect();
    let index = texts
        .iter()
        .position(|text| *text == "你好")
        .unwrap_or_else(|| panic!("{texts:?}"));
    let selected = read(msime_client_select(
        handle,
        view["generation"].as_u64().unwrap(),
        index,
    ));
    assert_eq!(selected["ok"], true, "{selected}");
    assert_eq!(selected["value"]["commit"], "你好", "{selected}");
    assert_eq!(read(msime_client_focus(handle, false))["ok"], true);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    drop(guard);
    assert_eq!(tree_listing(&state), before);
}

/// 目录树里每个普通文件的相对路径和 SHA-256。
fn tree_digests(root: &Path) -> Vec<(PathBuf, String)> {
    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                pending.push(entry.unwrap().path());
            }
            continue;
        }
        let digest = hex::encode(Sha256::digest(std::fs::read(&path).unwrap_or_default()));
        entries.push((path.strip_prefix(root).unwrap().to_path_buf(), digest));
    }
    entries.sort();
    entries
}

/// 在 `keyboard` 上只读准备（没有共享目录，所以是只读布局；`specification` 是合成资源的清单，真实词库时不给），建两次会话，每次打 `letters`、选 `pick`（给了时），再打拼不成音节的字母回车原样上屏。返回随包资源目录前后的差别。
fn type_on_the_read_only_layout(
    keyboard: &Path,
    root: &Path,
    specification: Option<&ResourceSet>,
    letters: &[u8],
    pick: Option<&str>,
) -> String {
    let state = root.join("group/MSIME");
    let private = root.join("keyboard-container/MSIME");
    let before = tree_digests(keyboard);
    // 合成的资源只对得上合成的清单，直接在它上面准备；真实词库走宿主的入口，用本版本锁定的清单。
    let mut document = match specification {
        Some(specification) => crate::read_only_state::prepare_on(
            keyboard,
            &state,
            &private,
            specification,
            &[],
            Edition::full(),
        )
        .unwrap(),
        None => {
            let document = prepare_read_only(keyboard, &state, &private);
            assert_eq!(document["ok"], true, "{document}");
            document["value"].clone()
        }
    };
    assert_eq!(
        canonical(&document["dictionaries"]),
        canonical(&json!(keyboard))
    );
    // 出厂就开着的回车学英文词（`enter_learns_english_word`），这里写明，不依赖默认值。
    document["preferences"]["enter_learns_english_word"] = json!(true);
    for round in 0..2 {
        let created = create(&document);
        assert_eq!(created["ok"], true, "{created}");
        let handle = created["value"]["session"].as_u64().unwrap();
        assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
        let mut view = Value::Null;
        for byte in letters {
            let typed = read(msime_client_character(handle, *byte, false));
            assert_eq!(typed["ok"], true, "{typed}");
            view = typed["value"]["view"].clone();
        }
        if let Some(word) = pick {
            let texts: Vec<&str> = view["candidates"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|candidate| candidate["text"].as_str())
                .collect();
            let index = texts
                .iter()
                .position(|text| *text == word)
                .unwrap_or_else(|| panic!("round {round}: {texts:?}"));
            let selected = read(msime_client_select(
                handle,
                view["generation"].as_u64().unwrap(),
                index,
            ));
            assert_eq!(selected["value"]["commit"], word, "{selected}");
        } else {
            assert_eq!(read(msime_client_command(handle, 3))["ok"], true);
        }
        // 回车原样上屏拼不成音节的字母：可写的会话会把它学成英文词，写进代次里的 `msime-english.db`。
        for byte in b"qwzx" {
            let typed = read(msime_client_character(handle, *byte, false));
            assert_eq!(typed["ok"], true, "{typed}");
        }
        let committed = read(msime_client_command(handle, 2));
        assert_eq!(committed["ok"], true, "{committed}");
        assert_eq!(committed["value"]["commit"], "qwzx", "{committed}");
        assert_eq!(read(msime_client_focus(handle, false))["ok"], true);
        assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    }
    let after = tree_digests(keyboard);
    let changed: Vec<_> = after
        .iter()
        .filter(|entry| !before.contains(entry))
        .collect();
    let removed: Vec<_> = before
        .iter()
        .filter(|entry| !after.contains(entry))
        .collect();
    if changed.is_empty() && removed.is_empty() {
        String::new()
    } else {
        format!("changed {changed:?} removed {removed:?}")
    }
}

/// 只读布局下词库就是随包资源：会话打字、回车原样上屏之后，资源目录一个字节也没变。改前回车学英文词把词写进了随包的 `msime-english.db`，之后同一份资源的校验都以 `resource length or digest mismatch` 失败（iOS 模拟器上 App 包可写，单元测试整批因此失败）。
#[test]
fn a_read_only_layout_leaves_the_bundled_resources_untouched() {
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard/EngineResources");
    let specification = synthetic_desktop_lock(&keyboard);
    let difference =
        type_on_the_read_only_layout(&keyboard, root.path(), Some(&specification), b"ni", None);
    assert!(difference.is_empty(), "{difference}");
}

/// 同上，真实词库（`MSIME_EVAL_RESOURCES` 指向暂存好的资源目录时才跑）：打 nihao 选「你好」上屏、回车原样上屏字母之后，随包资源目录一个字节也没变。
#[test]
fn a_read_only_layout_on_real_dictionaries_leaves_the_bundled_resources_untouched() {
    let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES").map(PathBuf::from) else {
        return;
    };
    let root = tempfile::tempdir().unwrap();
    let keyboard = root.path().join("keyboard/EngineResources");
    std::fs::create_dir_all(keyboard.parent().unwrap()).unwrap();
    let copied = std::process::Command::new("cp")
        .arg("-R")
        .arg(&resources)
        .arg(&keyboard)
        .status()
        .unwrap();
    assert!(copied.success());
    let difference =
        type_on_the_read_only_layout(&keyboard, root.path(), None, b"nihao", Some("你好"));
    assert!(difference.is_empty(), "{difference}");
}

/// 只读布局的会话：随包资源目录里带一张有两行（本地、拟好）的拼音表，词库就是它，用户目录和缓存在 `root/private`。`read_only` 为 false 时是同一份布局上的可写会话，作对照。
fn layout_session_with_rows(root: &Path, read_only: bool) -> (u64, PathBuf, PathBuf) {
    let resources = root.join("keyboard/EngineResources");
    let private = root.join("private");
    std::fs::create_dir_all(&resources).unwrap();
    rusqlite::Connection::open(resources.join("msime-pinyin.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
             INSERT INTO tbl_2_n VALUES('ni''hao','nh','本地',100),('ni''hao','nh','拟好',80);
             CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
             CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
             CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);",
        )
        .unwrap();
    for name in ["user", "cache"] {
        std::fs::create_dir_all(private.join(name)).unwrap();
    }
    let preferences = Preferences {
        default_ime_mode: msime_client_core::preferences::DefaultImeMode::Chinese,
        ..Preferences::default()
    };
    let mut options = json!({
        "api_version": 1,
        "resources": resources,
        "dictionaries": resources,
        "user_data": private.join("user"),
        "cache": private.join("cache"),
        "preferences": preferences,
    });
    if read_only {
        options["read_only_state"] = json!(true);
    }
    let created = create(&options);
    assert_eq!(created["ok"], true, "{created}");
    (
        created["value"]["session"].as_u64().unwrap(),
        resources,
        private,
    )
}

/// 打 nihao，返回候选文字和界面代次。
fn type_nihao(handle: u64) -> (Vec<String>, u64) {
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    let mut view = Value::Null;
    for byte in b"nihao" {
        let typed = read(msime_client_character(handle, *byte, false));
        assert_eq!(typed["ok"], true, "{typed}");
        view = typed["value"]["view"].clone();
    }
    let texts = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|candidate| candidate["text"].as_str().map(str::to_owned))
        .collect();
    (texts, view["generation"].as_u64().unwrap())
}

/// 只读会话不改词库：置顶、删除、固定排位和取消固定都以明确的错误拒绝，组字和候选原样留着，随包资源和私有目录一个字节也没变。改前置顶和删除直接改写随包的 `msime-pinyin.db`（只读布局下的词库就是它；模拟器上 App 包可写，之后资源校验再也过不去，真机上包只读，写失败只报一句「没存住」），固定排位写进只有键盘自己看得见、App 永远读不到的私有日志。可写的会话作对照：同一份布局上置顶照常改写词库。
#[test]
fn a_read_only_session_refuses_candidate_edits_and_keeps_the_composition() {
    let root = tempfile::tempdir().unwrap();

    let (writable, resources, _) = layout_session_with_rows(&root.path().join("writable"), false);
    let dictionary = resources.join("msime-pinyin.db");
    let before = std::fs::read(&dictionary).unwrap();
    let (texts, generation) = type_nihao(writable);
    let index = texts
        .iter()
        .position(|text| text == "拟好")
        .unwrap_or_else(|| panic!("{texts:?}"));
    let pinned = read(msime_client_pin_candidate(writable, generation, index));
    assert_eq!(pinned["ok"], true, "{pinned}");
    assert_ne!(
        std::fs::read(&dictionary).unwrap(),
        before,
        "precondition: pinning on this layout rewrites the dictionary"
    );
    assert_eq!(read(msime_client_destroy(writable))["ok"], true);

    let (handle, resources, private) =
        layout_session_with_rows(&root.path().join("read-only"), true);
    let (texts, generation) = type_nihao(handle);
    let index = texts
        .iter()
        .position(|text| text == "拟好")
        .unwrap_or_else(|| panic!("{texts:?}"));
    let resources_before = tree_digests(&resources);
    let private_before = tree_digests(&private);
    // 每一项各调一次，按顺序：置顶、删除、固定到第 1 位、取消固定。
    let results = [
        (
            "pin",
            read(msime_client_pin_candidate(handle, generation, index)),
        ),
        (
            "remove",
            read(msime_client_remove_candidate(handle, generation, index)),
        ),
        (
            "fix",
            read(msime_client_fix_candidate_position(
                handle, generation, index, 1,
            )),
        ),
        (
            "clear",
            read(msime_client_clear_candidate_position(
                handle, generation, index,
            )),
        ),
    ];
    for (name, result) in &results {
        assert_eq!(result["ok"], false, "{name}: {result}");
        assert_eq!(
            result["error"],
            crate::read_only_state::CANDIDATE_EDITING_UNAVAILABLE,
            "{name}: {result}"
        );
    }
    let view = read(msime_client_view(handle));
    assert_eq!(view["ok"], true, "{view}");
    assert_eq!(view["value"]["generation"], generation, "{view}");
    let still: Vec<&str> = view["value"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|candidate| candidate["text"].as_str())
        .collect();
    assert_eq!(still, texts, "{view}");
    assert_eq!(tree_digests(&resources), resources_before);
    assert_eq!(tree_digests(&private), private_before);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
