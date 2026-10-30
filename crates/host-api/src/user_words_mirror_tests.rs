use super::*;
use crate::tests::host_options_with_main_dictionary;

const T0: u64 = 1_790_000_000_000;

fn word(code: &str, value: &str) -> WordKey {
    WordKey::new(WordKind::Pinyin, code, value).unwrap()
}

/// 内存里的本机词库：可以让它忙、让它不收某些词，并记下每一个写入的请求编号。
#[derive(Default)]
struct Memory {
    words: BTreeSet<WordKey>,
    busy: bool,
    refused: BTreeSet<WordKey>,
    requests: Vec<String>,
}

impl Memory {
    fn with(words: &[&WordKey]) -> Self {
        Self {
            words: words.iter().map(|key| (*key).clone()).collect(),
            ..Self::default()
        }
    }
}

impl LocalDictionary for Memory {
    fn words(&mut self) -> Result<BTreeSet<WordKey>, LocalError> {
        Ok(self.words.clone())
    }

    fn apply(&mut self, changes: &[LocalChange]) -> Result<Vec<LocalOutcome>, LocalError> {
        if self.busy {
            return Err(LocalError::Busy);
        }
        assert!(changes.len() <= APPLY_BATCH);
        Ok(changes
            .iter()
            .map(|change| {
                assert!(change.request_id.starts_with("umirror-") && change.request_id.len() == 40);
                self.requests.push(change.request_id.clone());
                if self.refused.contains(&change.key) {
                    return LocalOutcome::Rejected;
                }
                if change.add {
                    self.words.insert(change.key.clone());
                } else {
                    self.words.remove(&change.key);
                }
                LocalOutcome::Applied
            })
            .collect())
    }
}

/// 一方：词库、私有状态和它最近一次的发布。键盘在基础访问模式下不发布。
struct Side {
    side: super::Side,
    local: Memory,
    state: MirrorState,
    published: Option<Publication>,
    publishes: bool,
}

impl Side {
    fn new(side: super::Side, words: &[&WordKey]) -> Self {
        Self {
            side,
            local: Memory::with(words),
            state: MirrorState::default(),
            published: None,
            publishes: true,
        }
    }

    /// 一次 sync，状态和发布都经过编码再解码，和真的写盘一样。
    fn sync(&mut self, theirs: &Side, now: u64, max_changes: usize) -> Report {
        let (report, failure) = run(
            self.side,
            &mut self.state,
            &mut self.local,
            theirs.published.as_ref(),
            max_changes,
            now,
        );
        assert_eq!(failure, None);
        self.state = MirrorState::decode(&self.state.encode().unwrap()).unwrap();
        if self.publishes {
            let publication = self.state.publication(self.side);
            self.published = Some(Publication::decode(&publication.encode().unwrap()).unwrap());
        }
        report
    }
}

fn pair(settings: &[&WordKey], keyboard: &[&WordKey]) -> (Side, Side) {
    (
        Side::new(super::Side::Settings, settings),
        Side::new(super::Side::Keyboard, keyboard),
    )
}

/// 两边轮流各同步几次，直到不再有任何改动。
fn settle(settings: &mut Side, keyboard: &mut Side, mut now: u64) -> u64 {
    for _ in 0..4 {
        now += 1000;
        settings.sync(keyboard, now, MAX_CHANGES);
        now += 1000;
        keyboard.sync(settings, now, MAX_CHANGES);
    }
    now
}

#[test]
fn the_first_run_merges_both_copies_and_never_deletes_a_word() {
    let (shared, settings_only, keyboard_only) = (
        word("ni'hao", "你好"),
        word("ni'men", "你们"),
        word("nan'ren", "男人"),
    );
    let (mut settings, mut keyboard) = pair(&[&shared, &settings_only], &[&shared, &keyboard_only]);
    // 键盘先跑：设置应用还没发布过，键盘只记录自己的词。
    let report = keyboard.sync(&settings, T0, MAX_CHANGES);
    assert_eq!((report.captured, report.applied), (2, 0));
    let report = settings.sync(&keyboard, T0 + 10, MAX_CHANGES);
    assert_eq!((report.captured, report.applied), (2, 1));
    let report = keyboard.sync(&settings, T0 + 20, MAX_CHANGES);
    assert_eq!((report.captured, report.applied), (0, 1));
    let everything: BTreeSet<WordKey> = [shared, settings_only, keyboard_only].into();
    assert_eq!(settings.local.words, everything);
    assert_eq!(keyboard.local.words, everything);
    // 已经一致时再同步什么都不做。
    let report = settings.sync(&keyboard, T0 + 30, MAX_CHANGES);
    assert_eq!(
        (report.captured, report.applied, report.deferred),
        (0, 0, 0)
    );
    let report = keyboard.sync(&settings, T0 + 40, MAX_CHANGES);
    assert_eq!(
        (report.captured, report.applied, report.deferred),
        (0, 0, 0)
    );
}

