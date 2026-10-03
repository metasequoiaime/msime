//! WebHost 的按键路由，跑在 `make_fixture` 写出的最小词库上（不读 `MSIME_EVAL_RESOURCES`，所以期望值不随发布的词库变化）。

#[path = "support/fixture.rs"]
mod fixture;

use std::path::Path;

use msime_engine_wasm::host::{Frame, Key, Out, Row, Scheme, WebHost};
use tempfile::TempDir;

/// 一个会话和它的三个临时目录；目录随它一起删掉。
struct Fixture {
    host: WebHost,
    resources: TempDir,
    _user: TempDir,
    _cache: TempDir,
}

fn open(scheme: Scheme, only_wubi86: bool) -> Fixture {
    let resources = tempfile::tempdir().expect("resource directory");
    fixture::write(&resources.path().join("msime.db"), only_wubi86).expect("fixture db");
    let user = tempfile::tempdir().expect("user directory");
    let cache = tempfile::tempdir().expect("cache directory");
    let host =
        WebHost::new_with_paths(scheme, 9, None, resources.path(), user.path(), cache.path())
            .expect("session on the fixture");
    Fixture {
        host,
        resources,
        _user: user,
        _cache: cache,
    }
}

fn quanpin() -> Fixture {
    open(Scheme::Quanpin, false)
}

/// 把一串字符转成按键：小写字母、大写字母（Shift）、数字、空格，其余可打印 ASCII 是标点。
fn typed(text: &str) -> Vec<Key> {
    text.bytes()
        .map(|byte| match byte {
            b'a'..=b'z' => Key::Letter(byte),
            b'A'..=b'Z' => Key::ShiftLetter(byte),
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
fn space_commits_the_first_seat() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "nihao");
    assert!(frame.composing);
    assert_eq!(frame.preedit, "nihao");
    assert_eq!(frame.page[0].text, "你好");
    assert!(frame.out.is_empty());
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit("你好", 0)]);
    assert!(!frame.composing);
    assert!(frame.page.is_empty());
}

#[test]
fn a_digit_picks_its_seat() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "nihao");
    let second = frame.page[1].text.clone();
    let frame = type_text(&mut fixture.host, "2");
    assert_eq!(frame.out, vec![commit(&second, 1)]);
    // 0 不选字，组字中也不打出来。
    let frame = type_text(&mut fixture.host, "nihao0");
    assert!(frame.out.is_empty());
    assert!(frame.composing);
}

#[test]
fn a_mouse_pick_commits_the_slot_on_the_current_page() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "nihao");
    let second = frame.page[1].text.clone();
    let frame = fixture.host.pick(1);
    assert_eq!(frame.out, vec![commit(&second, 1)]);
    // 空闲时点击什么也不做。
    assert!(fixture.host.pick(0).out.is_empty());
}

#[test]
fn enter_commits_the_letters() {
    let mut fixture = quanpin();
    type_text(&mut fixture.host, "nihao");
    let frame = fixture.host.keys(&[Key::Enter]);
    assert_eq!(frame.out, vec![commit("nihao", -1)]);
    assert!(!frame.composing);
    // 空闲的回车什么也不输出，也不进上文。
    let frame = fixture.host.keys(&[Key::Enter]);
    assert!(frame.out.is_empty());
    assert!(!frame.composing);
    assert_eq!(fixture.host.context_for_tests(), "nihao");
}

#[test]
fn escape_clears_and_then_exits() {
    let mut fixture = quanpin();
    type_text(&mut fixture.host, "nihao");
    let frame = fixture.host.keys(&[Key::Escape]);
    assert!(frame.out.is_empty());
    assert!(!frame.composing);
    assert!(frame.page.is_empty());
    let frame = fixture.host.keys(&[Key::Escape]);
    assert_eq!(frame.out, vec![Out::Exit]);
}

