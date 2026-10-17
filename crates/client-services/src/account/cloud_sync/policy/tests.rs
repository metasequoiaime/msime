//! 用例从 `shared/backend/Tests/BackendSyncPolicyTests.swift` 和 `platforms/android/tests/core/SyncMergePolicySmoke.java` 移植，加上三方合并、合并后修复和云端被清空的新规则。只用合成数据。

use super::*;
use crate::account::AccountPreferenceField;

fn string(value: &str) -> AccountPreferenceValue {
    AccountPreferenceValue::String(value.to_owned())
}

fn settings(pairs: &[(&str, AccountPreferenceValue)]) -> BTreeMap<String, AccountPreferenceValue> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect()
}

fn cloud_phrase(id: &str, text: &str) -> AccountPhrase {
    AccountPhrase {
        id: id.into(),
        text: text.into(),
        group: String::new(),
        position: 0,
    }
}

fn local_phrase(id: &str, text: &str) -> LocalPhrase {
    LocalPhrase {
        id: id.into(),
        text: text.into(),
    }
}

fn texts(phrases: &[AccountPhrase]) -> Vec<&str> {
    phrases.iter().map(|phrase| phrase.text.as_str()).collect()
}

#[test]
fn first_run_uploads_when_the_cloud_is_empty_and_asks_otherwise() {
    assert_eq!(first_run(false, None), Some(Mode::Upload));
    assert_eq!(first_run(false, Some(Choice::UseCloud)), Some(Mode::Upload));
    assert_eq!(first_run(true, None), None);
    assert_eq!(first_run(true, Some(Choice::Merge)), Some(Mode::Merge));
    assert_eq!(
        first_run(true, Some(Choice::UseCloud)),
        Some(Mode::Download)
    );
}

#[test]
fn dictionary_merge_without_local_words_downloads() {
    assert_eq!(
        dictionary_first_run(true, false, Some(Choice::Merge)),
        Some(Mode::Download)
    );
    assert_eq!(
        dictionary_first_run(true, true, Some(Choice::Merge)),
        Some(Mode::Merge)
    );
    assert_eq!(dictionary_first_run(true, true, None), None);
    assert_eq!(dictionary_first_run(false, true, None), Some(Mode::Upload));
}

#[test]
fn incremental_mode_table() {
    assert_eq!(incremental(false, false), Mode::None);
    assert_eq!(incremental(false, true), Mode::Upload);
    assert_eq!(incremental(true, false), Mode::Download);
    assert_eq!(incremental(true, true), Mode::Merge);
}

#[test]
fn cloud_changed_compares_the_cursor_with_the_revision() {
    assert!(cloud_changed(None, 0));
    assert!(!cloud_changed(Some(7), 7));
    assert!(cloud_changed(Some(7), 8));
}

#[test]
fn due_throttles_to_five_minutes_and_thirty_seconds_when_dirty() {
    let start = 1_000_000_000;
    assert!(due(start, None, false));
    assert!(!due(start + 29_000, Some(start), true));
    assert!(due(start + 30_000, Some(start), true));
    assert!(!due(start + 299_000, Some(start), false));
    assert!(due(start + 300_000, Some(start), false));
    // 时钟往回拨不会把同步锁死。
    assert!(due(start - 60_000, Some(start), false));
}

#[test]
fn prompt_is_repeated_only_after_the_throttle_unless_forced() {
    let start = 1_000_000_000;
    assert!(prompt_due(start, None, false));
    assert!(!prompt_due(start + 60_000, Some(start), false));
    assert!(prompt_due(start + 60_000, Some(start), true));
    assert!(prompt_due(start + 300_000, Some(start), false));
}

#[test]
fn local_only_settings_are_dropped() {
    let mut values = settings(&[
        ("input.schema", string("quanpin")),
        (
            "platform.ios.incognito",
            AccountPreferenceValue::Boolean(true),
        ),
        ("developer_options.x", AccountPreferenceValue::Integer(3)),
        ("diagnostic_log", AccountPreferenceValue::Boolean(true)),
        (
            "voice.contribute_audio",
            AccountPreferenceValue::Boolean(true),
        ),
    ]);
    without_local_only(&mut values);
    assert_eq!(values, settings(&[("input.schema", string("quanpin"))]));
}

#[test]
fn use_cloud_on_an_empty_or_never_uploaded_cloud_keeps_local_data() {
    assert!(use_cloud_protected(0, true, true));
    assert!(use_cloud_protected(5, false, true));
    assert!(!use_cloud_protected(5, true, true));
    assert!(!use_cloud_protected(0, false, false));
}