#[test]
fn a_deletion_on_one_side_and_an_addition_on_the_other_both_survive() {
    let (old, learned) = (word("ni'hao", "你好"), word("nan'ren", "男人"));
    let (mut settings, mut keyboard) = pair(&[&old], &[&old]);
    let now = settle(&mut settings, &mut keyboard, T0);
    // 设置页删掉一个词；同一段时间里键盘学到一个新词。
    settings.local.words.remove(&old);
    keyboard.local.words.insert(learned.clone());
    settle(&mut settings, &mut keyboard, now);
    let expected: BTreeSet<WordKey> = [learned].into();
    assert_eq!(settings.local.words, expected);
    assert_eq!(keyboard.local.words, expected);
}

#[test]
fn the_later_change_to_the_same_word_wins_whichever_side_merges_first() {
    let shared = word("ni'hao", "你好");
    for keyboard_first in [false, true] {
        let (mut settings, mut keyboard) = pair(&[&shared], &[&shared]);
        let now = settle(&mut settings, &mut keyboard, T0);
        // 键盘删掉（较早），然后设置应用里删了又加回来（较晚）。
        keyboard.local.words.remove(&shared);
        keyboard.sync(&settings, now + 1000, MAX_CHANGES);
        settings.local.words.remove(&shared);
        settings.sync(&keyboard, now + 2000, MAX_CHANGES);
        settings.local.words.insert(shared.clone());
        if keyboard_first {
            keyboard.sync(&settings, now + 3000, MAX_CHANGES);
        }
        settings.sync(&keyboard, now + 4000, MAX_CHANGES);
        settle(&mut settings, &mut keyboard, now + 5000);
        assert!(settings.local.words.contains(&shared), "{keyboard_first}");
        assert!(keyboard.local.words.contains(&shared), "{keyboard_first}");
    }
}

#[test]
fn concurrent_opposite_changes_converge_instead_of_swapping_back_and_forth() {
    let (a, b) = (word("ni'hao", "你好"), word("ni'men", "你们"));
    let (mut settings, mut keyboard) = pair(&[&a], &[&a]);
    let now = settle(&mut settings, &mut keyboard, T0);
    // 两边在合并对方之前各改一次：设置应用删 a、加 b；键盘删 a（记下了），之后又把 a 加回来、也加了 b。
    settings.local.words.remove(&a);
    settings.local.words.insert(b.clone());
    settings.sync(&keyboard, now + 1000, MAX_CHANGES);
    keyboard.local.words.remove(&a);
    keyboard.record(now + 1500);
    keyboard.local.words.insert(a.clone());
    keyboard.local.words.insert(b.clone());
    let now = settle(&mut settings, &mut keyboard, now + 2000);
    // 键盘把 a 加回来是最后一次改动，所以 a 在；b 两边都加了。
    assert_eq!(settings.local.words, [a, b].into());
    assert_eq!(keyboard.local.words, settings.local.words);
    // 收敛以后再同步不再写任何东西。
    let before = (settings.local.requests.len(), keyboard.local.requests.len());
    settle(&mut settings, &mut keyboard, now);
    assert_eq!(
        before,
        (settings.local.requests.len(), keyboard.local.requests.len())
    );
}

impl Side {
    /// 只记录本机改动、不合并对方，相当于 `publish` 操作。
    fn record(&mut self, now: u64) {
        let (_, failure) = run(
            self.side,
            &mut self.state,
            &mut self.local,
            None,
            MAX_CHANGES,
            now,
        );
        assert_eq!(failure, None);
        if self.publishes {
            self.published = Some(self.state.publication(self.side));
        }
    }
}

