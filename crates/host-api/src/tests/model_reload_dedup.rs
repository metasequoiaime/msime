//! 焦点切换重建会话时的模型热缓存复用、非输入会话不误清缓存与重复诊断行去重测试。

use super::*;

/// 准备最小可运行词库目录，返回传给 `msime_client_create` 或 `ffi::session::create_session` 的文档。多次对同一 `root` 调用时复用已建好的词库文件。
fn minimal_host_document(root: &Path, preferences: &Preferences, extra: Value) -> Value {
    let ensure_dir = |name: &str| {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    };
    let resources = ensure_dir("resources");
    let dictionaries = ensure_dir("dictionaries");
    let pinyin = resources.join("msime-pinyin.db");
    if !pinyin.is_file() {
        rusqlite::Connection::open(&pinyin)
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_n VALUES('ni','n','你',100);
                 CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);",
            )
            .unwrap();
        std::fs::copy(&pinyin, dictionaries.join("msime-pinyin.db")).unwrap();
    }
    let mut document = json!({
        "api_version": 1,
        "resources": resources,
        "user_data": ensure_dir("user"),
        "cache": ensure_dir("cache"),
        "dictionaries": dictionaries,
        "preferences": preferences,
        "preferences_directory": ensure_dir("state"),
    });
    if let Some(fields) = extra.as_object() {
        for (key, value) in fields {
            document[key] = value.clone();
        }
    }
    document
}

fn create_input_session(root: &Path, preferences: &Preferences, extra: Value) -> u64 {
    let payload = minimal_host_document(root, preferences, extra).to_string();
    let created = read(unsafe { msime_client_create(payload.as_ptr(), payload.len()) });
    assert_eq!(created["ok"], true, "{created}");
    created["value"]["session"].as_u64().unwrap()
}

#[cfg(feature = "onnx-rerank")]
fn install_synthetic_onnx_small(bundle_root: &Path) -> PathBuf {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../onnx-rerank/tests/fixtures");
    let directory = bundle_root.join(ffi::SMALL_RERANK_MODEL_DIRECTORY);
    std::fs::create_dir_all(&directory).unwrap();
    for name in ["lm_step.ort", "model.json", "vocab.json"] {
        std::fs::copy(fixtures.join(name), directory.join(name)).unwrap();
    }
    directory
}

