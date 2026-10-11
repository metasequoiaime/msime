use super::*;
use crate::community::resource::{CommunityResourceContent, SharedPhrase};
use std::fs;
use std::process::Command;

fn fresh() -> (tempfile::TempDir, CommonPhrasesStore) {
    let root = tempfile::tempdir().unwrap();
    let store = CommonPhrasesStore::new(root.path().join("preferences"));
    (root, store)
}

fn pack(id: u128, revision: u32, texts: &[&str]) -> CommunityResource {
    CommunityResource {
        id: Uuid::from_u128(id),
        kind: CommunityResourceKind::Phrase,
        name: "邮件签名".into(),
        description: "示例短语包".into(),
        author: "示例作者".into(),
        content: CommunityResourceContent {
            phrases: texts
                .iter()
                .map(|text| SharedPhrase {
                    text: (*text).into(),
                    group: String::new(),
                })
                .collect(),
            ..Default::default()
        },
        revision,
        saves: 0,
        saved: false,
        owned: false,
        rating_count: 0,
        rating_average: 0.0,
        my_rating: 0,
        moderation: None,
    }
}

fn texts(document: &CommonPhrases) -> Vec<&str> {
    document
        .phrases
        .iter()
        .map(|phrase| phrase.text.as_str())
        .collect()
}

#[cfg(unix)]
#[test]
fn common_phrase_writes_stay_bound_to_the_locked_directory_after_replacement() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("preferences");
    fs::create_dir(&directory).unwrap();
    let store = CommonPhrasesStore::new(&directory);
    let lock = store.lock().unwrap();

    let moved = root.path().join("preferences-moved");
    fs::rename(&directory, &moved).unwrap();
    fs::create_dir(&directory).unwrap();

    store
        .write_locked(&lock, &CommonPhrases::default())
        .unwrap();

    assert!(moved.join(FILE_NAME).exists());
    assert!(!directory.join(FILE_NAME).exists());
    fs::remove_dir_all(moved).unwrap();
}

#[test]
fn common_phrases_start_empty_and_keep_multiline_text() {
    let (_root, store) = fresh();
    assert_eq!(store.load().unwrap(), CommonPhrases::default());
    let document = store.add("此致\n敬礼").unwrap();
    assert_eq!(texts(&document), ["此致\n敬礼"]);
    assert_eq!(document.phrases[0].pack, None);
    assert_eq!(store.load().unwrap(), document);
    // 文件里 `pack` 明确写成 null，和契约的模型一致。
    let raw: serde_json::Value = serde_json::from_slice(&fs::read(&store.file).unwrap()).unwrap();
    assert_eq!(raw["phrases"][0]["pack"], serde_json::Value::Null);
    assert_eq!(raw["packs"], serde_json::json!([]));
}

#[test]
fn common_phrases_text_is_bounded_in_utf16_and_allows_only_line_feeds() {
    let (_root, store) = fresh();
    assert!(store.add(&"字".repeat(MAX_PHRASE_UTF16)).is_ok());
    assert!(matches!(
        store.add(&"字".repeat(MAX_PHRASE_UTF16 + 1)),
        Err(CommonPhrasesError::Invalid)
    ));
    // 补充平面的字占两个 UTF-16 单元：500 个正好到上限，501 个超出。
    assert!(store.add(&"𠀀".repeat(500)).is_ok());
    assert!(matches!(
        store.add(&"𠀀".repeat(501)),
        Err(CommonPhrasesError::Invalid)
    ));
    for invalid in ["", "  \n ", "制表\t符", "回车\r换行", "空\u{0}字符"] {
        assert!(
            matches!(store.add(invalid), Err(CommonPhrasesError::Invalid)),
            "{invalid:?}"
        );
    }
    assert_eq!(store.load().unwrap().phrases.len(), 2);
}

#[test]
fn common_phrases_refuse_more_than_two_hundred_own_entries() {
    let (_root, store) = fresh();
    for index in 0..MAX_OWN_PHRASES {
        store.add(&format!("常用语{index}")).unwrap();
    }
    assert!(matches!(
        store.add("再多一条"),
        Err(CommonPhrasesError::Limit)
    ));
    // 短语包的条目不占自己添加的名额。
    let document = store.install_pack(&pack(1, 1, &["包里的一条"])).unwrap();
    assert_eq!(document.document.phrases.len(), MAX_OWN_PHRASES + 1);
}