#[test]
fn in_basic_mode_settings_changes_reach_the_keyboard_and_its_own_words_stay() {
    let (shared, from_settings, learned) = (
        word("ni'hao", "你好"),
        word("ni'men", "你们"),
        word("nan'ren", "男人"),
    );
    let (mut settings, mut keyboard) = pair(&[&shared], &[&shared]);
    keyboard.publishes = false;
    let now = settle(&mut settings, &mut keyboard, T0);
    settings.local.words.remove(&shared);
    settings.local.words.insert(from_settings.clone());
    keyboard.local.words.insert(learned.clone());
    settle(&mut settings, &mut keyboard, now);
    assert_eq!(
        keyboard.local.words,
        [from_settings.clone(), learned].into()
    );
    // 键盘写不了共享目录，它学到的词到不了设置应用。
    assert_eq!(settings.local.words, [from_settings].into());
}

#[test]
fn a_busy_dictionary_or_the_change_limit_leaves_the_rest_for_next_time() {
    let words: Vec<WordKey> = ["ni", "nan", "nai", "nao", "neng"]
        .iter()
        .map(|code| word(code, "字"))
        .collect();
    let references: Vec<&WordKey> = words.iter().collect();
    let (mut settings, mut keyboard) = pair(&references, &[]);
    settings.sync(&keyboard, T0, MAX_CHANGES);
    keyboard.local.busy = true;
    let report = keyboard.sync(&settings, T0 + 10, MAX_CHANGES);
    assert_eq!((report.applied, report.deferred), (0, 5));
    assert!(keyboard.local.words.is_empty());
    assert_eq!(keyboard.state.merged, 0);
    keyboard.local.busy = false;
    let report = keyboard.sync(&settings, T0 + 20, 2);
    assert_eq!((report.applied, report.deferred), (2, 3));
    assert_eq!(keyboard.state.merged, 0);
    let report = keyboard.sync(&settings, T0 + 30, 2);
    assert_eq!((report.applied, report.deferred), (2, 1));
    let report = keyboard.sync(&settings, T0 + 40, 2);
    assert_eq!((report.applied, report.deferred), (1, 0));
    assert_eq!(keyboard.local.words.len(), 5);
    // 全部写好之后才确认合并到了设置应用的哪一版。
    assert_eq!(
        keyboard.state.merged,
        settings.published.as_ref().unwrap().revision
    );
    // 分批写进来的词不会被当成键盘自己的改动再发回去。
    let report = keyboard.sync(&settings, T0 + 50, MAX_CHANGES);
    assert_eq!(report.captured, 0);
}

#[test]
fn a_word_the_engine_refuses_is_not_sent_back_as_a_deletion() {
    let (good, odd) = (word("ni'men", "你们"), word("xq", "怪"));
    let (mut settings, mut keyboard) = pair(&[&good, &odd], &[]);
    keyboard.local.refused.insert(odd.clone());
    settings.sync(&keyboard, T0, MAX_CHANGES);
    let report = keyboard.sync(&settings, T0 + 10, MAX_CHANGES);
    assert_eq!((report.applied, report.rejected), (1, 1));
    settle(&mut settings, &mut keyboard, T0 + 20);
    assert!(settings.local.words.contains(&odd));
    assert_eq!(keyboard.local.words, [good].into());
    // 只试过那一次。
    let tries = keyboard
        .local
        .requests
        .iter()
        .filter(|id| **id == request_id(&odd, true, settings.state.registers[&odd].stamp))
        .count();
    assert_eq!(tries, 1);
}

#[test]
fn a_word_the_engine_cannot_delete_is_not_sent_back_as_a_new_word() {
    let stuck = word("ni'hao", "你好");
    let (mut settings, mut keyboard) = pair(&[&stuck], &[&stuck]);
    let now = settle(&mut settings, &mut keyboard, T0);
    settings.local.words.remove(&stuck);
    keyboard.local.refused.insert(stuck.clone());
    settle(&mut settings, &mut keyboard, now);
    assert!(!settings.local.words.contains(&stuck));
    assert!(keyboard.local.words.contains(&stuck));
    assert!(!keyboard.state.registers[&stuck].exists);
}

