//! 原生宿主调用的 C ABI（`crates/host-api`）：打开一个会话，把输入解码成一串宿主调用灌进去，最后关掉。每一步都经过 `host-api` → `input-runtime` → `engine` 的真实路径，与平台宿主收到按键时一样；回应按宿主的方式读出并交回 `msime_client_string_free`。
//!
//! 输入的第一个字节选输入方案和触屏布局，之后每个字节是一步：
//!
//! - 可打印 ASCII：标点走 `msime_client_punctuation`，其余（字母、数字、空格）走 `msime_client_character`。
//! - `0x00`–`0x1f`：`msime_client_command` 的编辑与翻页命令。
//! - `0x7f`：下一个字节作为按住 Shift 的字符。
//! - `0x80`–`0xff`：选词、开关、滑行、英文补全、九键筛选、中途换方案等，参数从后面的字节取（见 `step`）。
//!
//! 每轮在新的临时目录里放一份只有几个词条的拼音词库，学习写下的东西不会带到下一轮，崩溃输入因此能单独复现。panic 即使被 `response()` 的 `catch_unwind` 接住，也会先触发 libfuzzer-sys 装的 panic hook 中止进程，所以照样报告。

#![no_main]

use std::ffi::{c_char, CStr};
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use msime_client_core::preferences::Preferences;
use msime_host_api::*;
use serde_json::{json, Value};

/// 第一个字节选的方案与触屏布局。日文、粤拼、注音、笔画在夹具里没有对应的词库，缺词库时的处理也是要测的路径。
const SCHEMES: [(&str, &str); 12] = [
    ("quanpin", "twenty_six_key"),
    ("quanpin", "nine_key"),
    ("quanpin", "fourteen_key"),
    ("shuangpin", "twenty_six_key"),
    ("wubi", "twenty_six_key"),
    ("japanese", "twenty_six_key"),
    ("korean", "twenty_six_key"),
    ("cantonese", "twenty_six_key"),
    ("zhuyin", "twenty_six_key"),
    ("vietnamese", "twenty_six_key"),
    ("tibetan", "twenty_six_key"),
    ("stroke", "twenty_six_key"),
];

/// `msime_client_command` 认得的全部命令号。
const COMMANDS: [u32; 25] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 100, 101, 102, 103, 104, 105,
];

/// 拼音、五笔和快捷短语各几条，足以让常见编码出候选、组句、分页和学习。表结构与 `crates/host-api/src/tests.rs` 的会话测试相同。
const DICTIONARY: &str = "
    CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_1_n VALUES('ni','n','你',100);
    INSERT INTO tbl_1_n VALUES('ni','n','泥',60);
    CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_1_h VALUES('hao','h','好',100);
    INSERT INTO tbl_1_h VALUES('hao','h','号',70);
    CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_1_b VALUES('bu','b','不',90);
    CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_1_g VALUES('gao','g','高',80);
    CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_1_z VALUES('zhong','z','中',100);
    CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_1_w VALUES('wen','w','文',100);
    CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);
    CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);
    INSERT INTO tbl_2_z VALUES('zhong''wen','zw','中文',900);
    CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
    INSERT INTO wubi86 VALUES('wq','你',100);
    INSERT INTO wubi86 VALUES('vb','好',100);
    INSERT INTO wubi86 VALUES('k','中',100);
    CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
    INSERT INTO quick_parases VALUES('zj','此致敬礼',100);
    CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);
";

/// 词库文件的字节，只建一次，之后每轮复制一份。
fn dictionary() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(|| {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("msime-pinyin.db");
        rusqlite::Connection::open(&path)
            .and_then(|connection| connection.execute_batch(DICTIONARY))
            .expect("build the fixture dictionary");
        std::fs::read(&path).expect("read the fixture dictionary")
    })
}

fn preferences(scheme: &str, layout: &str) -> Value {
    let mut preferences =
        serde_json::to_value(Preferences::default()).expect("preferences serialize");
    preferences["default_ime_mode"] = json!("chinese");
    preferences["scheme"] = json!(scheme);
    preferences["touch_keyboard_layout"] = json!(layout);
    preferences
}

