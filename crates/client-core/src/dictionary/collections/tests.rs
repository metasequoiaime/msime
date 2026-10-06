use super::*;
use crate::community::resource::{CommunityResourceContent, SharedWord};
use crate::dictionary::personal::{PersonalWordApplied, PersonalWordPage, PersonalWordRequest};
use std::collections::BTreeMap;

struct Fixture {
    _root: tempfile::TempDir,
    personal_directory: PathBuf,
    store: DictionaryCollectionsStore,
    /// 模拟 Engine 的用户词库（身份到权重）：键盘应用过的请求在这里生效。
    dictionary: BTreeMap<String, i64>,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let personal_directory = root.path().join("personal");
        let store = DictionaryCollectionsStore::new(
            root.path().join("preferences"),
            PersonalDictionaryStore::new(&personal_directory),
        );
        Self {
            _root: root,
            personal_directory,
            store,
            dictionary: BTreeMap::new(),
        }
    }

    fn personal(&self) -> PersonalDictionaryStore {
        PersonalDictionaryStore::new(&self.personal_directory)
    }

    fn requests(&self) -> Vec<PersonalWordRequest> {
        self.personal().read().unwrap().requests
    }

    /// 像键盘那样把个人词库队列里等待中的请求全部应用，再让集合把待发送的送完，直到两边都空。集合加入的词用户词库里已经有时，像 host-api 那样不改它，只给回执。
    fn drain(&mut self) -> DictionaryCollectionsView {
        loop {
            let personal = self.personal();
            while personal.read().unwrap().pending_count() > 0 {
                let dictionary = &mut self.dictionary;
                personal
                    .synchronize_reporting_present(
                        |request| {
                            if let (None, Some(replacement)) =
                                (&request.previous, &request.replacement)
                            {
                                if request.id.starts_with("collections-")
                                    && dictionary.contains_key(&replacement.identity())
                                {
                                    return Ok(PersonalWordApplied::AlreadyPresent);
                                }
                            }
                            if let Some(previous) = &request.previous {
                                dictionary.remove(&previous.identity());
                            }
                            if let Some(replacement) = &request.replacement {
                                dictionary.insert(replacement.identity(), replacement.weight);
                            }
                            Ok(PersonalWordApplied::Written)
                        },
                        |_| {
                            Ok(PersonalWordPage {
                                entries: Vec::new(),
                                has_more: false,
                            })
                        },
                    )
                    .unwrap();
            }
            let view = self.store.flush().unwrap();
            if view.sent == Some(0) {
                assert!(view.collections.iter().all(|item| item.pending == 0));
                return view;
            }
        }
    }

    fn has(&self, word: &PersonalWord) -> bool {
        self.dictionary.contains_key(&word.identity())
    }
}

fn english(key: &str) -> PersonalWord {
    PersonalWord {
        kind: PersonalWordKind::English,
        key: key.into(),
        value: key.to_uppercase(),
        weight: 10_000,
    }
}

/// 第 `index` 个只由字母组成的编码（`a`、`b`、…、`z`、`ba`、…）。
fn letters(mut index: usize) -> String {
    let mut code = Vec::new();
    loop {
        code.push(b'a' + (index % 26) as u8);
        index /= 26;
        if index == 0 {
            break;
        }
    }
    code.reverse();
    String::from_utf8(code).unwrap()
}

fn id_of(view: &DictionaryCollectionsView, name: &str) -> String {
    view.collections
        .iter()
        .find(|item| item.name == name)
        .unwrap()
        .id
        .to_string()
}

fn create_with(fixture: &Fixture, name: &str, words: &[PersonalWord]) -> String {
    let view = fixture
        .store
        .create(name, PersonalWordKind::English)
        .unwrap();
    let id = id_of(&view, name);
    fixture.store.add_words(&id, words.to_vec()).unwrap();
    id
}

#[test]
fn collections_load_lists_import_formats_and_starts_empty() {
    let fixture = Fixture::new();
    let view = fixture.store.load().unwrap();
    assert!(view.collections.is_empty());
    assert_eq!(view.formats, ["txt", "standard", "windows", "hans", "rime"]);
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"collections": [], "formats": ["txt", "standard", "windows", "hans", "rime"]})
    );
}

