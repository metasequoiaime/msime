//! 粤拼和粤语双拼：WebHost 的按键路由跑在 `fixture::write_cantonese` 写出的最小 `msime-cantonese.db` 上，并和桌面宿主的 `msime_input_runtime::Runtime` 逐键对照；最后核对演示页列出的粤语双拼键位和引擎的键位表一致。

#[path = "support/fixture.rs"]
mod fixture;

use std::path::Path;

use msime_engine::cantonese::shuangpin;
use msime_engine::host::{prepare_options_for, Session};
use msime_engine::{SchemeSet, SchemeType};
use msime_engine_wasm::host::{Frame, Key, Out, Row, Scheme, WebHost, CANTONESE_DICTIONARY};
use msime_input_runtime::{Action, Runtime};
use tempfile::TempDir;

const PAGE_SIZE: usize = 9;

/// 一个会话和它的三个临时目录；目录随它一起删掉。
struct Fixture {
    host: WebHost,
    _resources: TempDir,
    _user: TempDir,
    _cache: TempDir,
}

fn open(scheme: Scheme) -> Fixture {
    let resources = tempfile::tempdir().expect("resource directory");
    fixture::write_cantonese(&resources.path().join(CANTONESE_DICTIONARY)).expect("fixture db");
    let user = tempfile::tempdir().expect("user directory");
    let cache = tempfile::tempdir().expect("cache directory");
    let host = WebHost::new_with_paths(
        scheme,
        PAGE_SIZE,
        None,
        resources.path(),
        user.path(),
        cache.path(),
    )
    .expect("session on the fixture");
    Fixture {
        host,
        _resources: resources,
        _user: user,
        _cache: cache,
    }
}

/// 小写字母、数字、空格，其余可打印 ASCII 是标点。
fn typed(text: &str) -> Vec<Key> {
    text.bytes()
        .map(|byte| match byte {
            b'a'..=b'z' => Key::Letter(byte),
            b'0'..=b'9' => Key::Digit(byte),
            b' ' => Key::Space,
            _ => Key::Punct(byte),
        })
        .collect()
}

fn type_text(host: &mut WebHost, text: &str) -> Frame {
    host.keys(&typed(text))
}

fn commit(text: &str, seat: i32) -> Out {
    Out::Commit {
        text: text.to_owned(),
        seat,
    }
}

fn texts(page: &[Row]) -> Vec<&str> {
    page.iter().map(|row| row.text.as_str()).collect()
}

#[test]
fn jyutping_composes_and_space_commits_the_first_seat() {
    let mut fixture = open(Scheme::Jyutping);
    let frame = type_text(&mut fixture.host, "neihou");
    assert!(frame.composing);
    assert_eq!(texts(&frame.page), ["你好", "妳好", "你", "妳"]);
    // 不加载句子模型：粤语候选按词库自己的顺序。
    assert!(!frame.model_on);
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit("你好", 0)]);
    assert!(!frame.composing);
}

#[test]
fn a_digit_picks_its_seat_and_a_partial_word_leaves_the_rest_composing() {
    let mut fixture = open(Scheme::Jyutping);
    type_text(&mut fixture.host, "neihou");
    let frame = type_text(&mut fixture.host, "2");
    assert_eq!(frame.out, vec![commit("妳好", 1)]);
    // 只覆盖第一个音节的「你」上屏后，`hou` 接着组字。
    let frame = type_text(&mut fixture.host, "neihou3");
    assert_eq!(frame.out, vec![commit("你", 2)]);
    assert!(frame.composing);
    assert_eq!(texts(&frame.page), ["好", "號"]);
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit("好", 0)]);
    assert!(!frame.composing);
}

#[test]
fn an_apostrophe_separates_jyutping_syllables_while_composing() {
    let mut fixture = open(Scheme::Jyutping);
    let frame = type_text(&mut fixture.host, "nei'hou");
    assert!(frame.out.is_empty(), "{:?}", frame.out);
    assert_eq!(frame.page[0].text, "你好");
    // 空闲时撇号仍是标点。
    fixture.host.keys(&[Key::Escape]);
    let frame = type_text(&mut fixture.host, "'");
    assert_eq!(frame.out, vec![commit("‘", -1)]);
}