#[test]
fn a_cleared_cloud_needs_a_base_an_advanced_revision_and_empty_content() {
    assert!(cloud_cleared(true, Some(3), 4, true));
    assert!(!cloud_cleared(false, Some(3), 4, true));
    assert!(!cloud_cleared(true, None, 4, true));
    assert!(!cloud_cleared(true, Some(4), 4, true));
    assert!(!cloud_cleared(true, Some(3), 4, false));
}

#[test]
fn settings_merge_takes_local_changes_and_keeps_cloud_changes() {
    let base = settings_digests(&settings(&[
        ("input.schema", string("quanpin")),
        ("input.traditional", AccountPreferenceValue::Boolean(false)),
        ("platform.android.theme", string("light")),
    ]));
    // 本机改了繁简，云端（另一台设备）改了方案和主题，还加了一个别的平台的键。
    let local = settings(&[
        ("input.schema", string("quanpin")),
        ("input.traditional", AccountPreferenceValue::Boolean(true)),
        ("platform.android.theme", string("light")),
    ]);
    let cloud = settings(&[
        ("input.schema", string("shuangpin")),
        ("input.traditional", AccountPreferenceValue::Boolean(false)),
        ("platform.android.theme", string("dark")),
        (
            "platform.ios.sound_enabled",
            AccountPreferenceValue::Boolean(false),
        ),
    ]);
    let merged = merge_settings(&cloud, &local, Some(&base));
    assert_eq!(
        merged,
        settings(&[
            ("input.schema", string("shuangpin")),
            ("input.traditional", AccountPreferenceValue::Boolean(true)),
            ("platform.android.theme", string("dark")),
            (
                "platform.ios.sound_enabled",
                AccountPreferenceValue::Boolean(false)
            ),
        ])
    );
    // 首次合并没有基线：本机导出的键全部胜出，别的平台的键保留。
    let first = merge_settings(&cloud, &local, None);
    assert_eq!(first["input.schema"], string("quanpin"));
    assert_eq!(first["platform.android.theme"], string("light"));
    assert_eq!(
        first["platform.ios.sound_enabled"],
        AccountPreferenceValue::Boolean(false)
    );
}

#[test]
fn a_key_the_cloud_removed_is_not_brought_back_unless_changed_locally() {
    let base = settings_digests(&settings(&[("input.schema", string("quanpin"))]));
    let merged = merge_settings(
        &BTreeMap::new(),
        &settings(&[("input.schema", string("quanpin"))]),
        Some(&base),
    );
    assert!(merged.is_empty());
    let changed = merge_settings(
        &BTreeMap::new(),
        &settings(&[("input.schema", string("wubi"))]),
        Some(&base),
    );
    assert_eq!(changed["input.schema"], string("wubi"));
}

#[test]
fn settings_outside_the_schema_and_type_mismatches_are_found() {
    let schema = AccountPreferenceSchema {
        fields: [
            (
                "input.schema".to_owned(),
                AccountPreferenceField {
                    value_type: "string".into(),
                },
            ),
            (
                "input.weight".to_owned(),
                AccountPreferenceField {
                    value_type: "number".into(),
                },
            ),
        ]
        .into_iter()
        .collect(),
        maximum_bytes: 1_048_576,
        update_mode: "replace".into(),
        revision_required: true,
    };
    let mut values = settings(&[
        ("input.schema", string("quanpin")),
        ("input.weight", AccountPreferenceValue::Integer(2)),
        ("platform.ios.global_theme", string("ink")),
    ]);
    within_schema(&mut values, &schema);
    assert_eq!(values.len(), 2);
    assert!(!settings_type_mismatch(&values, &schema));
    values.insert("input.schema".into(), AccountPreferenceValue::Boolean(true));
    assert!(settings_type_mismatch(&values, &schema));
}

#[test]
fn phrase_merge_deletes_what_one_side_removed_and_keeps_new_entries() {
    let base = text_digests(["早上好", "谢谢", "再见"]);
    // 本机删了「谢谢」、加了「辛苦了」；云端删了「再见」、加了「收到」。
    let local = [
        local_phrase("l1", "早上好"),
        local_phrase("l3", "再见"),
        local_phrase("l4", "辛苦了"),
    ];
    let cloud = [
        cloud_phrase("c1", "早上好"),
        cloud_phrase("c2", "谢谢"),
        cloud_phrase("c5", "收到"),
    ];
    let merged = merge_phrases(&local, &cloud, Some(&base), &BTreeSet::new());
    assert_eq!(texts(&merged), ["早上好", "辛苦了", "收到"]);
    // 两边都有的沿用云端 id，本机新加的用本机 id。
    assert_eq!(merged[0].id, "c1");
    assert_eq!(merged[1].id, "l4");
    // 首次合并没有基线：并集。
    let union = merge_phrases(&local, &cloud, None, &BTreeSet::new());
    assert_eq!(texts(&union), ["早上好", "再见", "辛苦了", "谢谢", "收到"]);
}