#[test]
fn collections_enable_and_disable_round_trip_through_the_personal_queue() {
    let mut fixture = Fixture::new();
    let words = [english("alpha"), english("beta"), english("gamma")];
    let id = create_with(&fixture, "英文", &words);
    // 新集合默认启用，词条已经排进个人词库队列。
    let requests = fixture.requests();
    assert_eq!(requests.len(), 3);
    assert!(requests
        .iter()
        .all(|request| request.previous.is_none() && request.replacement.is_some()));
    let view = fixture.drain();
    assert_eq!(view.collections[0].entry_count, 3);
    assert!(words.iter().all(|word| fixture.has(word)));

    fixture.store.set_enabled(&id, false).unwrap();
    let removals: Vec<_> = fixture
        .requests()
        .into_iter()
        .filter(|request| request.status == PersonalWordRequestStatus::Pending)
        .collect();
    assert_eq!(removals.len(), 3);
    assert!(removals
        .iter()
        .all(|request| request.previous.is_some() && request.replacement.is_none()));
    let view = fixture.drain();
    assert!(!view.collections[0].enabled);
    assert!(words.iter().all(|word| !fixture.has(word)));
    // 停用以后词条仍在集合里，重新启用就回来。
    assert_eq!(view.collections[0].entry_count, 3);

    fixture.store.set_enabled(&id, true).unwrap();
    fixture.drain();
    assert!(words.iter().all(|word| fixture.has(word)));
    // 状态没变时不排任何请求。
    let before = fixture.requests().len();
    fixture.store.set_enabled(&id, true).unwrap();
    assert_eq!(fixture.requests().len(), before);
}

#[test]
fn collections_disabling_one_keeps_words_another_enabled_collection_holds() {
    let mut fixture = Fixture::new();
    let shared = english("shared");
    let only_a = english("onlya");
    let only_b = english("onlyb");
    let a = create_with(&fixture, "甲", &[only_a.clone(), shared.clone()]);
    let b = create_with(&fixture, "乙", &[shared.clone(), only_b.clone()]);
    fixture.drain();
    assert!(fixture.has(&shared));

    fixture.store.set_enabled(&a, false).unwrap();
    fixture.drain();
    assert!(!fixture.has(&only_a));
    assert!(fixture.has(&shared), "乙仍然启用，共有的词不能删");
    assert!(fixture.has(&only_b));

    // 再删掉乙：这次共有的词已经不属于任何启用的集合。
    fixture.store.delete(&b).unwrap();
    let view = fixture.drain();
    assert!(!fixture.has(&shared));
    assert!(!fixture.has(&only_b));
    assert_eq!(view.collections.len(), 1);

    // 启用甲时，共有的词重新加入；乙已经不在了。
    fixture.store.set_enabled(&a, true).unwrap();
    fixture.drain();
    assert!(fixture.has(&shared) && fixture.has(&only_a));
}

#[test]
fn collections_words_learned_outside_any_collection_are_never_removed() {
    let mut fixture = Fixture::new();
    let learned = english("learned");
    fixture
        .dictionary
        .insert(learned.identity(), learned.weight);
    let id = create_with(&fixture, "英文", &[english("alpha")]);
    fixture.drain();
    fixture.store.delete(&id).unwrap();
    fixture.drain();
    assert!(fixture.has(&learned));
    assert!(fixture
        .requests()
        .iter()
        .all(|request| request.previous.as_ref() != Some(&learned)));
}

#[test]
fn collections_unsent_opposite_operations_cancel_out() {
    let mut fixture = Fixture::new();
    // 键盘持有个人词库的锁时什么也送不进去，修改只停在待发送队列里。
    std::fs::create_dir_all(&fixture.personal_directory).unwrap();
    let held = file_lock::open_lock_file(fixture.personal_directory.join("sync.lock")).unwrap();
    assert!(file_lock::try_exclusive(&held).unwrap());

    let id = create_with(&fixture, "英文", &[english("alpha"), english("beta")]);
    let view = fixture.store.load().unwrap();
    assert_eq!(view.collections[0].pending, 2);
    assert!(matches!(
        fixture.store.flush(),
        Ok(DictionaryCollectionsView { sent: Some(0), .. })
    ));
    fixture.store.set_enabled(&id, false).unwrap();
    let view = fixture.store.load().unwrap();
    assert_eq!(view.collections[0].pending, 0);
    drop(held);
    fixture.drain();
    assert!(
        fixture.requests().is_empty(),
        "加入和删除互相抵消，什么也不用送"
    );
    assert!(fixture.dictionary.is_empty());
}