#[test]
fn common_phrases_deduplicate_within_one_source_only() {
    let (_root, store) = fresh();
    store.add("您好").unwrap();
    assert!(matches!(
        store.add("您好"),
        Err(CommonPhrasesError::Duplicate)
    ));
    let other = store.add("谢谢").unwrap().phrases[1].id;
    assert!(matches!(
        store.replace(other, "您好"),
        Err(CommonPhrasesError::Duplicate)
    ));
    // 包里与自己的某条相同的文本照样收下，包内重复的只留一条。
    let outcome = store
        .install_pack(&pack(1, 1, &["您好", "再见", "再见"]))
        .unwrap();
    assert_eq!(texts(&outcome.document), ["您好", "谢谢", "您好", "再见"]);
    assert_eq!(outcome.skipped, 1);
}

#[test]
fn common_phrases_replace_remove_and_move_by_id() {
    let (_root, store) = fresh();
    store.add("一").unwrap();
    store.add("二").unwrap();
    let document = store.add("三").unwrap();
    let ids: Vec<_> = document.phrases.iter().map(|phrase| phrase.id).collect();

    let document = store.move_to(ids[2], 0).unwrap();
    assert_eq!(texts(&document), ["三", "一", "二"]);
    let document = store.move_to(ids[2], 2).unwrap();
    assert_eq!(texts(&document), ["一", "二", "三"]);
    assert!(matches!(
        store.move_to(ids[0], 3),
        Err(CommonPhrasesError::Invalid)
    ));

    let document = store.replace(ids[1], "贰\n第二行").unwrap();
    assert_eq!(texts(&document), ["一", "贰\n第二行", "三"]);
    assert_eq!(document.phrases[1].id, ids[1]);

    let document = store.remove(ids[0]).unwrap();
    assert_eq!(texts(&document), ["贰\n第二行", "三"]);
    assert!(matches!(
        store.remove(ids[0]),
        Err(CommonPhrasesError::NotFound)
    ));
    assert!(matches!(
        store.replace(Uuid::new_v4(), "四"),
        Err(CommonPhrasesError::NotFound)
    ));
}

#[test]
fn common_phrases_pack_install_update_and_removal_take_their_entries_along() {
    let (_root, store) = fresh();
    store.add("自己的").unwrap();
    let outcome = store
        .install_pack(&pack(1, 1, &["第一条", "第二\r\n行"]))
        .unwrap();
    assert_eq!(outcome.skipped, 0);
    assert_eq!(texts(&outcome.document), ["自己的", "第一条", "第二\n行"]);
    assert_eq!(
        outcome.document.packs,
        [CommonPhrasePack {
            id: Uuid::from_u128(1),
            name: "邮件签名".into(),
            revision: 1,
        }]
    );
    store.add("后来加的").unwrap();
    store.install_pack(&pack(2, 1, &["另一个包"])).unwrap();

    // 更新同一个包：条目整体替换，位置不变，版本跟着变。
    let outcome = store.install_pack(&pack(1, 3, &["新的一条"])).unwrap();
    assert_eq!(
        texts(&outcome.document),
        ["自己的", "新的一条", "后来加的", "另一个包"]
    );
    assert_eq!(outcome.document.packs[0].revision, 3);
    assert_eq!(outcome.document.packs.len(), 2);

    // 删除包时连带删除来自它的条目，别的来源不动。
    let document = store.remove_pack(Uuid::from_u128(1)).unwrap();
    assert_eq!(texts(&document), ["自己的", "后来加的", "另一个包"]);
    assert_eq!(document.packs.len(), 1);
    assert!(matches!(
        store.remove_pack(Uuid::from_u128(1)),
        Err(CommonPhrasesError::NotFound)
    ));
}