#[test]
fn phrases_this_device_could_not_hold_are_not_deleted_from_the_cloud() {
    let long = "长".repeat(1500);
    let base = text_digests(["早上好", long.as_str()]);
    let unheld = text_digests([long.as_str()]);
    let local = [local_phrase("l1", "早上好"), local_phrase("l2", "新的")];
    let cloud = [cloud_phrase("c1", "早上好"), cloud_phrase("c2", &long)];
    let merged = merge_phrases(&local, &cloud, Some(&base), &unheld);
    assert_eq!(texts(&merged), ["早上好", "新的", long.as_str()]);
    assert!(phrases_content_dirty(&local, &cloud, Some(&base), &unheld));
    assert!(!phrases_content_dirty(
        &local[..1],
        &cloud,
        Some(&base),
        &unheld
    ));
    assert!(!phrases_content_dirty(&local, &cloud, None, &unheld));
}

#[test]
fn repair_regenerates_duplicate_ids_and_keeps_whitespace_only_text() {
    let mut generated = 0;
    let repaired = repair_phrases(
        vec![
            cloud_phrase("same", "一"),
            cloud_phrase("same", "二"),
            cloud_phrase("bad id", "三"),
            cloud_phrase("x", "一"),
            cloud_phrase("y", "  "),
            cloud_phrase("z", ""),
            cloud_phrase("w", "nul\0"),
        ],
        4,
        || {
            generated += 1;
            format!("gen{generated}")
        },
    )
    .unwrap();
    assert_eq!(texts(&repaired), ["一", "二", "三", "  "]);
    assert_eq!(
        repaired.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["same", "gen1", "gen2", "y"]
    );
    assert_eq!(
        repaired.iter().map(|p| p.position).collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    let id = generated_phrase_id();
    assert_eq!(id.len(), 16);
    assert!(valid_phrase_id(&id));
}

#[test]
fn repair_refuses_over_limit_and_oversized_lists_instead_of_truncating() {
    let many: Vec<_> = (0..=MAX_PHRASES)
        .map(|index| cloud_phrase(&format!("p{index}"), &format!("第 {index} 条")))
        .collect();
    assert_eq!(
        repair_phrases(many.clone(), 1, generated_phrase_id),
        Err(PhrasesProblem::OverLimit)
    );
    assert!(repair_phrases(many[..MAX_PHRASES].to_vec(), 1, generated_phrase_id).is_ok());
    let large: Vec<_> = (0..200)
        .map(|index| {
            cloud_phrase(
                &format!("p{index}"),
                &format!("{index}{}", "长".repeat(1000)),
            )
        })
        .collect();
    assert_eq!(
        repair_phrases(large, 1, generated_phrase_id),
        Err(PhrasesProblem::TooLarge)
    );
    let mut grouped = cloud_phrase("g", "分组");
    grouped.group = "坏\u{7}".into();
    assert_eq!(
        repair_phrases(vec![grouped], 1, generated_phrase_id).unwrap()[0].group,
        ""
    );
}

#[test]
fn local_plan_aligns_by_text() {
    let local = [local_phrase("a", "保留"), local_phrase("b", "删掉")];
    let target = [
        cloud_phrase("1", "保留"),
        cloud_phrase("2", "新增"),
        cloud_phrase("3", "新增"),
    ];
    assert_eq!(
        local_plan(&local, &target),
        LocalPlan {
            add: vec!["新增".into()],
            remove: vec!["b".into()],
        }
    );
}

#[test]
fn skin_budget_matches_android() {
    assert_eq!(skin_budget(0), SKIN_FIELD_LIMIT);
    let overhead = SKINS_KEY.len() + 8;
    assert_eq!(
        skin_budget(DOCUMENT_LIMIT_BYTES - SKIN_FIELD_LIMIT - overhead + 10),
        SKIN_FIELD_LIMIT - 10
    );
    assert_eq!(skin_budget(DOCUMENT_LIMIT_BYTES), 0);
}

#[test]
fn skin_library_keeps_the_newest_designs_that_fit() {
    let library = serde_json::json!([
        {"id": "old", "name": "旧", "design": {"pad": "x".repeat(40)}, "updated_at": 1},
        {"id": "new", "name": "新", "design": {"pad": "x".repeat(40)}, "updated_at": 9},
        {"id": "mid", "name": "中", "design": {"pad": "x".repeat(40)}, "updated_at": 5},
    ])
    .to_string();
    let (all, total, kept) = trim_skin_library(&library, usize::MAX).unwrap();
    assert_eq!((total, kept), (3, 3));
    let ids: Vec<String> = serde_json::from_str::<Vec<serde_json::Value>>(&all)
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["new", "mid", "old"]);
    let one_item = all.len() / 3 + 2;
    let (trimmed, _, kept) = trim_skin_library(&library, one_item).unwrap();
    assert_eq!(kept, 1);
    assert!(trimmed.contains("\"new\""));
    assert!(trim_skin_library("{}", usize::MAX).is_none());
}