#[test]
fn collections_large_collections_are_sent_in_batches_the_queue_accepts() {
    let mut fixture = Fixture::new();
    let words: Vec<_> = (0..300)
        .map(|index| english(&letters(index + 100)))
        .collect();
    create_with(&fixture, "大词库", &words);
    let view = fixture.store.load().unwrap();
    // 个人词库队列同时只接受 128 个未完成的请求。
    assert_eq!(fixture.requests().len(), PERSONAL_QUEUE_CAPACITY);
    assert_eq!(view.collections[0].pending, 300 - PERSONAL_QUEUE_CAPACITY);
    assert_eq!(fixture.store.flush().unwrap().sent, Some(0));
    let view = fixture.drain();
    assert_eq!(view.collections[0].pending, 0);
    assert!(words.iter().all(|word| fixture.has(word)));
}

#[test]
fn collections_hold_at_most_twenty_thousand_entries() {
    let fixture = Fixture::new();
    let view = fixture
        .store
        .create("满", PersonalWordKind::English)
        .unwrap();
    let id = id_of(&view, "满");
    fixture.store.set_enabled(&id, false).unwrap();
    let words: Vec<_> = (0..MAX_COLLECTION_ENTRIES)
        .map(|index| english(&letters(index)))
        .collect();
    fixture.store.add_words(&id, words).unwrap();
    assert!(matches!(
        fixture
            .store
            .add_words(&id, vec![english(&letters(MAX_COLLECTION_ENTRIES))]),
        Err(DictionaryCollectionsError::Limit)
    ));
    // 已有的词不占名额。
    fixture.store.add_words(&id, vec![english("a")]).unwrap();
    assert_eq!(
        fixture.store.load().unwrap().collections[0].entry_count,
        MAX_COLLECTION_ENTRIES
    );
    // 导入到已满的集合时后面的行不读，并如实说明。
    let view = fixture
        .store
        .import(ImportRequest {
            id: Some(&id),
            name: None,
            kind: PersonalWordKind::English,
            format: "standard",
            text: "ZZZZZZ\tzzzzzz\n",
            readings: None,
        })
        .unwrap();
    let report = view.import.unwrap();
    assert_eq!(report.imported, 0);
    assert!(report.truncated);
}

#[test]
fn collections_names_are_one_to_thirty_two_characters() {
    let fixture = Fixture::new();
    for invalid in [
        "",
        " 前空格",
        "后空格 ",
        "换\n行",
        &"字".repeat(MAX_NAME_CHARS + 1),
    ] {
        assert!(
            matches!(
                fixture.store.create(invalid, PersonalWordKind::Pinyin),
                Err(DictionaryCollectionsError::InvalidName)
            ),
            "{invalid:?}"
        );
    }
    let view = fixture
        .store
        .create(&"字".repeat(MAX_NAME_CHARS), PersonalWordKind::Pinyin)
        .unwrap();
    let id = view.collections[0].id.to_string();
    assert!(matches!(
        fixture.store.rename(&id, ""),
        Err(DictionaryCollectionsError::InvalidName)
    ));
    let view = fixture.store.rename(&id, "工作").unwrap();
    assert_eq!(view.collections[0].name, "工作");
}

#[test]
fn collections_the_builtin_dictionary_cannot_be_changed() {
    let fixture = Fixture::new();
    for id in ["builtin", "builtin:pinyin", "builtin:wubi"] {
        for result in [
            fixture.store.set_enabled(id, false),
            fixture.store.delete(id),
            fixture.store.rename(id, "改名"),
        ] {
            let error = result.unwrap_err();
            assert!(matches!(error, DictionaryCollectionsError::BuiltinLocked));
            assert_eq!(error.code(), "builtin_locked");
        }
    }
    assert!(matches!(
        fixture.store.set_enabled(&Uuid::nil().to_string(), false),
        Err(DictionaryCollectionsError::Invalid)
    ));
    assert!(matches!(
        fixture
            .store
            .set_enabled(&Uuid::new_v4().to_string(), false),
        Err(DictionaryCollectionsError::NotFound)
    ));
}