#[test]
fn a_reset_dictionary_does_not_delete_the_words_on_the_other_side() {
    let words: Vec<WordKey> = (0..60)
        .map(|index| {
            WordKey::new(
                WordKind::English,
                &format!("word{}", "x".repeat(index + 1)),
                "w",
            )
            .unwrap()
        })
        .collect();
    let references: Vec<&WordKey> = words.iter().collect();
    let (mut settings, mut keyboard) = pair(&references, &references);
    let now = settle(&mut settings, &mut keyboard, T0);
    // 键盘的日志坏了、换成了空的。
    keyboard.local.words.clear();
    let report = keyboard.sync(&settings, now + 1000, MAX_CHANGES);
    assert_eq!((report.captured, report.held_deletions), (0, 60));
    settle(&mut settings, &mut keyboard, now + 2000);
    assert_eq!(settings.local.words.len(), 60);
    // 键盘这边也不会被自动写回去：那可能就是用户自己清空的。
    assert!(keyboard.local.words.is_empty());
    // 少量删除照常传过去。
    let mut few = Side::new(super::Side::Keyboard, &references[..10]);
    let mut other = Side::new(super::Side::Settings, &references[..10]);
    let now = settle(&mut other, &mut few, now + 3000);
    few.local.words.clear();
    let now = settle(&mut other, &mut few, now);
    assert!(other.local.words.is_empty());
    // 设置应用一侧的大批删除是用户的操作（停用一个命名词库、清除学习数据），照常传给键盘。
    let mut settings = Side::new(super::Side::Settings, &references);
    let mut keyboard = Side::new(super::Side::Keyboard, &references);
    let now = settle(&mut settings, &mut keyboard, now);
    settings.local.words.clear();
    let report = settings.sync(&keyboard, now + 1000, MAX_CHANGES);
    assert_eq!((report.captured, report.held_deletions), (60, 0));
    settle(&mut settings, &mut keyboard, now + 2000);
    assert!(keyboard.local.words.is_empty());
}

#[test]
fn tombstones_are_collected_only_after_the_other_side_has_seen_them() {
    let gone = word("ni'hao", "你好");
    let (mut settings, mut keyboard) = pair(&[&gone], &[&gone]);
    let now = settle(&mut settings, &mut keyboard, T0);
    settings.local.words.remove(&gone);
    settings.sync(&keyboard, now + 1000, MAX_CHANGES);
    assert!(settings.state.registers.contains_key(&gone));
    // 键盘在基础访问模式下合并了但发布不出来：设置应用不知道它见过，墓碑留着。
    keyboard.publishes = false;
    keyboard.sync(&settings, now + 2000, MAX_CHANGES);
    assert!(!keyboard.local.words.contains(&gone));
    settings.sync(&keyboard, now + 3000, MAX_CHANGES);
    assert!(settings.state.registers.contains_key(&gone));
    // 能发布以后，键盘确认见过，两边各自回收。
    keyboard.publishes = true;
    settle(&mut settings, &mut keyboard, now + 4000);
    assert!(!settings.state.registers.contains_key(&gone));
    assert!(!keyboard.state.registers.contains_key(&gone));
    assert!(settings.local.words.is_empty() && keyboard.local.words.is_empty());
}

#[test]
fn tombstones_beyond_the_cap_drop_the_oldest() {
    let mut state = MirrorState::default();
    for index in 0..MAX_TOMBSTONES + 3 {
        state.registers.insert(
            WordKey::new(WordKind::Quick, &format!("q{index}"), "x").unwrap(),
            Register {
                stamp: Hlc(MIN_STAMP + index as u64),
                exists: false,
                applied: false,
            },
        );
    }
    collect(&mut state, None);
    assert_eq!(state.registers.len(), MAX_TOMBSTONES);
    for index in 0..3 {
        assert!(!state
            .registers
            .contains_key(&WordKey::new(WordKind::Quick, &format!("q{index}"), "x").unwrap()));
    }
}

const MIN_STAMP: u64 = 1 << 10;