#[test]
fn skin_libraries_merge_by_id_and_update_time() {
    let local = serde_json::json!([
        {"id": "a", "name": "本机旧", "updated_at": 1},
        {"id": "b", "name": "本机新", "updated_at": 9},
    ])
    .to_string();
    let cloud = serde_json::json!([
        {"id": "a", "name": "云端新", "updated_at": 5},
        {"id": "b", "name": "云端旧", "updated_at": 2},
        {"id": "c", "name": "云端独有", "updated_at": 3},
        {"name": "没有 id"},
    ])
    .to_string();
    let merged: Vec<serde_json::Value> =
        serde_json::from_str(&merge_skin_libraries(&local, &cloud).unwrap()).unwrap();
    let names: Vec<&str> = merged
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["云端新", "本机新", "云端独有"]);
}

#[test]
fn words_are_deduplicated_and_batched() {
    let word = |kind: &str, key: &str, value: &str| Word {
        kind: kind.into(),
        key: key.into(),
        value: value.into(),
        weight: 1,
    };
    let words = vec![
        word("pinyin", "ni", "你"),
        word("pinyin", "ni", "你"),
        word("quick", "dz", "地址"),
        word("unknown", "x", "y"),
        word("pinyin", "", "空"),
        word("wubi", "wq", "你"),
    ];
    let batches = batches(words, 2);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0][1].kind, "quickPhrase");
    assert_eq!(batches[1].len(), 1);
}

#[test]
fn failure_messages_match_android_and_ios() {
    let failure = |status: u16, code: Option<&str>| AccountFailure {
        error: AccountError::Unavailable,
        status: Some(status),
        server_code: code.map(str::to_owned),
        retry_after_seconds: None,
        network_failure: false,
    };
    let message = |failure: AccountFailure| failure_message(&failure);
    assert_eq!(message(AccountError::Cancelled.into()), None);
    assert_eq!(
        message(AccountError::Unavailable.into()).as_deref(),
        Some(NETWORK_MESSAGE)
    );
    // 新增的断网标记不改同步的措辞：没拿到响应本来就按网络不可用说。
    assert_eq!(
        message(AccountFailure::network()).as_deref(),
        Some(NETWORK_MESSAGE)
    );
    assert_eq!(
        message(failure(401, None)).as_deref(),
        Some("登录已失效，请重新登录后再同步")
    );
    assert_eq!(
        message(failure(503, Some("service_disabled"))).as_deref(),
        Some("云同步暂时不可用")
    );
    assert_eq!(
        message(failure(400, Some("invalid_phrases"))).as_deref(),
        Some("常用语超出云端限制（最多 500 条，每条最多 2000 字），常用语没有同步")
    );
    assert_eq!(
        message(failure(400, Some("invalid_preference_field"))).as_deref(),
        Some("有设置项云端不认识，设置没有同步（invalid_preference_field）")
    );
    assert_eq!(
        message(failure(403, Some("account_banned"))).as_deref(),
        Some("账号已被停用，无法同步")
    );
    assert_eq!(
        message(failure(429, None)).as_deref(),
        Some("云端暂时繁忙，稍后自动重试（HTTP 429）")
    );
    assert_eq!(
        message(failure(404, Some("not_found"))).as_deref(),
        Some("同步失败（HTTP 404 not_found）")
    );
    assert_eq!(
        message(failure(409, None)).as_deref(),
        Some("同步失败（HTTP 409）")
    );
    assert!(is_conflict(Some(409), Some("revision_conflict")));
    assert!(!is_conflict(Some(409), Some("session_changed")));
}