#[test]
fn collections_corrupt_files_are_reported_and_left_alone() {
    let fixture = Fixture::new();
    let id = create_with(&fixture, "英文", &[english("alpha")]);
    let directory = fixture.store.directory.clone();
    let index = std::fs::read(directory.join("index.json")).unwrap();

    std::fs::write(directory.join("index.json"), b"{broken").unwrap();
    assert!(matches!(
        fixture.store.load(),
        Err(DictionaryCollectionsError::Corrupt)
    ));
    assert!(matches!(
        fixture.store.create("新的", PersonalWordKind::English),
        Err(DictionaryCollectionsError::Corrupt)
    ));
    assert_eq!(
        std::fs::read(directory.join("index.json")).unwrap(),
        b"{broken"
    );

    std::fs::write(directory.join("index.json"), &index).unwrap();
    let collection = directory.join(format!("{id}.json"));
    let original = std::fs::read(&collection).unwrap();
    std::fs::write(&collection, b"[]").unwrap();
    assert!(matches!(
        fixture.store.load(),
        Err(DictionaryCollectionsError::Corrupt)
    ));
    // 集合文件丢了也是损坏，不是空集合。
    std::fs::remove_file(&collection).unwrap();
    assert!(matches!(
        fixture.store.load(),
        Err(DictionaryCollectionsError::Corrupt)
    ));
    std::fs::write(&collection, &original).unwrap();
    assert_eq!(fixture.store.load().unwrap().collections.len(), 1);
}

#[test]
fn collections_words_must_match_the_collection_kind() {
    let fixture = Fixture::new();
    let view = fixture
        .store
        .create("拼音", PersonalWordKind::Pinyin)
        .unwrap();
    let id = id_of(&view, "拼音");
    assert!(matches!(
        fixture.store.add_words(&id, vec![english("alpha")]),
        Err(DictionaryCollectionsError::Invalid)
    ));
    let word = PersonalWord {
        kind: PersonalWordKind::Pinyin,
        key: "ni'hao".into(),
        value: "你好".into(),
        weight: 10_000,
    };
    let view = fixture
        .store
        .add_words(&id, vec![word.clone(), word.clone()])
        .unwrap();
    assert_eq!(view.collections[0].entry_count, 1);
    let view = fixture.store.remove_words(&id, &[word]).unwrap();
    assert_eq!(view.collections[0].entry_count, 0);
}

struct Readings;

impl HanReadings for Readings {
    fn pinyin(&self, word: &str) -> Option<String> {
        match word {
            "你好" => Some("ni'hao".into()),
            "世界" => Some("shi'jie".into()),
            _ => None,
        }
    }
}

fn import(
    fixture: &Fixture,
    kind: PersonalWordKind,
    format: &str,
    text: &str,
) -> std::result::Result<DictionaryCollectionsView, DictionaryCollectionsError> {
    fixture.store.import(ImportRequest {
        id: None,
        name: Some("导入"),
        kind,
        format,
        text,
        readings: Some(&Readings),
    })
}

#[test]
fn collections_import_reads_text_formats_and_refuses_scel() {
    let fixture = Fixture::new();
    let view = import(
        &fixture,
        PersonalWordKind::Pinyin,
        "standard",
        "\u{feff}你好\tni'hao\t100\n坏行\n世界\tshi'jie\n你好\tni'hao\n",
    )
    .unwrap();
    let report = view.import.clone().unwrap();
    assert_eq!(report.imported, 2);
    assert_eq!(report.duplicates, 1);
    assert_eq!(report.failed, 1);
    assert_eq!(report.first_failures[0].line, 2);
    assert_eq!(
        view.collections[0].source,
        CollectionSource::Import {
            format: "standard".into()
        }
    );

    // `txt` 是 `standard` 的别名；列的顺序反了时换一种读法并说明。
    let view = import(&fixture, PersonalWordKind::English, "txt", "hello\tHello\n").unwrap();
    assert!(!view.import.as_ref().unwrap().swapped);
    let view = import(&fixture, PersonalWordKind::Pinyin, "txt", "ni'hao\t你好\n").unwrap();
    assert!(view.import.as_ref().unwrap().swapped);

    // Rime 的头部不是词条，行号从文件开头数。
    let rime = "# Rime dictionary\n---\nname: sample\nversion: \"1\"\n...\n你好\tni hao\t1\n";
    let view = import(&fixture, PersonalWordKind::Pinyin, "rime", rime).unwrap();
    assert_eq!(view.import.as_ref().unwrap().imported, 1);
    assert_eq!(view.import.as_ref().unwrap().failed, 0);

    // `hans` 的读音由宿主给出，给不出读音的行算失败。
    let view = import(
        &fixture,
        PersonalWordKind::Pinyin,
        "hans",
        "你好\n世界\n未知\n",
    )
    .unwrap();
    let report = view.import.as_ref().unwrap();
    assert_eq!((report.imported, report.failed), (2, 1));
    assert!(matches!(
        import(&fixture, PersonalWordKind::English, "hans", "你好\n"),
        Err(DictionaryCollectionsError::Invalid)
    ));
    assert!(matches!(
        fixture.store.import(ImportRequest {
            id: None,
            name: Some("导入"),
            kind: PersonalWordKind::Pinyin,
            format: "hans",
            text: "你好\n",
            readings: None,
        }),
        Err(DictionaryCollectionsError::UnsupportedFormat)
    ));

    let error = import(&fixture, PersonalWordKind::Pinyin, "scel", "").unwrap_err();
    assert_eq!(error.code(), "unsupported_format");
    let action: DictionaryCollectionsAction = serde_json::from_value(serde_json::json!({
        "operation": "import", "name": "搜狗", "kind": "pinyin", "format": "scel", "bytes_base64": "AAAA"
    }))
    .unwrap();
    assert_eq!(
        fixture.store.perform(action, None).unwrap_err().code(),
        "unsupported_format"
    );

    assert!(matches!(
        import(
            &fixture,
            PersonalWordKind::Pinyin,
            "standard",
            "# 只有注释\n"
        ),
        Err(DictionaryCollectionsError::Import(
            ImportError::NoUsableRows
        ))
    ));
    assert!(matches!(
        import(
            &fixture,
            PersonalWordKind::Pinyin,
            "standard",
            "你好\tni'hao\u{7}\n"
        ),
        Err(DictionaryCollectionsError::Import(
            ImportError::ControlCharacters
        ))
    ));
    // 失败的导入不建集合。
    assert_eq!(fixture.store.load().unwrap().collections.len(), 5);
}