#[test]
fn common_phrases_pack_limits_and_kind_are_enforced() {
    let (_root, store) = fresh();
    for index in 0..MAX_PACKS {
        store
            .install_pack(&pack(index as u128 + 1, 1, &["短语"]))
            .unwrap();
    }
    assert!(matches!(
        store.install_pack(&pack(100, 1, &["短语"])),
        Err(CommonPhrasesError::Limit)
    ));
    // 已安装的包可以更新，不受个数上限影响。
    store.install_pack(&pack(1, 2, &["更新"])).unwrap();

    let (_root, store) = fresh();
    let mut reply = pack(1, 1, &["短语"]);
    reply.kind = CommunityResourceKind::Reply;
    assert!(matches!(
        store.install_pack(&reply),
        Err(CommonPhrasesError::Invalid)
    ));
    // 服务端允许 2000 个 UTF-16 单元，本地只收 1000 以内的；一条都收不下就拒绝。
    let long = "字".repeat(MAX_PHRASE_UTF16 + 1);
    let outcome = store.install_pack(&pack(1, 1, &[&long, "短的"])).unwrap();
    assert_eq!(outcome.skipped, 1);
    assert_eq!(texts(&outcome.document), ["短的"]);
    assert!(matches!(
        store.install_pack(&pack(2, 1, &[&long])),
        Err(CommonPhrasesError::Invalid)
    ));
    let full: Vec<String> = (0..MAX_PACK_PHRASES)
        .map(|index| index.to_string())
        .collect();
    let refs: Vec<&str> = full.iter().map(String::as_str).collect();
    let outcome = store.install_pack(&pack(3, 1, &refs)).unwrap();
    assert_eq!(outcome.skipped, 0);
}

#[test]
fn invalid_local_pack_phrases_do_not_reserve_normalization_capacity() {
    let long = "字".repeat(MAX_PHRASE_UTF16 + 1);
    let resource = pack(1, 1, &[&long]);

    let (normalized, skipped) = super::normalized_pack_texts(&resource);

    assert!(normalized.is_empty());
    assert_eq!(normalized.capacity(), 0);
    assert_eq!(skipped, 1);
}

#[test]
fn common_phrases_file_size_is_bounded() {
    let (_root, store) = fresh();
    // 每条正好 1000 个 UTF-16 单元、约 3 KB；自己的 200 条加上几个满包，总会超过 2 MB。
    let body = "字".repeat(MAX_PHRASE_UTF16 - 4);
    for index in 0..MAX_OWN_PHRASES {
        store.add(&format!("{index:04}{body}")).unwrap();
    }
    let mut refused = false;
    for pack_index in 0..MAX_PACKS {
        let texts: Vec<String> = (0..MAX_PACK_PHRASES)
            .map(|index| format!("{index:04}{body}"))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        match store.install_pack(&pack(pack_index as u128 + 1, 1, &refs)) {
            Ok(_) => {}
            Err(CommonPhrasesError::TooLarge) => {
                refused = true;
                break;
            }
            Err(error) => panic!("{error}"),
        }
    }
    assert!(refused);
    assert!(fs::metadata(&store.file).unwrap().len() <= MAX_FILE_BYTES);
    store.load().unwrap();
}

#[test]
fn common_phrases_corrupt_file_is_reported_and_left_alone() {
    let (_root, store) = fresh();
    store.add("保留").unwrap();
    for corrupt in [
        b"{not json".to_vec(),
        // 条目引用了没有安装的包。
        serde_json::to_vec(&serde_json::json!({
            "phrases": [{"id": Uuid::from_u128(9), "text": "孤儿", "pack": Uuid::from_u128(7)}],
            "packs": []
        }))
        .unwrap(),
        // 重复的 id。
        serde_json::to_vec(&serde_json::json!({
            "phrases": [
                {"id": Uuid::from_u128(9), "text": "一", "pack": null},
                {"id": Uuid::from_u128(9), "text": "二", "pack": null}
            ],
            "packs": []
        }))
        .unwrap(),
        // 不认识的字段。
        serde_json::to_vec(&serde_json::json!({"phrases": [], "packs": [], "extra": 1})).unwrap(),
    ] {
        fs::write(&store.file, &corrupt).unwrap();
        assert!(matches!(store.load(), Err(CommonPhrasesError::Corrupt)));
        assert!(matches!(
            store.add("新的"),
            Err(CommonPhrasesError::Corrupt)
        ));
        assert_eq!(fs::read(&store.file).unwrap(), corrupt);
    }
    let oversized = vec![b' '; MAX_FILE_BYTES as usize + 1];
    fs::write(&store.file, &oversized).unwrap();
    assert!(matches!(store.load(), Err(CommonPhrasesError::Corrupt)));
    assert_eq!(
        fs::metadata(&store.file).unwrap().len(),
        oversized.len() as u64
    );
}