#[test]
fn unreadable_documents_are_refused_and_state_starts_over() {
    assert!(MirrorState::decode(b"{}").is_none());
    assert!(Publication::decode(b"not json").is_none());
    // 编码不规范的词条（大写拼音）整份不认。
    let odd = r#"{"format":"msime-user-words","version":1,"publisher":"settings","revision":5000,"merged":0,"entries":[["pinyin","NI'HAO","你好",2000,true]]}"#.as_bytes();
    assert!(Publication::decode(odd).is_none());
    // 时间戳比发布的修订号还大，不可能。
    let ahead = r#"{"format":"msime-user-words","version":1,"publisher":"settings","revision":1500,"merged":0,"entries":[["pinyin","ni'hao","你好",2000,true]]}"#.as_bytes();
    assert!(Publication::decode(ahead).is_none());
    let fine = r#"{"format":"msime-user-words","version":1,"publisher":"keyboard","revision":5000,"merged":0,"entries":[["pinyin","ni'hao","你好",2000,true],["quick","addr","第一行\n第二行",3000,false]]}"#.as_bytes();
    let publication = Publication::decode(fine).unwrap();
    assert_eq!(publication.entries.len(), 2);
    assert_eq!(
        Publication::decode(&publication.encode().unwrap()).unwrap(),
        publication
    );
}

fn call(body: Value) -> Value {
    let bytes = body.to_string();
    // SAFETY: 指针和长度来自同一个活着的字符串；返回的响应随即释放。
    let response = unsafe { crate::msime_client_user_words_mirror(bytes.as_ptr(), bytes.len()) };
    let text = unsafe { std::ffi::CStr::from_ptr(response) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(response) };
    serde_json::from_str(&text).unwrap()
}

fn seed(options: &Value, entries: &[(&str, &str, &str)]) {
    for (index, (kind, key, value)) in entries.iter().enumerate() {
        let edited = unsafe_call(
            crate::msime_client_dictionary,
            json!({"options": options, "action": {"operation": "edit", "previous": null, "replacement": {"kind": kind, "key": key, "value": value, "weight": 100}, "request_id": format!("seed-{}-{index}", key.replace('\'', "-"))}}),
        );
        assert_eq!(edited["ok"], true, "{edited}");
    }
}

fn unsafe_call(
    function: unsafe extern "C" fn(*const u8, usize) -> *mut c_char,
    body: Value,
) -> Value {
    let bytes = body.to_string();
    // SAFETY: 同上。
    let response = unsafe { function(bytes.as_ptr(), bytes.len()) };
    let text = unsafe { std::ffi::CStr::from_ptr(response) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(response) };
    serde_json::from_str(&text).unwrap()
}

fn my_words(options: &Value) -> BTreeSet<WordKey> {
    let engine = HostOptions::from_document(options.clone())
        .unwrap()
        .into_engine_options();
    EngineDictionary::new(&engine, false).words().unwrap()
}

#[test]
fn two_engine_dictionaries_converge_through_their_publications() {
    let root = tempfile::tempdir().unwrap();
    let settings = host_options_with_main_dictionary(&root.path().join("settings"));
    let keyboard = host_options_with_main_dictionary(&root.path().join("keyboard"));
    seed(
        &settings,
        &[
            ("pinyin", "ni'men", "你们"),
            ("quick_phrase", "addr", "第一行\n第二行"),
        ],
    );
    seed(
        &keyboard,
        &[("pinyin", "nan'ren", "男人"), ("english", "dont", "don't")],
    );
    let group = root.path().join("group");
    let settings_file = group.join("settings/user-words/words.json");
    let keyboard_file = group.join("keyboard/user-words/words.json");
    let sync = |options: &Value,
                side: &str,
                own: &std::path::Path,
                theirs: &std::path::Path,
                private: &str| {
        call(json!({
            "operation": "sync",
            "side": side,
            "options": options,
            "directory": root.path().join(private).to_str().unwrap(),
            "theirs": theirs.to_str().unwrap(),
            "publish": own.to_str().unwrap(),
        }))
    };
    let first = sync(
        &keyboard,
        "keyboard",
        &keyboard_file,
        &settings_file,
        "keyboard-mirror",
    );
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(first["value"]["theirs"], "absent");
    assert_eq!(first["value"]["captured"], 2);
    let second = sync(
        &settings,
        "settings",
        &settings_file,
        &keyboard_file,
        "settings-mirror",
    );
    assert_eq!(second["value"]["applied"], 2, "{second}");
    let third = sync(
        &keyboard,
        "keyboard",
        &keyboard_file,
        &settings_file,
        "keyboard-mirror",
    );
    assert_eq!(third["value"]["applied"], 2, "{third}");
    assert_eq!(my_words(&settings), my_words(&keyboard));
    assert_eq!(my_words(&settings).len(), 4);

    // 键盘的输入会话开着时 sync 写不进去，留到下一次；publish 照样能记下键盘自己的改动。
    seed(&settings, &[("pinyin", "ni'hao", "你好")]);
    sync(
        &settings,
        "settings",
        &settings_file,
        &keyboard_file,
        "settings-mirror",
    );
    let engine = HostOptions::from_document(keyboard.clone())
        .unwrap()
        .into_engine_options();
    let session = DictionaryAccess::try_session(
        std::path::Path::new(&engine.user_data),
        std::path::Path::new(&engine.dictionaries),
    )
    .unwrap()
    .unwrap();
    let busy = sync(
        &keyboard,
        "keyboard",
        &keyboard_file,
        &settings_file,
        "keyboard-mirror",
    );
    assert_eq!(busy["value"]["deferred"], 1, "{busy}");
    let published = call(json!({
        "operation": "publish",
        "side": "keyboard",
        "options": keyboard,
        "directory": root.path().join("keyboard-mirror").to_str().unwrap(),
        "publish": keyboard_file.to_str().unwrap(),
    }));
    assert_eq!(published["value"]["published"], true, "{published}");
    drop(session);
    let done = sync(
        &keyboard,
        "keyboard",
        &keyboard_file,
        &settings_file,
        "keyboard-mirror",
    );
    assert_eq!(
        (
            done["value"]["applied"].clone(),
            done["value"]["deferred"].clone()
        ),
        (json!(1), json!(0))
    );
    assert_eq!(my_words(&keyboard).len(), 5);
}