/// 读出一个回应并按宿主的约定交回。回应必须是带 `ok` 的 JSON：宿主只认这个形状。
fn take(response: *mut c_char) -> Value {
    assert!(!response.is_null(), "host API returned a null response");
    // SAFETY: host-api 的回应是 `CString::into_raw` 交出的、以 NUL 结尾的字符串，交回之前一直有效。
    let bytes = unsafe { CStr::from_ptr(response) }.to_bytes().to_vec();
    // SAFETY: 同一个指针，只交回这一次。
    unsafe { msime_client_string_free(response) };
    let value: Value = serde_json::from_slice(&bytes).expect("response is JSON");
    assert!(value["ok"].is_boolean(), "response without ok: {value}");
    value
}

/// 按顺序取字节；取完之后一律给 0，让最后一步的参数也有值。
struct Input<'a> {
    data: &'a [u8],
}

impl Input<'_> {
    fn next(&mut self) -> Option<u8> {
        let (&first, rest) = self.data.split_first()?;
        self.data = rest;
        Some(first)
    }

    fn byte(&mut self) -> u8 {
        self.next().unwrap_or(0)
    }

    fn bytes(&mut self, length: usize) -> &[u8] {
        let (taken, rest) = self.data.split_at(length.min(self.data.len()));
        self.data = rest;
        taken
    }
}

/// QWERTY 上 `a`..`z` 各键的中心，键宽 10、键高 14，滑行请求用。
fn glide_keys() -> Value {
    let rows: [(&str, f64, f64); 3] = [
        ("qwertyuiop", 5.0, 7.0),
        ("asdfghjkl", 10.0, 21.0),
        ("zxcvbnm", 20.0, 35.0),
    ];
    let mut centres = [[0.0f64; 2]; 26];
    for (letters, left, y) in rows {
        for (column, letter) in letters.bytes().enumerate() {
            centres[usize::from(letter - b'a')] = [left + 10.0 * column as f64, y];
        }
    }
    json!(centres)
}

