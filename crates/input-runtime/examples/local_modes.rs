//! Type each shortcut mode the way the reference's guide documents it, against the real dictionary.
//!
//! The eight modes have had a preference apiece, a menu entry apiece and a host test apiece for a
//! while. None of that says what happens when someone actually types `Shift+T` and then `rq`: the
//! host tests use empty resource files, because the gate they exercise only asks whether the file is
//! there, and the shared tests use a synthetic engine. So the one thing the guide is specific about
//! - what each mode puts on screen - was the part nothing checked.
//!
//! This types the guide's own examples and reads the candidates back. It needs the verified
//! dictionary release, so it is an example rather than a CTest, like `runtime_dictionary` beside it.
//!
//! usage: local_modes <verified-dictionary-directory>

use msime_engine::host::{prepare_options, Session};
use msime_input_runtime::{Action, Runtime};

fn shift(runtime: &mut Runtime<Session>, value: u8) -> Result<(), Box<dyn std::error::Error>> {
    runtime.dispatch(Action::Character { value, shift: true })?;
    Ok(())
}

fn type_ascii(
    runtime: &mut Runtime<Session>,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for byte in text.bytes() {
        runtime.dispatch(Action::Character {
            value: byte,
            shift: false,
        })?;
    }
    Ok(())
}

fn candidates(runtime: &Runtime<Session>) -> Vec<String> {
    runtime
        .view()
        .candidates
        .iter()
        .map(|candidate| candidate.text.clone())
        .collect()
}