/// `OnnxRerankers` 是进程级单槽缓存：与其他测试并行运行时，若别的线程用自己的临时目录替换了该单槽，等对方跑完再重跑本段断言；缓存为空（`None`）或仍是当前 `expected_root` 时绝不重试，避免掩盖真实回归。
#[cfg(feature = "onnx-rerank")]
fn run_with_exclusive_onnx_slot(expected_root: &Path, mut check: impl FnMut() -> bool) {
    for attempt in 0..20 {
        if check() {
            return;
        }
        assert!(
            attempt < 19,
            "并行测试持续占用单槽 `OnnxRerankers`：{}",
            expected_root.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// 缓存已持有重排器（`OnnxRerankers` 或 `SENTENCE_MODELS` 加 `MEASURED`）时，`msime_client_create` 直接返回预填好的 `KeyboardRerankerSlot`，保持建会话后 `keyboard_reranker_loading.is_some() && !runtime.has_reranker()` 的契约，且不再为每次焦点切换新建操作系统线程。
#[test]
fn warm_cache_prefills_keyboard_reranker_slot_without_spawning_threads() {
    let _sink = capture_diagnostics();
    let mut preferences = chinese_preferences();
    preferences.sentence_association.neural_keyboard = true;

    #[cfg(feature = "onnx-rerank")]
    {
        let root = tempfile::tempdir().unwrap();
        install_synthetic_onnx_small(root.path());
        run_with_exclusive_onnx_slot(root.path(), || {
            let warm_existing = ffi::onnx_reranker_loaded_root().as_deref() == Some(root.path());
            let before = keyboard_reranker_thread_spawns();
            let first = create_input_session(root.path(), &preferences, json!({}));
            if !warm_existing {
                assert_eq!(keyboard_reranker_thread_spawns(), before + 1);
            }
            SESSIONS.with(|sessions| {
                let session = &sessions.borrow()[&first];
                assert!(session.keyboard_reranker_loading.is_some());
                assert!(!session.runtime.has_reranker());
            });
            settle_keyboard_reranker(first);
            SESSIONS.with(|sessions| {
                assert!(sessions.borrow()[&first].runtime.has_reranker());
            });
            assert_eq!(read(msime_client_destroy(first))["ok"], true);

            // 模拟 Fcitx5 失焦销毁会话、重聚焦再建新会话：热缓存命中时零线程派生，槽立即处于已就绪状态。
            for _ in 0..5 {
                if matches!(ffi::onnx_reranker_loaded_root(), Some(other) if other != root.path()) {
                    return false;
                }
                let spawns = keyboard_reranker_thread_spawns();
                let handle = create_input_session(root.path(), &preferences, json!({}));
                if keyboard_reranker_thread_spawns() != spawns {
                    assert!(
                        matches!(ffi::onnx_reranker_loaded_root(), Some(other) if other != root.path()),
                        "ONNX 热缓存命中时不得再起后台加载线程"
                    );
                    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
                    return false;
                }
                SESSIONS.with(|sessions| {
                    let session = &sessions.borrow()[&handle];
                    let slot = session
                        .keyboard_reranker_loading
                        .as_ref()
                        .expect("建会话后槽必须存在");
                    assert!(
                        slot.lock().unwrap().as_ref().unwrap().is_some(),
                        "热缓存返回时槽内应已预填好重排器"
                    );
                    assert!(!session.runtime.has_reranker());
                });
                assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
                SESSIONS.with(|sessions| {
                    let session = &sessions.borrow()[&handle];
                    assert!(session.keyboard_reranker_loading.is_none());
                    assert!(session.runtime.has_reranker());
                });
                assert_eq!(read(msime_client_destroy(handle))["ok"], true);
            }
            true
        });
    }

    // 随包句子模型缓存（`SENTENCE_MODELS` + `MEASURED`）同样预填槽且不起新线程。
    let bundled_root = tempfile::tempdir().unwrap();
    let sentence_model = bundled_root.path().join("synthetic-keyboard.safetensors");
    write_context_free_sentence_model(&sentence_model);
    let extra = json!({ "sentence_model": sentence_model });
    let before = keyboard_reranker_thread_spawns();
    let first = create_input_session(bundled_root.path(), &preferences, extra.clone());
    assert_eq!(keyboard_reranker_thread_spawns(), before + 1);
    settle_keyboard_reranker(first);
    SESSIONS.with(|sessions| {
        assert!(sessions.borrow()[&first].runtime.has_reranker());
    });
    assert_eq!(read(msime_client_destroy(first))["ok"], true);

    for _ in 0..4 {
        let spawns = keyboard_reranker_thread_spawns();
        let handle = create_input_session(bundled_root.path(), &preferences, extra.clone());
        assert_eq!(
            keyboard_reranker_thread_spawns(),
            spawns,
            "随包模型热缓存命中时不得再起后台加载线程"
        );
        SESSIONS.with(|sessions| {
            let session = &sessions.borrow()[&handle];
            let slot = session
                .keyboard_reranker_loading
                .as_ref()
                .expect("建会话后槽必须存在");
            assert!(slot.lock().unwrap().as_ref().unwrap().is_some());
            assert!(!session.runtime.has_reranker());
        });
        assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
        SESSIONS.with(|sessions| {
            let session = &sessions.borrow()[&handle];
            assert!(session.keyboard_reranker_loading.is_none());
            assert!(session.runtime.has_reranker());
        });
        assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    }
}

/// 镜像会话（`keyboard_reranker: false`）与候选预览会话（`SessionUse::Preview`）无论创建还是更新偏好，都只清掉自身 `runtime` 上的打分器，不调用 `RerankerMount::Off` 释放进程共享的 `OnnxRerankers` 缓存；只有允许挂模型的输入会话关闭开关时才释放。
#[test]
fn mirror_and_preview_sessions_do_not_evict_keyboard_reranker_cache() {
    let _sink = capture_diagnostics();
    let root = tempfile::tempdir().unwrap();
    #[cfg(feature = "onnx-rerank")]
    install_synthetic_onnx_small(root.path());
    let sentence_model = root.path().join("non-evict-sentence.safetensors");
    write_context_free_sentence_model(&sentence_model);
    let extra = if cfg!(feature = "onnx-rerank") {
        json!({})
    } else {
        json!({ "sentence_model": sentence_model })
    };

    let mut on = chinese_preferences();
    on.sentence_association.neural_keyboard = true;
    let mut off = on.clone();
    off.sentence_association.neural_keyboard = false;

    let warm = create_input_session(root.path(), &on, extra.clone());
    settle_keyboard_reranker(warm);
    assert_eq!(read(msime_client_destroy(warm))["ok"], true);

    let off_before = ffi::keyboard_reranker_off_calls();

    // 镜像会话（`keyboard_reranker: false`）创建及切换开关均不触发 `RerankerMount::Off`。
    let mut mirror_extra = extra.clone();
    mirror_extra["keyboard_reranker"] = json!(false);
    let mirror = create_input_session(root.path(), &on, mirror_extra.clone());
    SESSIONS.with(|sessions| {
        assert!(!sessions.borrow()[&mirror].runtime.has_reranker());
    });
    assert_eq!(update(mirror, 1, &off)["value"]["deferred"], false);
    assert_eq!(update(mirror, 2, &on)["value"]["deferred"], false);
    assert_eq!(read(msime_client_destroy(mirror))["ok"], true);
    assert_eq!(ffi::keyboard_reranker_off_calls(), off_before);

    // `SessionUse::Preview` 以关闭开关创建，以及开启后在 `apply_pending` 中切到关闭，都不触发 `RerankerMount::Off`。
    let preview_off_bytes =
        serde_json::to_vec(&minimal_host_document(root.path(), &off, extra.clone())).unwrap();
    let preview_off =
        ffi::session::create_session(&preview_off_bytes, ffi::session::SessionUse::Preview)
            .unwrap()["session"]
            .as_u64()
            .unwrap();
    SESSIONS.with(|sessions| {
        assert!(!sessions.borrow()[&preview_off].runtime.has_reranker());
    });
    assert_eq!(read(msime_client_destroy(preview_off))["ok"], true);

    let preview_on_bytes =
        serde_json::to_vec(&minimal_host_document(root.path(), &on, extra.clone())).unwrap();
    let preview_on =
        ffi::session::create_session(&preview_on_bytes, ffi::session::SessionUse::Preview).unwrap()
            ["session"]
            .as_u64()
            .unwrap();
    assert_eq!(read(msime_client_focus(preview_on, true))["ok"], true);
    assert_eq!(update(preview_on, 1, &off)["value"]["deferred"], false);
    SESSIONS.with(|sessions| {
        assert!(!sessions.borrow()[&preview_on].runtime.has_reranker());
    });
    assert_eq!(read(msime_client_destroy(preview_on))["ok"], true);

    let preview_doc = minimal_host_document(root.path(), &off, extra.clone());
    let preview = candidate_preview::preview_candidates(&preview_doc, "nihao", 5, 10).unwrap();
    assert!(!preview.candidates.is_empty());
    assert_eq!(
        ffi::keyboard_reranker_off_calls(),
        off_before,
        "镜像会话与预览会话都不得调用 `keyboard_reranker(false, ...)` 释放缓存"
    );

    #[cfg(feature = "onnx-rerank")]
    run_with_exclusive_onnx_slot(root.path(), || {
        let warm = create_input_session(root.path(), &on, extra.clone());
        settle_keyboard_reranker(warm);
        assert_eq!(read(msime_client_destroy(warm))["ok"], true);

        let off_before = ffi::keyboard_reranker_off_calls();
        let mut mirror_extra = extra.clone();
        mirror_extra["keyboard_reranker"] = json!(false);
        let mirror = create_input_session(root.path(), &on, mirror_extra);
        assert_eq!(update(mirror, 1, &off)["value"]["deferred"], false);
        assert_eq!(update(mirror, 2, &on)["value"]["deferred"], false);
        assert_eq!(read(msime_client_destroy(mirror))["ok"], true);

        let preview_off_bytes =
            serde_json::to_vec(&minimal_host_document(root.path(), &off, extra.clone())).unwrap();
        let preview_off =
            ffi::session::create_session(&preview_off_bytes, ffi::session::SessionUse::Preview)
                .unwrap()["session"]
                .as_u64()
                .unwrap();
        assert_eq!(read(msime_client_destroy(preview_off))["ok"], true);
        assert_eq!(ffi::keyboard_reranker_off_calls(), off_before);

        match ffi::onnx_reranker_loaded_root() {
            Some(loaded) if loaded != root.path() => return false,
            None => panic!("镜像会话或预览会话误释放了 `OnnxRerankers` 缓存"),
            Some(_) => {}
        }
        let resources = root.path().join("resources");
        assert!(matches!(
            ffi::cached_keyboard_reranker_for(
                ffi::RerankerMount::Keystroke,
                resources.to_str().unwrap(),
                None,
                None,
                None,
                None,
                None,
            ),
            Some(Some(_))
        ));

        let spawns_before = keyboard_reranker_thread_spawns();
        let reused = create_input_session(root.path(), &on, extra.clone());
        if keyboard_reranker_thread_spawns() != spawns_before {
            assert!(
                matches!(ffi::onnx_reranker_loaded_root(), Some(other) if other != root.path()),
                "镜像会话和预览会话之后 ONNX 缓存仍应保持热态"
            );
            assert_eq!(read(msime_client_destroy(reused))["ok"], true);
            return false;
        }
        assert_eq!(read(msime_client_focus(reused, true))["ok"], true);
        assert_eq!(update(reused, 1, &off)["value"]["deferred"], false);
        assert_eq!(read(msime_client_destroy(reused))["ok"], true);
        if matches!(ffi::onnx_reranker_loaded_root(), Some(other) if other != root.path()) {
            return false;
        }
        assert!(ffi::cached_keyboard_reranker_for(
            ffi::RerankerMount::Keystroke,
            resources.to_str().unwrap(),
            None,
            None,
            None,
            None,
            None,
        )
        .is_none());
        true
    });

    #[cfg(not(feature = "onnx-rerank"))]
    {
        let spawns_before = keyboard_reranker_thread_spawns();
        let reused = create_input_session(root.path(), &on, extra.clone());
        assert_eq!(
            keyboard_reranker_thread_spawns(),
            spawns_before,
            "镜像会话和预览会话之后缓存仍应保持热态"
        );
        assert_eq!(read(msime_client_focus(reused, true))["ok"], true);

        // 允许挂模型的输入会话把 `neural_keyboard` 关掉、或直接以关闭状态建会话时，正常走 `RerankerMount::Off` 释放缓存。
        assert_eq!(update(reused, 1, &off)["value"]["deferred"], false);
        assert_eq!(read(msime_client_destroy(reused))["ok"], true);
        assert_eq!(ffi::keyboard_reranker_off_calls(), off_before + 1);

        let created_off = create_input_session(root.path(), &off, extra);
        assert_eq!(read(msime_client_destroy(created_off))["ok"], true);
        assert_eq!(ffi::keyboard_reranker_off_calls(), off_before + 2);
    }
}

/// 连续相同的 `report_keyboard_reranker`（忽略末尾 `" (shared)"`）与 `report_settled_model` 诊断只输出一次；状态改变、或者对应开关关闭再重新打开后，会再次输出。
#[test]
fn identical_model_diagnostics_are_deduplicated_and_reemitted_on_change_or_toggle() {
    let _sink = capture_diagnostics();

    // 1. 落定重排模型：焦点切换连续建会话不重复刷屏，装上模型或开关关后再开时重新报告。
    let settled_root = tempfile::tempdir().unwrap();
    let state = settled_root.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let mut desktop_on = neural_desktop_preferences();
    desktop_on.sentence_association.neural_keyboard = false;
    let mut desktop_off = desktop_on.clone();
    desktop_off.sentence_association.neural_desktop = false;

    for _ in 0..3 {
        let handle = create_input_session(settled_root.path(), &desktop_on, json!({}));
        assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    }
    let missing_line = format!(
        "settled model missing: no settled-model pack under {}",
        resource_packs::root(&state).display()
    );
    assert_eq!(
        settled_model_lines(settled_root.path()),
        std::slice::from_ref(&missing_line)
    );

    let model = install_settled_model(&state);
    let attached_line = format!("settled model attached: {}", model.display());
    let first_attached = create_input_session(settled_root.path(), &desktop_on, json!({}));
    assert_eq!(read(msime_client_destroy(first_attached))["ok"], true);
    let second_attached = create_input_session(settled_root.path(), &desktop_on, json!({}));
    assert_eq!(
        settled_model_lines(settled_root.path()),
        [missing_line.clone(), attached_line.clone()]
    );

    // 在 `rebuild`（`apply_pending`）里关掉 `neural_desktop` 会清掉去重记录，再打开时重新输出一次。
    assert_eq!(
        update(second_attached, 1, &desktop_off)["value"]["deferred"],
        false
    );
    assert_eq!(
        update(second_attached, 2, &desktop_on)["value"]["deferred"],
        false
    );
    assert_eq!(read(msime_client_destroy(second_attached))["ok"], true);
    let third_attached = create_input_session(settled_root.path(), &desktop_on, json!({}));
    assert_eq!(read(msime_client_destroy(third_attached))["ok"], true);
    assert_eq!(
        settled_model_lines(settled_root.path()),
        [
            missing_line.clone(),
            attached_line.clone(),
            attached_line.clone()
        ]
    );

    // 资源包重装到同一个发布目录：路径不变，文件的修改时间变了，下一个会话重新报告。
    std::fs::File::options()
        .write(true)
        .open(&model)
        .unwrap()
        .set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000))
        .unwrap();
    let replaced = create_input_session(settled_root.path(), &desktop_on, json!({}));
    assert_eq!(read(msime_client_destroy(replaced))["ok"], true);
    assert_eq!(
        settled_model_lines(settled_root.path()),
        [
            missing_line,
            attached_line.clone(),
            attached_line.clone(),
            attached_line
        ]
    );

    // 2. 逐键重排器（`"(shared)"` 后缀归一化与开关切换）。
    assert!(should_report_keyboard_reranker_diagnostic(
        "keyboard reranker onnx small",
        "/synthetic/rerank-model-small (page 1.2 ms, limit 15.0 ms) (sentence pool 32)"
    ));
    assert!(!should_report_keyboard_reranker_diagnostic(
        "keyboard reranker onnx small",
        "/synthetic/rerank-model-small (page 1.2 ms, limit 15.0 ms) (sentence pool 32) (shared)"
    ));
    assert!(!should_report_keyboard_reranker_diagnostic(
        "keyboard reranker onnx small",
        "/synthetic/rerank-model-small (page 1.2 ms, limit 15.0 ms) (sentence pool 32)"
    ));
    assert!(should_report_keyboard_reranker_diagnostic(
        "keyboard reranker at pause",
        "keyboard reranker onnx small: /synthetic/rerank-model-small (page 1.2 ms, limit 60.0 ms) (sentence pool 32)"
    ));
    assert!(!should_report_keyboard_reranker_diagnostic(
        "keyboard reranker at pause",
        "keyboard reranker onnx small: /synthetic/rerank-model-small (page 1.2 ms, limit 60.0 ms) (sentence pool 32) (shared)"
    ));

    #[cfg(feature = "onnx-rerank")]
    {
        let onnx_root = tempfile::tempdir().unwrap();
        let model_dir = install_synthetic_onnx_small(onnx_root.path());
        let mut kb_on = chinese_preferences();
        kb_on.sentence_association.neural_keyboard = true;
        let mut kb_off = kb_on.clone();
        kb_off.sentence_association.neural_keyboard = false;

        let onnx_lines = || -> Vec<String> {
            captured_diagnostics(onnx_root.path().to_str().unwrap())
                .into_iter()
                .filter(|line| line.starts_with("keyboard reranker "))
                .collect()
        };

        for _ in 0..3 {
            let handle = create_input_session(onnx_root.path(), &kb_on, json!({}));
            settle_keyboard_reranker(handle);
            assert_eq!(read(msime_client_destroy(handle))["ok"], true);
        }
        let initial = onnx_lines();
        assert_eq!(
            initial.len(),
            1,
            "首次加载与后续 `(shared)` 会话不应重复刷屏：{initial:?}"
        );
        assert!(initial[0].contains(&model_dir.display().to_string()));

        // `AttachedOnnxReranker.detail` 本身仍保留 `" (shared)"` 后缀。
        let mut local_cache = ffi::OnnxRerankers::default();
        let _first_direct = local_cache
            .attach(onnx_root.path(), f64::MAX, f64::MAX, None)
            .attached()
            .unwrap();
        let shared_direct = local_cache
            .attach_cached(onnx_root.path(), f64::MAX, f64::MAX, None)
            .unwrap()
            .attached()
            .unwrap();
        assert!(
            shared_direct.detail.ends_with(" (shared)"),
            "{}",
            shared_direct.detail
        );

        let toggled = create_input_session(onnx_root.path(), &kb_on, json!({}));
        assert_eq!(update(toggled, 1, &kb_off)["value"]["deferred"], false);
        assert_eq!(read(msime_client_destroy(toggled))["ok"], true);
        let reopened = create_input_session(onnx_root.path(), &kb_on, json!({}));
        settle_keyboard_reranker(reopened);
        assert_eq!(read(msime_client_destroy(reopened))["ok"], true);
        assert_eq!(onnx_lines().len(), 2);
    }

    // 3. 随包句子模型同样对连续相同诊断去重，并在 `RerankerMount::Off` 关闭重开后重新输出。
    let bundled_root = tempfile::tempdir().unwrap();
    let sentence_path = bundled_root.path().join("dedup-sentence.safetensors");
    write_context_free_sentence_model(&sentence_path);
    let extra = json!({ "sentence_model": sentence_path });
    let mut kb_on = chinese_preferences();
    kb_on.sentence_association.neural_keyboard = true;
    let mut kb_off = kb_on.clone();
    kb_off.sentence_association.neural_keyboard = false;

    let bundled_lines = || -> Vec<String> {
        captured_diagnostics(bundled_root.path().to_str().unwrap())
            .into_iter()
            .filter(|line| line.starts_with("keyboard reranker "))
            .collect()
    };

    let first = create_input_session(bundled_root.path(), &kb_on, extra.clone());
    settle_keyboard_reranker(first);
    assert_eq!(read(msime_client_destroy(first))["ok"], true);
    let second = create_input_session(bundled_root.path(), &kb_on, extra.clone());
    settle_keyboard_reranker(second);
    assert_eq!(bundled_lines().len(), 1);

    // 镜像会话与预览会话不清空去重记录。
    let mut mirror_extra = extra.clone();
    mirror_extra["keyboard_reranker"] = json!(false);
    let mirror = create_input_session(bundled_root.path(), &kb_on, mirror_extra);
    assert_eq!(read(msime_client_destroy(mirror))["ok"], true);
    let preview_bytes = serde_json::to_vec(&minimal_host_document(
        bundled_root.path(),
        &kb_off,
        extra.clone(),
    ))
    .unwrap();
    let preview = ffi::session::create_session(&preview_bytes, ffi::session::SessionUse::Preview)
        .unwrap()["session"]
        .as_u64()
        .unwrap();
    assert_eq!(read(msime_client_destroy(preview))["ok"], true);
    let third = create_input_session(bundled_root.path(), &kb_on, extra.clone());
    settle_keyboard_reranker(third);
    assert_eq!(read(msime_client_destroy(third))["ok"], true);
    assert_eq!(bundled_lines().len(), 1);

    // 输入会话关闭 `neural_keyboard` 触发 `RerankerMount::Off`，再打开时重新输出诊断。
    assert_eq!(update(second, 1, &kb_off)["value"]["deferred"], false);
    assert_eq!(update(second, 2, &kb_on)["value"]["deferred"], false);
    settle_keyboard_reranker(second);
    assert_eq!(read(msime_client_destroy(second))["ok"], true);
    assert_eq!(bundled_lines().len(), 2);

    let fourth = create_input_session(bundled_root.path(), &kb_on, extra);
    settle_keyboard_reranker(fourth);
    assert_eq!(read(msime_client_destroy(fourth))["ok"], true);
    assert_eq!(bundled_lines().len(), 2);
}