/// 执行一步，返回回应。
fn step(
    handle: u64,
    byte: u8,
    generation: u64,
    revision: &mut u64,
    input: &mut Input,
) -> *mut c_char {
    match byte {
        b' '..=b'~' if byte.is_ascii_punctuation() => msime_client_punctuation(handle, byte),
        b' '..=b'~' => msime_client_character(handle, byte, false),
        0x00..=0x1f => msime_client_command(handle, COMMANDS[usize::from(byte) % COMMANDS.len()]),
        0x7f => msime_client_character(handle, input.byte(), true),
        0x80..=0xff => match byte & 0x1f {
            0 => msime_client_select(handle, generation, usize::from(input.byte() % 12)),
            1 => msime_client_select_any_candidate(handle, generation, usize::from(input.byte())),
            2 => msime_client_pin_candidate(handle, generation, usize::from(input.byte() % 12)),
            3 => msime_client_remove_candidate(handle, generation, usize::from(input.byte() % 12)),
            4 => msime_client_choose_nine_key_spelling(
                handle,
                generation,
                usize::from(input.byte() % 12),
            ),
            5 => msime_client_grid_key(handle, input.byte()),
            6 => msime_client_set_candidate_page_size(handle, input.byte()),
            7 => msime_client_set_chinese_punctuation(handle, input.byte() & 1 == 1),
            8 => msime_client_set_paired_punctuation(handle, input.byte() & 1 == 1),
            9 => msime_client_set_punctuation_lock(handle, input.byte()),
            10 => msime_client_set_english_mode(handle, input.byte() & 1 == 1),
            11 => msime_client_set_caps_lock(handle, input.byte() & 1 == 1),
            12 => msime_client_set_private_session(handle, input.byte() & 1 == 1),
            13 => msime_client_set_nine_key_mode(handle, input.byte() & 1 == 1),
            14 => msime_client_set_key_grid(handle, input.byte()),
            15 => msime_client_set_character_width(handle, input.byte() & 1 == 1),
            16 => {
                let ascii = input.byte();
                let preceding = u32::from(input.byte()) | u32::from(input.byte()) << 8;
                msime_client_punctuation_with_context(handle, ascii, preceding)
            }
            17 => msime_client_punctuation_ascii(handle, input.byte()),
            18 => {
                let length = usize::from(input.byte() % 66);
                let limit = usize::from(input.byte() % 34);
                let prefix = input.bytes(length);
                // SAFETY: `prefix` 是 `length` 个以内的有效字节，调用期间不变。
                unsafe {
                    msime_client_english_completions(handle, prefix.as_ptr(), prefix.len(), limit)
                }
            }
            19 => {
                let single = input.byte() & 1 == 1;
                let length = usize::from(input.byte() % 70);
                let strokes = input.bytes(length);
                // SAFETY: 同上。
                unsafe {
                    msime_client_set_nine_key_filter(
                        handle,
                        single,
                        strokes.as_ptr(),
                        strokes.len(),
                    )
                }
            }
            20 => {
                let count = usize::from(input.byte() % 48);
                let points: Vec<Value> = (0..count)
                    .map(|index| {
                        json!([
                            f64::from(input.byte()) / 2.0,
                            f64::from(input.byte()) / 5.0,
                            index as u64 * 16
                        ])
                    })
                    .collect();
                let request = json!({"keys": glide_keys(), "key_width": 10.0, "key_height": 14.0, "points": points}).to_string();
                // SAFETY: 同上。
                unsafe { msime_client_glide(handle, request.as_ptr(), request.len()) }
            }
            21 => {
                let (scheme, layout) = SCHEMES[usize::from(input.byte()) % SCHEMES.len()];
                *revision += 1;
                let snapshot = json!({"format_version": 1, "revision": *revision, "preferences": preferences(scheme, layout)}).to_string();
                // SAFETY: 同上。
                unsafe {
                    msime_client_update_preferences(handle, snapshot.as_ptr(), snapshot.len())
                }
            }
            22 => msime_client_all_candidates(handle),
            23 => msime_client_rerank_settled(handle),
            24 => msime_client_focus(handle, input.byte() & 1 == 1),
            25 => msime_client_reset_cache(handle),
            _ => msime_client_view(handle),
        },
    }
}

fuzz_target!(|data: &[u8]| {
    let mut input = Input { data };
    let Some(first) = input.next() else {
        return;
    };
    let (scheme, layout) = SCHEMES[usize::from(first) % SCHEMES.len()];
    let root = tempfile::tempdir().expect("temporary directory");
    for name in ["resources", "user", "cache", "dictionaries"] {
        std::fs::create_dir_all(root.path().join(name)).expect("create fixture directory");
    }
    for name in ["resources", "dictionaries"] {
        std::fs::write(root.path().join(name).join("msime-pinyin.db"), dictionary())
            .expect("copy the fixture dictionary");
    }
    let options = json!({
        "api_version": 1,
        "resources": root.path().join("resources"),
        "user_data": root.path().join("user"),
        "cache": root.path().join("cache"),
        "dictionaries": root.path().join("dictionaries"),
        "preferences": preferences(scheme, layout),
        "preferences_directory": root.path(),
    })
    .to_string();
    // SAFETY: `options` 在调用期间有效。
    let created = take(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    let Some(handle) = created["value"]["session"].as_u64() else {
        return;
    };
    take(msime_client_focus(handle, true));
    let mut generation = 0;
    let mut revision = 0;
    while let Some(byte) = input.next() {
        let value = take(step(handle, byte, generation, &mut revision, &mut input));
        // 选词要带当前这一代的编号，否则一律被当作过期选择拒掉，碰不到选词之后的代码。
        if let Some(current) = value["value"]["view"]["generation"]
            .as_u64()
            .or_else(|| value["value"]["generation"].as_u64())
        {
            generation = current;
        }
    }
    take(msime_client_destroy(handle));
});