fn community(id: u128, revision: u32, codes: &[&str]) -> CommunityResource {
    CommunityResource {
        id: Uuid::from_u128(id),
        kind: CommunityResourceKind::Dictionary,
        name: "社区英文".into(),
        description: String::new(),
        author: "示例作者".into(),
        content: CommunityResourceContent {
            entries: codes
                .iter()
                .map(|code| SharedWord {
                    kind: crate::cloud::dictionary::DictionaryKind::English,
                    code: (*code).into(),
                    word: code.to_uppercase(),
                    weight: 10_000,
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

#[test]
fn collections_community_dictionaries_install_and_update_in_place() {
    let mut fixture = Fixture::new();
    let view = fixture
        .store
        .install_community(&community(7, 1, &["alpha", "beta"]))
        .unwrap();
    assert_eq!(
        view.collections[0].source,
        CollectionSource::Community {
            resource_id: Uuid::from_u128(7),
            revision: 1
        }
    );
    fixture.drain();
    assert!(fixture.has(&english("alpha")) && fixture.has(&english("beta")));

    let view = fixture
        .store
        .install_community(&community(7, 2, &["beta", "gamma"]))
        .unwrap();
    assert_eq!(view.collections.len(), 1);
    assert_eq!(view.collections[0].entry_count, 2);
    fixture.drain();
    assert!(!fixture.has(&english("alpha")));
    assert!(fixture.has(&english("beta")) && fixture.has(&english("gamma")));

    let mut reply = community(8, 1, &["alpha"]);
    reply.kind = CommunityResourceKind::Reply;
    assert!(matches!(
        fixture.store.install_community(&reply),
        Err(DictionaryCollectionsError::Invalid)
    ));
}

#[test]
fn collections_actions_use_the_operation_tag() {
    let fixture = Fixture::new();
    let action: DictionaryCollectionsAction = serde_json::from_value(serde_json::json!({
        "operation": "create", "name": "工作", "kind": "quickPhrase"
    }))
    .unwrap();
    let view = fixture.store.perform(action, None).unwrap();
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(
        value["collections"][0]["source"],
        serde_json::json!({"type": "user"})
    );
    assert_eq!(value["collections"][0]["kind"], "quickPhrase");
    assert_eq!(value["collections"][0]["enabled"], true);
    let action: DictionaryCollectionsAction =
        serde_json::from_value(serde_json::json!({"operation": "flush"})).unwrap();
    assert_eq!(fixture.store.perform(action, None).unwrap().sent, Some(0));
    assert!(
        serde_json::from_value::<DictionaryCollectionsAction>(serde_json::json!({
            "operation": "create", "name": "工作", "kind": "pinyin", "extra": 1
        }))
        .is_err()
    );
}

fn shuishan(weight: i64) -> PersonalWord {
    PersonalWord {
        kind: PersonalWordKind::Pinyin,
        key: "shui'shan".into(),
        value: "水杉".into(),
        weight,
    }
}

#[test]
fn collections_never_overwrite_or_remove_a_word_the_user_already_had() {
    const OWN_WEIGHT: i64 = 4_321;
    const COLLECTION_WEIGHT: i64 = 10_000;
    let other = PersonalWord {
        kind: PersonalWordKind::Pinyin,
        key: "ni'hao".into(),
        value: "你好".into(),
        weight: COLLECTION_WEIGHT,
    };
    for ending in ["disable", "delete", "remove_words"] {
        let mut fixture = Fixture::new();
        let own = shuishan(OWN_WEIGHT);
        fixture.dictionary.insert(own.identity(), OWN_WEIGHT);
        let view = fixture
            .store
            .create("拼音", PersonalWordKind::Pinyin)
            .unwrap();
        let id = id_of(&view, "拼音");
        fixture
            .store
            .add_words(&id, vec![shuishan(COLLECTION_WEIGHT), other.clone()])
            .unwrap();
        fixture.drain();
        assert_eq!(
            fixture.dictionary.get(&own.identity()),
            Some(&OWN_WEIGHT),
            "{ending}: 用户自己的权重不能被集合改掉"
        );
        assert!(fixture.has(&other));

        match ending {
            "disable" => {
                fixture.store.set_enabled(&id, false).unwrap();
            }
            "delete" => {
                fixture.store.delete(&id).unwrap();
            }
            _ => {
                fixture
                    .store
                    .remove_words(&id, &[shuishan(COLLECTION_WEIGHT), other.clone()])
                    .unwrap();
            }
        }
        fixture.drain();
        assert_eq!(
            fixture.dictionary.get(&own.identity()),
            Some(&OWN_WEIGHT),
            "{ending}: 用户原有的词不能随集合删掉"
        );
        assert!(!fixture.has(&other), "{ending}: 集合自己加的词照常删掉");
        assert!(
            fixture.requests().iter().all(|request| request
                .previous
                .as_ref()
                .is_none_or(|word| word.identity() != own.identity())),
            "{ending}: 不应有删除用户原有词的请求"
        );
    }
}

#[test]
fn collections_a_removal_queued_before_the_receipt_is_not_sent() {
    let mut fixture = Fixture::new();
    let own = shuishan(4_321);
    fixture.dictionary.insert(own.identity(), own.weight);
    let view = fixture
        .store
        .create("拼音", PersonalWordKind::Pinyin)
        .unwrap();
    let id = id_of(&view, "拼音");
    // 加入已经送进个人词库队列，键盘还没应用就停用了集合：「删除」只能排着，等回执到了再丢掉。
    fixture
        .store
        .add_words(&id, vec![shuishan(10_000)])
        .unwrap();
    assert_eq!(fixture.requests().len(), 1);
    fixture.store.set_enabled(&id, false).unwrap();
    fixture.drain();
    assert_eq!(fixture.dictionary.get(&own.identity()), Some(&own.weight));
    assert!(fixture
        .requests()
        .iter()
        .all(|request| request.previous.is_none()));
    assert!(fixture
        .personal()
        .read()
        .unwrap()
        .already_present
        .is_empty());
}

#[test]
fn collections_a_word_the_user_deleted_by_hand_is_the_collection_s_again() {
    let mut fixture = Fixture::new();
    let own = shuishan(4_321);
    fixture.dictionary.insert(own.identity(), own.weight);
    let view = fixture
        .store
        .create("拼音", PersonalWordKind::Pinyin)
        .unwrap();
    let id = id_of(&view, "拼音");
    fixture
        .store
        .add_words(&id, vec![shuishan(10_000)])
        .unwrap();
    fixture.drain();
    fixture.store.set_enabled(&id, false).unwrap();
    fixture.drain();
    // 用户在个人词库页手动删掉了它；再启用时集合真正写进了这个词，之后停用就照常删掉。
    fixture.dictionary.remove(&own.identity());
    fixture.store.set_enabled(&id, true).unwrap();
    fixture.drain();
    assert_eq!(fixture.dictionary.get(&own.identity()), Some(&10_000));
    fixture.store.set_enabled(&id, false).unwrap();
    fixture.drain();
    assert!(!fixture.has(&own));
}