/// #10085 的同一组键：粤语双拼 `sxcr` 先给「相傳」，点选「上」上屏它、剩下的 `cr` 接着组字。粤拼方案前两键不是完整音节时自动按双拼读，结果相同。
#[test]
fn cantonese_shuangpin_reads_two_keys_per_syllable() {
    for scheme in [Scheme::CantoneseShuangpin, Scheme::Jyutping] {
        let mut fixture = open(scheme);
        let frame = type_text(&mut fixture.host, "sxcr");
        assert_eq!(texts(&frame.page), ["相傳", "上", "相", "削"], "{scheme:?}");
        let frame = fixture.host.pick(1);
        assert_eq!(frame.out, vec![commit("上", 1)], "{scheme:?}");
        assert!(frame.composing, "{scheme:?}");
        assert_eq!(texts(&frame.page), ["傳", "速"], "{scheme:?}");
        let frame = fixture.host.keys(&[Key::Space]);
        assert_eq!(frame.out, vec![commit("傳", 0)], "{scheme:?}");
    }
}

#[test]
fn a_semicolon_is_the_ing_final_in_cantonese_shuangpin() {
    let mut fixture = open(Scheme::CantoneseShuangpin);
    let frame = type_text(&mut fixture.host, "b;");
    assert!(frame.out.is_empty(), "{:?}", frame.out);
    assert_eq!(texts(&frame.page), ["兵", "冰"]);
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit("兵", 0)]);
    // 空闲时 `;` 是中文标点。
    let frame = type_text(&mut fixture.host, ";");
    assert_eq!(frame.out, vec![commit("；", -1)]);
    // 独立鼻音 m 是 `om`。
    let frame = type_text(&mut fixture.host, "omgo ");
    assert_eq!(frame.out, vec![commit("唔該", 0)]);
}

#[test]
fn punctuation_finishes_the_composition_with_the_highlighted_seat() {
    let mut fixture = open(Scheme::Jyutping);
    let frame = type_text(&mut fixture.host, "hoenggong,");
    assert_eq!(frame.out, vec![commit("香港", 0), commit("，", -1)]);
    assert!(!frame.composing);
}

/// 粤语方案读不到词库时创建失败，不会退回别的方案。
#[test]
fn a_cantonese_scheme_without_its_dictionary_fails_to_open() {
    let empty = tempfile::tempdir().expect("resource directory");
    let user = tempfile::tempdir().expect("user directory");
    let cache = tempfile::tempdir().expect("cache directory");
    for scheme in [Scheme::Jyutping, Scheme::CantoneseShuangpin] {
        assert!(
            WebHost::new_with_paths(
                scheme,
                PAGE_SIZE,
                None,
                empty.path(),
                user.path(),
                cache.path()
            )
            .is_err(),
            "{scheme:?}"
        );
    }
}

/// 桌面宿主的 Runtime，用同一份夹具词库，粤语双拼由 `force_cantonese_shuangpin` 决定（host-api 对独立的粤语双拼方案就是这样设的）。本地模式、混输和纠错按网页的 `session_options` 关掉。
fn runtime(dictionaries: &Path, state: &Path, forced: bool) -> Runtime {
    let text = |path: &Path| path.to_str().expect("UTF-8 path").to_owned();
    let resources = state.join("resources");
    std::fs::create_dir_all(&resources).expect("resource directory");
    let mut options = prepare_options_for(
        &text(&resources),
        &text(&state.join("user")),
        &text(&state.join("cache")),
        "web-engine-cantonese",
        SchemeSet::of(&[SchemeType::Cantonese]),
    )
    .expect("prepared options");
    options.scheme = SchemeType::Cantonese as u8;
    options.cantonese_dictionary = text(&dictionaries.join(CANTONESE_DICTIONARY));
    options.force_cantonese_shuangpin = forced;
    options.learning = false;
    options.autocorrect_transposition = false;
    options.autocorrect_neighbor = false;
    options.mixed_english = false;
    options.mixed_emoji = false;
    options.mixed_kaomoji = false;
    options.mixed_symbols = false;
    options.local_unicode = false;
    options.local_date_time = false;
    options.local_date_time_inline = false;
    options.local_date_time_menu = false;
    options.local_quick_phrase = false;
    options.local_emoji = false;
    options.local_kaomoji = false;
    options.local_super_jianpin = false;
    options.local_temporary_english = false;
    options.local_temporary_japanese = false;
    options.local_expression = false;
    options.local_command = false;
    options.local_mention = false;
    options.local_chaizi = false;
    options.local_symbol = false;
    options.local_shift_letter_entry = false;
    options.sentence_alternatives = true;
    options.first_page_rows = PAGE_SIZE;
    let page_size = u8::try_from(PAGE_SIZE).expect("page size");
    let mut runtime =
        Runtime::new(Session::new(&options).expect("session"), page_size).expect("runtime");
    runtime.focus(true).expect("focus");
    runtime
}