#[test]
fn backspace_edits_the_composition_then_deletes_text() {
    let mut fixture = quanpin();
    type_text(&mut fixture.host, "nihao");
    let frame = fixture.host.keys(&[Key::Backspace { word: false }]);
    assert!(frame.out.is_empty());
    assert!(frame.composing);
    assert_eq!(frame.preedit, "niha");
    let frame = fixture.host.keys(&[Key::Backspace { word: false }; 4]);
    assert!(frame.out.is_empty());
    assert!(!frame.composing);
    let frame = fixture.host.keys(&[Key::Backspace { word: true }]);
    assert_eq!(frame.out, vec![Out::Back { word: true }]);
}

#[test]
fn paging_expands_past_the_initial_candidates() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "a");
    assert_eq!(frame.page_index, 0);
    assert!(!frame.has_prev);
    assert!(frame.has_next);
    let mut frame = frame;
    for _ in 0..3 {
        frame = fixture.host.keys(&[Key::PageNext { punct: None }]);
    }
    // 首次查询只给 24 个（三页，第三页不满），第四页（下标 3）只有展开之后才有。
    assert_eq!(frame.page_index, 3);
    assert!(!frame.page.is_empty());
    assert!(frame.has_prev);
    // 一直翻到底：展开已经在最后一页尝试过，has_next 才变成 false。
    let mut last = frame.page_index;
    loop {
        let next = fixture.host.keys(&[Key::PageNext { punct: None }]);
        if next.page_index == last {
            assert!(!next.has_next);
            break;
        }
        last = next.page_index;
    }
    let frame = fixture.host.keys(&[Key::PagePrev { punct: None }]);
    assert_eq!(frame.page_index, last - 1);
    assert!(frame.has_next);
}

#[test]
fn minus_and_equals_page_while_composing_and_type_when_idle() {
    let mut fixture = quanpin();
    let minus = Key::PagePrev { punct: Some(b'-') };
    let equals = Key::PageNext { punct: Some(b'=') };
    type_text(&mut fixture.host, "a");
    // 组字时 `=`、`-` 和 PageDown、PageUp 一样只翻页，不上屏。
    let frame = fixture.host.keys(&[equals]);
    assert!(frame.out.is_empty());
    assert!(frame.composing);
    assert_eq!(frame.page_index, 1);
    let frame = fixture.host.keys(&[minus]);
    assert!(frame.out.is_empty());
    assert!(frame.composing);
    assert_eq!(frame.page_index, 0);
    fixture.host.keys(&[Key::Escape]);
    // 空闲时它们和同一个标点键的结果完全相同。
    let idle_minus = fixture.host.keys(&[minus]).out;
    assert_eq!(idle_minus, vec![Out::Type("-".to_owned())]);
    let idle_equals = fixture.host.keys(&[equals]).out;
    assert_eq!(idle_equals, type_text(&mut quanpin().host, "=").out);
    assert!(!idle_equals.is_empty());
    assert_eq!(fixture.host.context_for_tests(), "-=");
}

#[test]
fn bare_paging_keys_do_nothing_when_idle() {
    let mut fixture = quanpin();
    let frame = fixture
        .host
        .keys(&[Key::PagePrev { punct: None }, Key::PageNext { punct: None }]);
    assert!(frame.out.is_empty());
    assert!(!frame.composing);
    assert_eq!(fixture.host.context_for_tests(), "");
}

#[test]
fn has_next_holds_until_the_last_page_tried_to_expand() {
    let mut fixture = quanpin();
    let mut frame = type_text(&mut fixture.host, "xi'an");
    // 走到最后一页：只要还没在最后一页尝试过展开，has_next 就一直是 true。
    loop {
        assert!(frame.has_next);
        let next = fixture.host.keys(&[Key::PageNext { punct: None }]);
        if next.page_index == frame.page_index {
            assert!(!next.has_next);
            break;
        }
        frame = next;
    }
    // 新的组字重新给一次机会。
    let frame = type_text(&mut fixture.host, " nihao");
    assert!(frame.has_next);
}

#[test]
fn highlight_moves_within_and_across_pages() {
    let mut fixture = quanpin();
    type_text(&mut fixture.host, "a");
    let frame = fixture.host.keys(&[Key::HighlightNext; 9]);
    assert_eq!(frame.page_index, 1);
    assert_eq!(frame.highlight, 0);
    let frame = fixture.host.keys(&[Key::HighlightPrev]);
    assert_eq!(frame.page_index, 0);
    assert_eq!(frame.highlight, 8);
    let seat_eight = frame.page[8].text.clone();
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit(&seat_eight, 8)]);
}