/// Enter a mode, type its example, and hand back what the panel offers.
fn offer(
    options: &msime_engine::host::EngineOptions,
    mode: u8,
    input: &str,
) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let mut runtime = Runtime::new(Session::new(options)?, 9)?;
    runtime.focus(true)?;
    shift(&mut runtime, mode)?;
    let entered = runtime.view().local_mode.clone();
    type_ascii(&mut runtime, input)?;
    Ok((entered, candidates(&runtime)))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resources = std::env::args_os()
        .nth(1)
        .ok_or("usage: local_modes <verified-dictionary-directory>")?;
    let resources = std::fs::canonicalize(resources)?;
    let temporary = tempfile::tempdir()?;
    let options = prepare_options(
        resources.to_str().ok_or("non-UTF-8 resource path")?,
        temporary.path().join("user").to_str().unwrap(),
        temporary.path().join("cache").to_str().unwrap(),
        "local-mode-guide-probe",
    )?;

    // 日期与时间（T）: rq / riqi / date for the date, sj / shijian / time for the clock, xq /
    // xingqi / week for the weekday. The guide lists three spellings each, and the point of the
    // list is that they are interchangeable, so all three are typed rather than one.
    for spelling in ["rq", "riqi", "date"] {
        let (mode, offered) = offer(&options, b'T', spelling)?;
        assert_eq!(mode, "date_time", "Shift+T did not enter the date mode");
        assert!(
            offered
                .iter()
                .any(|text| text.contains('年') && text.contains('月')),
            "date mode offered {offered:?} for {spelling}"
        );
    }
    for spelling in ["sj", "shijian", "time"] {
        let (_, offered) = offer(&options, b'T', spelling)?;
        assert!(
            offered
                .iter()
                .any(|text| text.contains(':') || text.contains('时')),
            "time mode offered {offered:?} for {spelling}"
        );
    }
    for spelling in ["xq", "xingqi", "week"] {
        let (_, offered) = offer(&options, b'T', spelling)?;
        assert!(
            offered
                .iter()
                .any(|text| text.contains("星期") || text.contains("周")),
            "weekday mode offered {offered:?} for {spelling}"
        );
    }

    // Unicode（U）: a bare code point and one written with the `+` the guide shows.
    for (input, expected) in [("4e00", "一"), ("+1f600", "😀")] {
        let mut runtime = Runtime::new(Session::new(&options)?, 9)?;
        runtime.focus(true)?;
        shift(&mut runtime, b'U')?;
        assert_eq!(runtime.view().local_mode, "unicode");
        type_ascii(&mut runtime, input)?;
        let committed = runtime.dispatch(Action::SelectHighlighted)?;
        assert_eq!(
            committed.commit.as_deref(),
            Some(expected),
            "Unicode mode committed {:?} for {input}",
            committed.commit
        );
    }

    // Emoji（E）and kaomoji（M）: the guide's own examples, by full spelling and by initials.
    for input in ["xiaolian", "xl", "laugh"] {
        let (mode, offered) = offer(&options, b'E', input)?;
        assert_eq!(mode, "emoji", "Shift+E did not enter the emoji mode");
        assert!(
            !offered.is_empty(),
            "emoji mode offered nothing for {input}"
        );
    }
    for input in ["haixiu", "hx", "kiss"] {
        let (mode, offered) = offer(&options, b'M', input)?;
        assert_eq!(mode, "kaomoji", "Shift+M did not enter the kaomoji mode");
        assert!(
            !offered.is_empty(),
            "kaomoji mode offered nothing for {input}"
        );
    }

    // 超级简拼（J）: every letter is an initial, so `nh` reaches 你好.
    let (mode, offered) = offer(&options, b'J', "nh")?;
    assert_eq!(
        mode, "super_jianpin",
        "Shift+J did not enter the jianpin mode"
    );
    assert!(
        offered.iter().any(|text| text == "你好"),
        "super jianpin offered {offered:?} for nh"
    );

    // 临时英文（Y）: letters are English, the space bar commits what was typed, and the mode ends
    // with the commit - the guide's "上屏后回到中文".
    let mut runtime = Runtime::new(Session::new(&options)?, 9)?;
    runtime.focus(true)?;
    shift(&mut runtime, b'Y')?;
    assert_eq!(runtime.view().local_mode, "temporary_english");
    type_ascii(&mut runtime, "hello")?;
    let committed = runtime.dispatch(Action::SelectHighlighted)?;
    assert_eq!(
        committed.commit.as_deref(),
        Some("hello"),
        "temporary English committed {:?}",
        committed.commit
    );
    assert_ne!(
        runtime.view().local_mode,
        "temporary_english",
        "temporary English stayed on after committing"
    );

    // 临时日语（R）: romaji, and the same return to Chinese afterwards.
    let mut runtime = Runtime::new(Session::new(&options)?, 9)?;
    runtime.focus(true)?;
    shift(&mut runtime, b'R')?;
    assert_eq!(runtime.view().local_mode, "temporary_japanese");
    type_ascii(&mut runtime, "nihon")?;
    let offered = candidates(&runtime);
    assert!(
        offered.iter().any(|text| text.chars().any(|character| {
            ('\u{3040}'..='\u{30ff}').contains(&character)
                || ('\u{4e00}'..='\u{9fff}').contains(&character)
        })),
        "temporary Japanese offered {offered:?} for nihon"
    );
    let committed = runtime.dispatch(Action::SelectHighlighted)?;
    assert!(committed.commit.is_some());
    assert_ne!(
        runtime.view().local_mode,
        "temporary_japanese",
        "temporary Japanese stayed on after committing"
    );

    // 快捷短语（K）: there is nothing to recall until the user has stored one, so this checks the
    // half that does not need a user dictionary - the mode opens and takes letters rather than
    // leaving them to be typed as a capital K and a word.
    let mut runtime = Runtime::new(Session::new(&options)?, 9)?;
    runtime.focus(true)?;
    shift(&mut runtime, b'K')?;
    assert_eq!(runtime.view().local_mode, "quick_phrase");
    let taken = runtime.dispatch(Action::Character {
        value: b'a',
        shift: false,
    })?;
    assert!(
        taken.handled,
        "quick phrase mode did not take its code letter"
    );

    // 符号模式（S）: 分组码 `xl` 与符号名 `alpha`。分组码表与符号名表都编在引擎里，不读词库。
    for (input, expected) in [("xl", "α"), ("alpha", "α"), ("jt", "→")] {
        let (mode, offered) = offer(&options, b'S', input)?;
        assert_eq!(mode, "symbol", "Shift+S did not enter the symbol mode");
        assert!(
            offered.iter().any(|text| text == expected),
            "symbol mode offered {offered:?} for {input}"
        );
    }

    println!("local modes: date/time, Unicode, emoji, kaomoji, symbols, super jianpin, temporary English and Japanese, quick phrase all answer as the guide describes");
    Ok(())
}