#[test]
fn status_line_matches_android_and_ios() {
    let now = 1_700_000_000_000;
    let line = |enabled: bool, failure: Option<&str>, unheld: usize, synced: Option<u64>| {
        StatusLine {
            enabled,
            disabled_on_cloud_cleared: false,
            failure,
            cloud_cleared: false,
            skins_trimmed: false,
            unheld_phrases: unheld,
            last_synced_ms: synced,
            now_ms: now,
        }
        .render()
    };
    assert_eq!(line(false, Some(NETWORK_MESSAGE), 0, None), "云同步已关闭");
    assert_eq!(line(true, None, 0, None), "云同步已开启 · 尚未同步");
    assert_eq!(
        line(true, None, 0, Some(now - 120_000)),
        "已同步 · 最近 2 分钟前"
    );
    assert_eq!(
        line(true, None, 2, Some(now)),
        "云端有 2 条常用语超出本机上限，未下载但已保留"
    );
    assert_eq!(line(true, Some(NETWORK_MESSAGE), 2, None), NETWORK_MESSAGE);
    assert_eq!(line(true, Some(""), 0, Some(now)), "已同步 · 最近 刚刚");
    let trimmed = StatusLine {
        enabled: true,
        disabled_on_cloud_cleared: false,
        failure: None,
        cloud_cleared: false,
        skins_trimmed: true,
        unheld_phrases: 0,
        last_synced_ms: Some(now),
        now_ms: now,
    };
    assert_eq!(trimmed.render(), "自定义皮肤太多，只同步了最近的设计");
}

#[test]
fn the_cloud_cleared_lines_name_the_third_choice_and_why_sync_is_off() {
    let line = |enabled: bool, disabled_on_cloud_cleared: bool, cloud_cleared: bool| {
        StatusLine {
            enabled,
            disabled_on_cloud_cleared,
            failure: None,
            cloud_cleared,
            skins_trimmed: false,
            unheld_phrases: 0,
            last_synced_ms: None,
            now_ms: 0,
        }
        .render()
    };
    assert_eq!(line(true, false, true), CLOUD_CLEARED_LINE);
    assert_eq!(
        CLOUD_CLEARED_LINE,
        "云端数据已被清空，请选择合并、使用云端或关闭同步"
    );
    // 选了「关闭同步，保留本机」：关着的原因说清楚，云端还记着清空也一样。
    assert_eq!(line(false, true, true), DISABLED_ON_CLOUD_CLEARED_LINE);
    assert_eq!(line(false, true, false), DISABLED_ON_CLOUD_CLEARED_LINE);
    // 用户自己关的开关照旧。标记只在关着时才说话，打开以后按别的情况说。
    assert_eq!(line(false, false, true), "云同步已关闭");
    assert_eq!(line(true, true, true), CLOUD_CLEARED_LINE);
}

#[test]
fn only_the_two_sync_choices_can_be_stored() {
    // 宿主的回答多一个「关闭同步」，它不是同步选择。
    assert_eq!(Answer::Merge.choice(), Some(Choice::Merge));
    assert_eq!(Answer::UseCloud.choice(), Some(Choice::UseCloud));
    assert_eq!(Answer::Disable.choice(), None);
    assert_eq!(
        serde_json::from_str::<Answer>("\"disable\"").unwrap(),
        Answer::Disable
    );
    // 记进状态文件、留给以后那一轮的选择（`pending_choice`、词库留下的选择）是 `Choice`，写不下「关闭同步」：记着的旧选择不会在以后哪一轮把同步关掉。
    assert!(serde_json::from_str::<Choice>("\"disable\"").is_err());
    assert_eq!(
        serde_json::from_str::<Choice>("\"use_cloud\"").unwrap(),
        Choice::UseCloud
    );
}

#[test]
fn relative_time_matches_android() {
    let now = 1_000_000_000;
    assert_eq!(ago(now, now - 59_000), "刚刚");
    assert_eq!(ago(now, now - 5 * 60_000), "5 分钟前");
    assert_eq!(ago(now, now - 3 * 3_600_000), "3 小时前");
    assert_eq!(ago(now, now - 50 * 3_600_000), "2 天前");
    assert_eq!(ago(now, now + 60_000), "刚刚");
}

#[test]
fn the_merge_backoff_doubles_up_to_its_cap() {
    assert_eq!(stall_ms(0), STALL_BASE_MS);
    assert_eq!(stall_ms(1), STALL_BASE_MS);
    assert_eq!(stall_ms(2), 2 * STALL_BASE_MS);
    assert_eq!(stall_ms(3), 4 * STALL_BASE_MS);
    assert_eq!(stall_ms(7), 64 * STALL_BASE_MS);
    assert_eq!(stall_ms(8), MAX_STALL_MS);
    assert_eq!(stall_ms(u32::MAX), MAX_STALL_MS);
}