#[test]
fn a_new_composition_in_the_same_batch_starts_at_the_first_seat() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "ni");
    let first = frame.page[0].text.clone();
    let second = frame.page[1].text.clone();
    let frame = fixture.host.keys(&[Key::HighlightNext]);
    assert_eq!(frame.highlight, 1);
    // 上屏后重打同一个码，和中间状态不同的退格加同一个字母，批末的列表和批首一样，高亮仍要回到首位。
    let frame = type_text(&mut fixture.host, " ni");
    assert_eq!(frame.out, vec![commit(&second, 1)]);
    assert_eq!(frame.highlight, 0);
    fixture.host.keys(&[Key::HighlightNext]);
    let frame = fixture
        .host
        .keys(&[Key::Backspace { word: false }, Key::Letter(b'i')]);
    assert_eq!(frame.highlight, 0);
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit(&first, 0)]);
    // 只动了高亮的批次不换组字，高亮留在原处。
    type_text(&mut fixture.host, "ni");
    fixture.host.keys(&[Key::HighlightNext]);
    let frame = fixture.host.keys(&[Key::HighlightNext, Key::HighlightPrev]);
    assert_eq!(frame.highlight, 1);
}

#[test]
fn an_apostrophe_separates_syllables() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "xi'an");
    assert!(frame.out.is_empty());
    assert_eq!(frame.page[0].text, "西安");
    let frame = fixture.host.keys(&[Key::Space]);
    assert_eq!(frame.out, vec![commit("西安", 0)]);
}

#[test]
fn punctuation_translates_or_passes_through() {
    let mut fixture = quanpin();
    assert_eq!(
        type_text(&mut fixture.host, ",").out,
        vec![commit("，", -1)]
    );
    assert_eq!(
        type_text(&mut fixture.host, "-").out,
        vec![Out::Type("-".to_owned())]
    );
    assert_eq!(
        type_text(&mut fixture.host, "nihao.").out,
        vec![commit("你好", 0), commit("。", -1)]
    );
    // 没有中文形式的标点结束组字时和它一起上屏。
    assert_eq!(
        type_text(&mut fixture.host, "nihao@").out,
        vec![commit("你好", 0), commit("@", -1)]
    );
}

#[test]
fn punctuation_finishes_the_highlighted_seat() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "nihao");
    let second = frame.page[1].text.clone();
    fixture.host.keys(&[Key::HighlightNext]);
    let frame = type_text(&mut fixture.host, ",");
    assert_eq!(frame.out, vec![commit(&second, 1), commit("，", -1)]);
}

#[test]
fn quotes_alternate_and_follow_backspace_and_reset() {
    let mut fixture = quanpin();
    assert_eq!(
        type_text(&mut fixture.host, "\"").out,
        vec![commit("“", -1)]
    );
    assert_eq!(
        type_text(&mut fixture.host, "\"").out,
        vec![commit("”", -1)]
    );
    // 退格删掉开引号后，下一个仍是开引号。
    assert_eq!(
        type_text(&mut fixture.host, "\"").out,
        vec![commit("“", -1)]
    );
    fixture.host.keys(&[Key::Backspace { word: false }]);
    assert_eq!(
        type_text(&mut fixture.host, "\"").out,
        vec![commit("“", -1)]
    );
    // 新回合从开引号开始。
    fixture.host.reset();
    assert_eq!(
        type_text(&mut fixture.host, "\"").out,
        vec![commit("“", -1)]
    );

    fixture.host.reset();
    assert_eq!(type_text(&mut fixture.host, "'").out, vec![commit("‘", -1)]);
    assert_eq!(type_text(&mut fixture.host, "'").out, vec![commit("’", -1)]);
    assert_eq!(type_text(&mut fixture.host, "'").out, vec![commit("‘", -1)]);
    fixture.host.keys(&[Key::Backspace { word: false }]);
    assert_eq!(type_text(&mut fixture.host, "'").out, vec![commit("‘", -1)]);
    fixture.host.reset();
    assert_eq!(type_text(&mut fixture.host, "'").out, vec![commit("‘", -1)]);

    fixture.host.reset();
    assert_eq!(
        type_text(&mut fixture.host, "<<>>").out,
        vec![
            commit("《", -1),
            commit("〈", -1),
            commit("〉", -1),
            commit("》", -1)
        ]
    );
    // 组字中的引号先结束组字。
    assert_eq!(
        type_text(&mut fixture.host, "nihao\"").out,
        vec![commit("你好", 0), commit("“", -1)]
    );
}