/// 网页和桌面宿主对同样的键给出同样的首页（字和编码），首选上屏同样的字；两边都调用引擎同一个粤语方案，网页不另写粤语规则。
#[test]
fn webhost_matches_the_desktop_runtime() {
    let cases: [(Scheme, &[&str]); 2] = [
        (
            Scheme::Jyutping,
            &[
                "n",
                "nei",
                "neihou",
                "nei'hou",
                "soeng",
                "sxcr",
                "mgoi",
                "hoenggong",
                "gwongdungwaa",
            ],
        ),
        (
            Scheme::CantoneseShuangpin,
            &[
                "n", "nz", "nzhb", "sx", "sxcr", "b;", "omgo", "hxgs", "isdpwa",
            ],
        ),
    ];
    for (scheme, inputs) in cases {
        let mut fixture = open(scheme);
        let dictionaries = tempfile::tempdir().expect("dictionary directory");
        fixture::write_cantonese(&dictionaries.path().join(CANTONESE_DICTIONARY))
            .expect("fixture db");
        let state = tempfile::tempdir().expect("state directory");
        let mut runtime = runtime(
            dictionaries.path(),
            state.path(),
            scheme == Scheme::CantoneseShuangpin,
        );
        for input in inputs {
            for byte in input.bytes() {
                runtime
                    .dispatch(Action::Character {
                        value: byte,
                        shift: false,
                    })
                    .expect("key");
            }
            let view = runtime.view();
            let expected: Vec<(String, String)> = view
                .candidates
                .iter()
                .map(|candidate| (candidate.text.clone(), candidate.code.clone()))
                .collect();
            let frame = type_text(&mut fixture.host, input);
            let actual: Vec<(String, String)> = frame
                .page
                .iter()
                .map(|row| (row.text.clone(), row.code.clone()))
                .collect();
            assert_eq!(actual, expected, "{scheme:?} {input:?}");
            assert!(!actual.is_empty(), "{scheme:?} {input:?} has no candidates");

            let expected_commit = view
                .candidates
                .first()
                .and_then(|candidate| {
                    runtime
                        .dispatch(Action::Select(candidate.id))
                        .expect("select")
                        .commit
                })
                .expect("runtime commit");
            let frame = fixture.host.keys(&[Key::Space]);
            assert_eq!(
                frame.out,
                vec![commit(&expected_commit, 0)],
                "{scheme:?} {input:?}"
            );
            // 首选只覆盖前几个音节时两边都还在组字；清掉，下一例从空闲开始。
            runtime
                .dispatch(Action::Command(msime_engine::host::Command::Cancel))
                .expect("cancel");
            fixture.host.keys(&[Key::Escape]);
        }
    }
}