#[test]
fn requests_are_checked_before_anything_is_read_or_written() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let directory = root.path().join("mirror");
    let base = json!({
        "operation": "sync",
        "side": "settings",
        "options": options,
        "directory": directory.to_str().unwrap(),
    });
    let with = |field: &str, value: Value| {
        let mut body = base.clone();
        body[field] = value;
        body
    };
    for body in [
        with("directory", json!("relative/dir")),
        with("theirs", json!("relative.json")),
        with("side", json!("panel")),
        with("operation", json!("delete_everything")),
        with("max_changes", json!(0)),
        with("max_changes", json!(MAX_CHANGES + 1)),
        with("unexpected", json!(true)),
        {
            let same = json!(root.path().join("same.json").to_str().unwrap());
            let mut body = with("theirs", same.clone());
            body["publish"] = same;
            body
        },
    ] {
        let reply = call(body);
        assert_eq!(reply["error"], INVALID, "{reply}");
    }
    assert!(!directory.exists());
    let reply = unsafe { crate::msime_client_user_words_mirror(std::ptr::null(), 0) };
    let text = unsafe { std::ffi::CStr::from_ptr(reply) }
        .to_str()
        .unwrap()
        .to_owned();
    unsafe { crate::msime_client_string_free(reply) };
    assert!(text.contains(INVALID));
    // 发布方与 side 相同的发布不合并。
    let own = root.path().join("own.json");
    std::fs::write(
        &own,
        r#"{"format":"msime-user-words","version":1,"publisher":"settings","revision":5000,"merged":0,"entries":[]}"#.as_bytes(),
    )
    .unwrap();
    let reply = call(with("theirs", json!(own.to_str().unwrap())));
    assert_eq!(reply["value"]["theirs"], "unreadable", "{reply}");
}

#[test]
fn a_second_call_on_the_same_directory_is_told_to_wait() {
    let root = tempfile::tempdir().unwrap();
    let options = host_options_with_main_dictionary(root.path());
    let directory = root.path().join("mirror");
    std::fs::create_dir_all(&directory).unwrap();
    let lock =
        msime_client_core::file_lock::open_private_lock_file(directory.join(LOCK_FILE)).unwrap();
    let _held = msime_client_core::file_lock::ScopedFileLock::try_exclusive(lock)
        .unwrap()
        .unwrap();
    let reply = call(json!({
        "operation": "publish",
        "side": "keyboard",
        "options": options,
        "directory": directory.to_str().unwrap(),
    }));
    assert_eq!(reply["error"], BUSY, "{reply}");
}