#[test]
fn shuangpin_profiles_decode_their_own_finals() {
    for (scheme, keys) in [(Scheme::Xiaohe, "nihc"), (Scheme::Ziranma, "nihk")] {
        let mut fixture = open(scheme, false);
        let frame = type_text(&mut fixture.host, keys);
        assert_eq!(frame.page[0].text, "你好", "{scheme:?}");
        let frame = fixture.host.keys(&[Key::Space]);
        assert_eq!(frame.out, vec![commit("你好", 0)], "{scheme:?}");
    }
}

#[test]
fn wubi_unique_four_code_commits_itself() {
    let mut fixture = open(Scheme::Wubi86, false);
    let frame = type_text(&mut fixture.host, "wqv");
    assert!(frame.composing);
    assert!(frame.out.is_empty());
    assert!(!frame.model_on);
    let frame = type_text(&mut fixture.host, "b");
    assert_eq!(frame.out, vec![commit("你好", -1)]);
    assert!(!frame.composing);
}

#[test]
fn wubi_fifth_letter_top_commits() {
    let mut fixture = open(Scheme::Wubi86, false);
    let frame = type_text(&mut fixture.host, "ggll");
    assert_eq!(texts(&frame.page), vec!["五一", "一五"]);
    let frame = type_text(&mut fixture.host, "w");
    assert_eq!(frame.out, vec![commit("五一", -1)]);
    assert!(frame.composing);
    assert_eq!(frame.preedit, "w");
}

#[test]
fn wubi_empty_code_is_held_until_backspace() {
    let mut fixture = open(Scheme::Wubi86, false);
    let frame = type_text(&mut fixture.host, "xyxy");
    assert!(frame.composing);
    assert!(frame.empty_code);
    assert!(frame.page.is_empty());
    let frame = type_text(&mut fixture.host, "a");
    assert!(frame.out.is_empty());
    assert!(frame.empty_code);
    assert_eq!(frame.preedit, "xyxy");
    let frame = fixture.host.keys(&[Key::Backspace { word: false }]);
    assert!(!frame.empty_code);
    assert_eq!(frame.preedit, "xyx");
    let frame = fixture.host.keys(&[Key::Escape]);
    assert!(!frame.composing);
    // 空闲时的 z 不在五笔码表里，原样打出。
    assert_eq!(
        type_text(&mut fixture.host, "z").out,
        vec![Out::Type("z".to_owned())]
    );
}

#[test]
fn wubi_only_dictionary_answers_wubi_and_leaves_pinyin_empty() {
    let mut wubi = open(Scheme::Wubi86, true);
    let frame = type_text(&mut wubi.host, "gg");
    assert_eq!(texts(&frame.page), vec!["五一", "一五"]);

    let mut pinyin = open(Scheme::Quanpin, true);
    let frame = type_text(&mut pinyin.host, "nihao");
    assert!(frame.composing);
    assert!(texts(&frame.page).iter().all(|text| *text != "你好"));
}

#[test]
fn a_pinyin_session_needs_no_english_dictionary() {
    let mut fixture = quanpin();
    assert!(!fixture.resources.path().join("english.db").exists());
    assert!(!Path::new(&fixture.resources.path().join("others.db")).exists());
    let frame = type_text(&mut fixture.host, "nihao ");
    assert_eq!(frame.out, vec![commit("你好", 0)]);
}

#[test]
fn shift_letters_type_capitals() {
    let mut fixture = quanpin();
    assert_eq!(
        type_text(&mut fixture.host, "A").out,
        vec![Out::Type("A".to_owned())]
    );
    let frame = type_text(&mut fixture.host, "niH");
    assert_eq!(frame.out, vec![commit("ni", -1), Out::Type("H".to_owned())]);
    assert!(!frame.composing);
}