/// 演示页（packages/web-engine/demo/site/demo.js）给访客看的粤语双拼键位表必须和引擎的 `cantonese::shuangpin::encode` 一致：列出的每个韵母都落在它写的键上，引擎认得的每个韵母都列了出来；声母表同理。
#[test]
fn the_demo_lists_the_engine_cantonese_shuangpin_layout() {
    let demo = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/web-engine/demo/site/demo.js"),
    )
    .expect("demo.js");
    let finals = demo_table(&demo, "CANTONESE_SHUANGPIN_FINALS");
    let initials = demo_table(&demo, "CANTONESE_SHUANGPIN_INITIALS");
    let syllabic = demo_table(&demo, "CANTONESE_SHUANGPIN_SYLLABIC");

    let mut listed = Vec::new();
    for (key, spellings) in &finals {
        let [key] = key.as_bytes() else {
            panic!("final key {key:?} is not one key");
        };
        for spelling in spellings.split(' ') {
            assert_eq!(
                shuangpin::encode(&format!("b{spelling}")),
                Some([b'b', *key]),
                "final {spelling} is not on {}",
                char::from(*key)
            );
            listed.push(spelling.to_owned());
        }
    }
    // 引擎认得的每个韵母（最长四个字母）都在表里。
    let mut unlisted = Vec::new();
    let mut spelling = Vec::new();
    for length in 1..=4u32 {
        for index in 0..26usize.pow(length) {
            spelling.clear();
            let mut rest = index;
            for _ in 0..length {
                spelling.push(b'a' + (rest % 26) as u8);
                rest /= 26;
            }
            let rest = std::str::from_utf8(&spelling).expect("ASCII");
            if shuangpin::encode(&format!("b{rest}")).is_some() && !listed.iter().any(|f| f == rest)
            {
                unlisted.push(rest.to_owned());
            }
        }
    }
    assert!(unlisted.is_empty(), "the demo misses finals {unlisted:?}");

    assert!(!initials.is_empty());
    for (initial, key) in &initials {
        let [key] = key.as_bytes() else {
            panic!("initial key {key:?} is not one key");
        };
        // 空串是零声母。
        assert_eq!(
            shuangpin::encode(&format!("{initial}aa")).map(|code| code[0]),
            Some(*key),
            "initial {initial:?}"
        );
    }
    assert!(!syllabic.is_empty());
    for (syllable, code) in &syllabic {
        assert_eq!(
            shuangpin::encode(syllable).as_ref().map(|code| &code[..]),
            Some(code.as_bytes()),
            "syllable {syllable}"
        );
    }
}

/// demo.js 里 `const <name> = [` 到 `];` 之间每行一个 `["…", "…"],`。
fn demo_table(demo: &str, name: &str) -> Vec<(String, String)> {
    let start = format!("const {name} = [");
    let mut lines = demo.lines().skip_while(|line| line.trim() != start).skip(1);
    let mut rows = Vec::new();
    for line in lines.by_ref() {
        let line = line.trim();
        if line == "];" {
            return rows;
        }
        let inner = line
            .strip_prefix("[\"")
            .and_then(|rest| rest.strip_suffix("\"],"))
            .unwrap_or_else(|| panic!("{name}: unexpected line {line:?}"));
        let (left, right) = inner
            .split_once("\", \"")
            .unwrap_or_else(|| panic!("{name}: unexpected line {line:?}"));
        rows.push((left.to_owned(), right.to_owned()));
    }
    panic!("{name} is not in demo.js");
}

/// SDK 的 `SCHEMES`（packages/web-engine/src/index.js）里每个名字都是 WebHost 认得的方案，两种粤语方案都在其中。
#[test]
fn every_sdk_scheme_is_a_webhost_scheme() {
    let sdk = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packages/web-engine/src/index.js"),
    )
    .expect("index.js");
    let list = sdk
        .lines()
        .find_map(|line| line.strip_prefix("export const SCHEMES = Object.freeze(["))
        .and_then(|rest| rest.strip_suffix("]);"))
        .expect("SCHEMES in index.js");
    let names: Vec<&str> = list
        .split(", ")
        .map(|name| name.trim_matches('"'))
        .collect();
    for name in &names {
        assert!(
            Scheme::parse(name).is_some(),
            "{name} is not a WebHost scheme"
        );
    }
    for name in ["jyutping", "cantonese_shuangpin"] {
        assert!(names.contains(&name), "SCHEMES lacks {name}");
    }
}