#[test]
fn common_phrases_actions_use_the_operation_tag() {
    let (_root, store) = fresh();
    let action: CommonPhrasesAction =
        serde_json::from_value(serde_json::json!({"operation": "add", "text": "你好"})).unwrap();
    let outcome = store.perform(action).unwrap();
    let id = outcome.document.phrases[0].id;
    let value = serde_json::to_value(&outcome).unwrap();
    assert_eq!(value["phrases"][0]["text"], "你好");
    assert!(value.get("skipped").is_none());
    let action: CommonPhrasesAction = serde_json::from_value(serde_json::json!({
        "operation": "move", "id": id, "index": 0
    }))
    .unwrap();
    store.perform(action).unwrap();
    let action: CommonPhrasesAction = serde_json::from_value(serde_json::json!({
        "operation": "install_pack", "resource": pack(5, 1, &["包"])
    }))
    .unwrap();
    assert_eq!(store.perform(action).unwrap().document.packs.len(), 1);
    assert!(
        serde_json::from_value::<CommonPhrasesAction>(serde_json::json!({
            "operation": "add", "text": "你好", "extra": true
        }))
        .is_err()
    );
    assert_eq!(
        CommonPhrasesError::Duplicate.code(),
        "common_phrases_duplicate"
    );
}

#[cfg(unix)]
#[test]
fn common_phrases_refuse_a_symlinked_file() {
    use std::os::unix::fs::symlink;

    let (root, store) = fresh();
    fs::create_dir_all(root.path().join("preferences")).unwrap();
    let outside = root.path().join("outside.json");
    fs::write(&outside, b"{\"phrases\":[],\"packs\":[]}").unwrap();
    symlink(&outside, &store.file).unwrap();
    assert!(store.load().is_err());
    assert!(store.add("不该写到外面").is_err());
    assert_eq!(
        fs::read(&outside).unwrap(),
        b"{\"phrases\":[],\"packs\":[]}"
    );
}

const CHILD_DIRECTORY: &str = "MSIME_COMMON_PHRASES_CHILD_DIRECTORY";
const CHILD_PREFIX: &str = "MSIME_COMMON_PHRASES_CHILD_PREFIX";
const CHILD_ADDS: usize = 40;

/// 并发测试的子进程入口：只有父测试设了环境变量时才做事，单独运行时什么也不做。
#[test]
fn common_phrases_child_process_adds() {
    let (Ok(directory), Ok(prefix)) = (std::env::var(CHILD_DIRECTORY), std::env::var(CHILD_PREFIX))
    else {
        return;
    };
    let store = CommonPhrasesStore::new(directory);
    for index in 0..CHILD_ADDS {
        store.add(&format!("{prefix}{index}")).unwrap();
    }
}

#[test]
fn common_phrases_concurrent_adds_from_two_processes_are_all_kept() {
    let (root, store) = fresh();
    let directory = root.path().join("preferences");
    let executable = std::env::current_exe().unwrap();
    let children: Vec<_> = ["甲", "乙"]
        .into_iter()
        .map(|prefix| {
            Command::new(&executable)
                .args([
                    "--exact",
                    "common_phrases::tests::common_phrases_child_process_adds",
                    "--test-threads=1",
                ])
                .env(CHILD_DIRECTORY, &directory)
                .env(CHILD_PREFIX, prefix)
                .spawn()
                .unwrap()
        })
        .collect();
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let document = store.load().unwrap();
    assert_eq!(document.phrases.len(), CHILD_ADDS * 2);
    for prefix in ["甲", "乙"] {
        for index in 0..CHILD_ADDS {
            let text = format!("{prefix}{index}");
            assert!(texts(&document).contains(&text.as_str()), "{text}");
        }
    }
}