#[test]
fn shift_tap_toggles_english() {
    let mut fixture = quanpin();
    let frame = fixture.host.keys(&[Key::ShiftTap]);
    assert!(frame.english);
    assert_eq!(
        type_text(&mut fixture.host, "a,").out,
        vec![Out::Type("a".to_owned()), Out::Type(",".to_owned())]
    );
    let frame = fixture.host.keys(&[Key::ShiftTap]);
    assert!(!frame.english);
    assert_eq!(
        type_text(&mut fixture.host, ",").out,
        vec![commit("，", -1)]
    );
    // 组字中切换先把字母原样上屏。
    type_text(&mut fixture.host, "ni");
    let frame = fixture.host.keys(&[Key::ShiftTap]);
    assert_eq!(frame.out, vec![commit("ni", -1)]);
    assert!(frame.english);
}

#[test]
fn idle_digits_and_space_pass_through() {
    let mut fixture = quanpin();
    assert_eq!(
        type_text(&mut fixture.host, "1 ").out,
        vec![Out::Type("1".to_owned()), Out::Type(" ".to_owned())]
    );
}

#[test]
fn the_context_holds_only_emitted_text() {
    let mut fixture = quanpin();
    let mut expected = String::new();
    for frame in [
        type_text(&mut fixture.host, "nihao "),
        type_text(&mut fixture.host, "1,"),
        type_text(&mut fixture.host, "xi'an."),
        type_text(&mut fixture.host, "nihao"),
    ] {
        for out in frame.out {
            match out {
                Out::Commit { text, .. } | Out::Type(text) => expected.push_str(&text),
                Out::Back { .. } | Out::Exit => {}
            }
        }
    }
    // 还在组字的 nihao 不在上文里。
    assert_eq!(expected, "你好1，西安。");
    assert_eq!(fixture.host.context_for_tests(), expected);
    fixture
        .host
        .keys(&[Key::Escape, Key::Backspace { word: false }]);
    assert_eq!(fixture.host.context_for_tests(), "你好1，西安");
    fixture.host.keys(&[Key::Backspace { word: true }]);
    assert_eq!(fixture.host.context_for_tests(), "你好1，");
    fixture.host.reset();
    assert_eq!(fixture.host.context_for_tests(), "");
}

#[test]
fn backspace_that_deletes_nothing_on_the_page_keeps_the_context() {
    let mut fixture = quanpin();
    type_text(&mut fixture.host, "\"nihao ");
    fixture.host.set_backspace_deletes(false);
    let frame = fixture.host.keys(&[Key::Backspace { word: false }; 3]);
    assert_eq!(frame.out, vec![Out::Back { word: false }; 3]);
    assert_eq!(fixture.host.context_for_tests(), "“你好");
    // 开引号还在上文里，下一个引号是关引号。
    assert_eq!(
        type_text(&mut fixture.host, "\"").out,
        vec![commit("”", -1)]
    );
    fixture.host.set_backspace_deletes(true);
    fixture.host.keys(&[Key::Backspace { word: false }]);
    assert_eq!(fixture.host.context_for_tests(), "“你好");
}

#[test]
fn a_batch_reports_every_output_in_order() {
    let mut fixture = quanpin();
    let frame = type_text(&mut fixture.host, "nihao nihao2,");
    let second = {
        let mut probe = quanpin();
        type_text(&mut probe.host, "nihao").page[1].text.clone()
    };
    assert_eq!(
        frame.out,
        vec![commit("你好", 0), commit(&second, 1), commit("，", -1)]
    );
    assert!(!frame.composing);
}

#[test]
fn page_size_is_bounded() {
    let resources = tempfile::tempdir().unwrap();
    fixture::write(&resources.path().join("msime.db"), false).unwrap();
    let user = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    for size in [0, 10] {
        assert!(WebHost::new_with_paths(
            Scheme::Quanpin,
            size,
            None,
            resources.path(),
            user.path(),
            cache.path()
        )
        .is_err());
    }
}
