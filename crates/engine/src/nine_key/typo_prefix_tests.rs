use super::lenient_reading_tests::{
    assert_session_aligned, fixture, open, type_keys, without_sentences, EXTRA_KEYS_AT_END,
    EXTRA_KEY_INSIDE, SENTENCE, WRONG_FIRST_KEY,
};
use super::typo_prefix::append_prefix_paths;
use super::*;
use crate::types::NineKeyShuangpinKind;

/// 首行的词、它覆盖的键数和全拼。
fn front(session: &NineKeySession) -> Option<(String, usize, String)> {
    session.candidates.first().map(|row| {
        (
            row.word.to_string(),
            row.pinyin.len(),
            row.canonical_pinyin.to_string(),
        )
    })
}

fn words(session: &NineKeySession) -> Vec<String> {
    session
        .candidates
        .iter()
        .map(|row| row.word.to_string())
        .collect()
}

/// 未锁定的数字整串切不切得成音节（最后一段可以没打完）。
fn splits_whole(codes: &str) -> bool {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    !table.split_paths_limited(codes, &[], &prior, 1).is_empty()
}

// ---- 拼写表 ----

#[test]
fn the_complete_prefix_stops_before_the_stray_key() {
    let table = KeyGrid::NineKey.spelling_table();
    let prefix = |codes: &str| table.complete_prefix_len(codes, &[]);
    assert_eq!(prefix("5468426777"), 7);
    assert_eq!(prefix("6442677"), 5);
    assert_eq!(prefix(EXTRA_KEY_INSIDE), 11);
    assert_eq!(prefix(EXTRA_KEYS_AT_END), 22);
    // 第一键就切不出完整音节。
    assert_eq!(prefix(WRONG_FIRST_KEY), 0);
    assert_eq!(prefix("775468426"), 0);
    assert_eq!(prefix("77"), 0);
    // 整串都是完整音节时就是全长；最后一段没打完时停在它前面。
    assert_eq!(prefix(SENTENCE), SENTENCE.len());
    assert_eq!(prefix("5467"), 3);
    // 最长的那条不一定是用户想打的那条：`5468` 按 `li'mu` 切得到第 4 位。
    assert_eq!(prefix("5468"), 4);
    assert_eq!(prefix(""), 0);
    assert_eq!(prefix(&"7".repeat(DIGIT_LIMIT + 1)), 0);
}

/// 长度 1 到 5 的全部 2–9 数字串：前缀全长时严格切分一定有结果；严格切分没有结果时，后面再按任何一键前缀都不变（拼写表收了每个音节的每个前缀，到得了更远就说明原来的串切得开）。
#[test]
fn the_complete_prefix_freezes_once_the_run_cannot_be_split() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::default();
    let mut stuck = 0;
    for length in 1..=5u32 {
        for index in 0..8usize.pow(length) {
            let mut rest = index;
            let codes: String = (0..length)
                .map(|_| {
                    let code = char::from(b'2' + (rest % 8) as u8);
                    rest /= 8;
                    code
                })
                .collect();
            let prefix = table.complete_prefix_len(&codes, &[]);
            let strict = !table.split_paths_limited(&codes, &[], &prior, 1).is_empty();
            assert!(prefix <= codes.len());
            if prefix == codes.len() {
                assert!(strict, "{codes}");
            }
            if strict {
                continue;
            }
            stuck += 1;
            assert!(prefix < codes.len(), "{codes}");
            for next in b'2'..=b'9' {
                let longer = format!("{codes}{}", char::from(next));
                assert_eq!(
                    table.complete_prefix_len(&longer, &[]),
                    prefix,
                    "{codes} 之后按 {}",
                    char::from(next)
                );
                assert!(
                    table
                        .split_paths_limited(&longer, &[], &prior, 1)
                        .is_empty(),
                    "{longer}"
                );
            }
        }
    }
    assert!(stuck > 0);
}

#[test]
fn the_complete_prefix_keeps_user_splits() {
    let table = KeyGrid::NineKey.spelling_table();
    // `426` 跨过第 4 位的切分，hao 不成立；切分之前的 `ni'ha` 仍是完整音节。
    let crossing = table.complete_prefix_len("6442677", &[4]);
    assert!((4..=5).contains(&crossing), "{crossing}");
    let mut paths = Vec::new();
    let prior = SyllablePrior::shipped(table);
    let prefix = append_prefix_paths(table, "6442677", &[4], &prior, &mut paths);
    assert_eq!(prefix, crossing);
    assert!(!paths.is_empty());
    for path in &paths {
        let mut end = 0;
        let ends: Vec<usize> = path
            .iter()
            .map(|piece| {
                end += piece.len();
                end
            })
            .collect();
        assert_eq!(ends.last(), Some(&prefix), "{path:?}");
        assert!(ends.contains(&4), "{path:?}");
    }
    // 切分正好落在前缀末尾。
    assert_eq!(table.complete_prefix_len("6442677", &[5]), 5);
    // 切分把开头隔成切不成完整音节的一段。
    assert_eq!(table.complete_prefix_len("5468426", &[1]), 0);
}

#[test]
fn prefix_paths_spell_the_prefix_in_whole_syllables() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    let mut paths = Vec::new();
    assert_eq!(
        append_prefix_paths(table, "5468426777", &[], &prior, &mut paths),
        7
    );
    assert!(!paths.is_empty() && paths.len() <= QUERY_PATH_LIMIT);
    assert!(paths.iter().any(|path| path == &["jin", "tian"]));
    for path in &paths {
        assert_eq!(path.iter().map(String::len).sum::<usize>(), 7, "{path:?}");
        for piece in path {
            assert!(
                table
                    .syllables
                    .iter()
                    .any(|(syllable, _)| syllable == piece),
                "{piece} 不是完整音节"
            );
        }
    }
    // 与用户刚好打到那一位、并在那里打了切分时的严格切分是同一批。
    assert_eq!(
        paths,
        table.split_paths_limited("5468426", &[7], &prior, QUERY_PATH_LIMIT)
    );

    // 一个音节也切不出：原样返回。
    let mut untouched = vec![vec!["ni".to_owned()]];
    assert_eq!(
        append_prefix_paths(table, "775468426", &[], &prior, &mut untouched),
        0
    );
    assert_eq!(untouched, [["ni"]]);
}

#[test]
fn the_complete_prefix_does_not_allocate() {
    let table = KeyGrid::NineKey.spelling_table();
    let (prefix, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        table.complete_prefix_len(EXTRA_KEY_INSIDE, &[])
    });
    assert_eq!(prefix, 11);
    assert_eq!(allocations, 0);
}

/// 14 键同样按完整音节找前缀，精确码的位置只认它指定的字母；三种双拼网格不纳入。
#[test]
fn letter_grids_find_the_prefix_and_shuangpin_grids_do_not() {
    let fourteen = KeyGrid::FourteenKey.spelling_table();
    // BN UI GH AS OP 之后两个 UI 键：`bu'ga'ou` 到第 6 位，UI 键不起头任何音节。
    assert_eq!(fourteen.complete_prefix_len("bugaouu", &[]), 6);
    assert_eq!(fourteen.complete_prefix_len("uu", &[]), 0);
    // 精确的 N 排除了同键的 b：`niu` 成立，再往后的 UI 键接不上。
    assert_eq!(fourteen.complete_prefix_len("Nuuu", &[]), 3);
    assert_eq!(fourteen.complete_prefix_len("Buuu", &[]), 2);

    let shuangpin = crate::nine_key_shuangpin::resolve(NineKeyShuangpinKind::Migala3, None)
        .expect("builtin table");
    let grid = KeyGrid::NineKeyShuangpin(shuangpin);
    assert_eq!(grid.spelling_table().complete_prefix_len("644711", &[]), 0);
    let mut paths = Vec::new();
    let prior = SyllablePrior::default();
    assert_eq!(
        append_prefix_paths(grid.spelling_table(), "644711", &[], &prior, &mut paths),
        0
    );
    assert!(paths.is_empty());
}

// ---- 会话 ----

/// 整串切不开时，首行是错键之前的词，与用户刚好打到最长可切前缀那一位时的首行相同，不再是开头的单字。`546877426` 的前缀是按 `li'mu` 切出来的 4 键，用户打到那一位时的首行「今天」是 `jin't` 的补全、对不上后面的键，所以首行仍是「今」。
#[test]
fn candidates_fall_back_to_the_prefix_before_the_stray_key() {
    let fixture = fixture();
    let table = KeyGrid::NineKey.spelling_table();
    // 每条：数字串、整句开着时的首行、整句关着时的首行、是否与打到前缀那一位时相同。
    for (codes, sentence, word, as_typed_to_the_prefix) in [
        ("6442677", "你好", "你好", true),
        ("5468426777", "今天", "今天", true),
        ("546877426", "今", "今", false),
        (EXTRA_KEY_INSIDE, "今天天", "今天", true),
        (EXTRA_KEYS_AT_END, "今天天气", "今天天气", true),
    ] {
        assert!(!splits_whole(codes), "{codes}");
        let prefix = table.complete_prefix_len(codes, &[]);
        for (name, expected, mut session, mut reference) in [
            (
                "整句开",
                sentence,
                open(&fixture.paths, false),
                open(&fixture.paths, false),
            ),
            (
                "混入英文",
                sentence,
                open(&fixture.paths, true),
                open(&fixture.paths, true),
            ),
            (
                "整句关",
                word,
                without_sentences(&fixture.paths),
                without_sentences(&fixture.paths),
            ),
        ] {
            type_keys(&mut session, codes);
            type_keys(&mut reference, &codes[..prefix]);
            let leading = front(&session).unwrap_or_else(|| panic!("{codes}（{name}）没有候选"));
            assert_eq!(leading.0, expected, "{codes}（{name}）");
            if as_typed_to_the_prefix {
                assert_eq!(
                    Some(&leading),
                    front(&reference).as_ref(),
                    "{codes}（{name}）"
                );
            }
            assert!(leading.1 <= prefix, "{codes}（{name}）");
            // 候选只覆盖前缀，没有一行吃到错键。
            assert!(
                session
                    .candidates
                    .iter()
                    .all(|row| row.pinyin.len() <= prefix),
                "{codes}（{name}）：{:?}",
                words(&session)
            );
            assert_session_aligned(&session);
        }
    }
}

/// 首行覆盖到的那一段读它自己的拼音，错键和后面的键接着按容错切分读。
#[test]
fn the_reading_follows_the_prefix_row() {
    let fixture = fixture();
    for (codes, expected) in [
        ("6442677", "ni'hao's's"),
        ("5468426777", "jin'tian's's's"),
        (EXTRA_KEY_INSIDE, "jin'tian'tian's'qi'wen'me'yang"),
    ] {
        let mut session = open(&fixture.paths, false);
        type_keys(&mut session, codes);
        assert_eq!(assert_session_aligned(&session), expected, "{codes}");
    }
}

/// 一键一键打：整串还切得开时不走前缀；切不开之后首行和它覆盖的键数不再变，每一步的读音都对得上已按的键。
#[test]
fn the_leading_row_freezes_after_the_stray_key() {
    let fixture = fixture();
    for codes in [EXTRA_KEY_INSIDE, EXTRA_KEYS_AT_END, "6442677"] {
        let mut session = open(&fixture.paths, false);
        let mut frozen: Option<(String, usize, String)> = None;
        for (index, key) in codes.bytes().enumerate() {
            assert!(session.character(key).handled);
            assert_session_aligned(&session);
            if splits_whole(&codes[..=index]) {
                assert!(frozen.is_none(), "{codes} 第 {index} 键又切得开了");
                continue;
            }
            let leading = front(&session).expect("leading row");
            assert!(leading.1 > 3 || codes == "6442677", "{codes}：{leading:?}");
            match &frozen {
                Some(first) => assert_eq!(&leading, first, "{codes} 第 {index} 键"),
                None => frozen = Some(leading),
            }
        }
        assert!(frozen.is_some(), "{codes}");
    }
}

/// 第一键就切不出完整音节时没有前缀可退，候选与原来相同：这份夹具下一行也没有。
#[test]
fn a_run_without_a_complete_first_syllable_keeps_its_candidates() {
    let fixture = fixture();
    for codes in [WRONG_FIRST_KEY, "775468426"] {
        for mut session in [
            open(&fixture.paths, false),
            without_sentences(&fixture.paths),
        ] {
            type_keys(&mut session, codes);
            assert_eq!(words(&session), Vec::<String>::new(), "{codes}");
            assert!(session.path_buffer.is_empty(), "{codes}");
            assert_session_aligned(&session);
        }
    }
}

/// 整串切得开的输入不走前缀：切分路径都拼满全部数字或只是开头一个音节的兜底。
#[test]
fn a_run_that_splits_is_left_alone() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, SENTENCE);
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    let strict = table.split_paths_limited(SENTENCE, &[], &prior, QUERY_PATH_LIMIT);
    assert_eq!(session.path_buffer[..strict.len()], strict[..]);
    assert_eq!(
        front(&session).map(|row| row.0).as_deref(),
        Some("今天天气")
    );
}

/// 锁定了拼写之后再打错：前缀从第一个未锁定的数字算起，候选仍然守住锁定的拼写。
#[test]
fn prefix_candidates_keep_the_locked_spelling() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, "546842684267477");
    let jin = session
        .spellings
        .iter()
        .position(|spelling| spelling == "jin")
        .expect("jin in the spelling column");
    assert!(session.choose_spelling(jin).handled);
    assert_eq!(session.locked, ["jin"]);
    let leading = front(&session).expect("leading row");
    assert_eq!(leading.0, "今天天气");
    assert_eq!(leading.1, 13);
    for row in &session.candidates {
        if !row.canonical_pinyin.is_empty() {
            assert!(
                row.canonical_pinyin.starts_with("jin"),
                "{} 读 {}",
                row.word,
                row.canonical_pinyin
            );
        }
    }
    let reading = assert_session_aligned(&session);
    assert!(reading.starts_with("jin'tian'tian'qi'"), "{reading}");
}

/// 用户打了切分：前缀路径不跨切分。`6442'677` 的前缀是 `ni'ha'o` 的 5 键，候选与打到 `6442'6` 时一样（那时「你好」就已经是 `ni'ha` 这一组查出来的首行，不是这里新加的）。
#[test]
fn prefix_candidates_keep_user_splits() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, "6442'677");
    assert_eq!(session.splits, [4]);
    let mut reference = open(&fixture.paths, false);
    type_keys(&mut reference, "6442'6");
    assert!(front(&session).is_some());
    assert_eq!(front(&session), front(&reference));
    assert_eq!(words(&session), words(&reference));
    let mut longest = 0;
    for path in &session.path_buffer {
        let mut end = 0;
        let ends: Vec<usize> = path
            .iter()
            .map(|piece| {
                end += piece.len();
                end
            })
            .collect();
        assert!(end <= 4 || ends.contains(&4), "{path:?}");
        longest = longest.max(end);
    }
    assert_eq!(longest, 5, "{:?}", session.path_buffer);
    assert_session_aligned(&session);

    // 切分在错键之前、落在音节边界上：照常退到前缀。
    session.command(Command::Cancel);
    type_keys(&mut session, "64'42677");
    assert_eq!(front(&session).map(|row| row.0).as_deref(), Some("你好"));
}

/// 选了首字母：前缀路径同样只留以它开头的。
#[test]
fn prefix_candidates_keep_the_chosen_initial() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, "6442677");
    let letter = session
        .spellings
        .iter()
        .position(|spelling| spelling == "M")
        .expect("M in the spelling column");
    assert!(session.choose_spelling(letter).handled);
    assert!(!session.candidates.is_empty());
    assert!(session.path_buffer.iter().any(|path| path.len() > 1));
    for path in &session.path_buffer {
        assert!(path[0].starts_with('m'), "{path:?}");
    }
    for row in &session.candidates {
        if !row.canonical_pinyin.is_empty() {
            assert!(
                row.canonical_pinyin.starts_with('m'),
                "{} 读 {}",
                row.word,
                row.canonical_pinyin
            );
        }
    }
    let reading = assert_session_aligned(&session);
    assert!(reading.starts_with('m'), "{reading}");
}

/// 打到上限的串带着错键：再按一键时先选掉的首行是错键之前的词，不是开头的单字。
#[test]
fn the_limit_pick_takes_the_prefix_row() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    let codes = format!("546842684267477{}", "5468426".repeat(5));
    let codes = &codes[..DIGIT_LIMIT];
    type_keys(&mut session, codes);
    assert_eq!(session.digits.len(), DIGIT_LIMIT);
    assert_eq!(
        KeyGrid::NineKey
            .spelling_table()
            .complete_prefix_len(codes, &[]),
        13
    );
    let leading = front(&session).expect("leading row");
    assert_eq!((leading.0.as_str(), leading.1), ("今天天气", 13));
    assert!(session.limit_pick_ready(b'5'));
    assert!(session.select(0).handled);
    assert_eq!(session.digits, codes[13..]);
    assert_session_aligned(&session);
}

/// 14 键走同一条路：`bu'ga'ou` 之后的 UI 键切不开，候选退到前 6 键。
#[test]
fn fourteen_key_candidates_fall_back_to_the_prefix() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    session.set_grid(KeyGrid::FourteenKey);
    type_keys(&mut session, "bugaouu");
    assert!(session
        .path_buffer
        .iter()
        .any(|path| path.iter().map(String::len).sum::<usize>() == 6));
    assert!(!session.candidates.is_empty());
    assert!(session.candidates.iter().all(|row| row.pinyin.len() <= 6));
}

/// 九键双拼不纳入：切不成对时候选照旧只有开头那个音节的字。
#[test]
fn nine_key_shuangpin_candidates_are_unchanged() {
    let fixture = fixture();
    let table = crate::nine_key_shuangpin::resolve(NineKeyShuangpinKind::Migala3, None)
        .expect("builtin table");
    let mut session = open(&fixture.paths, false);
    session.set_grid(KeyGrid::NineKeyShuangpin(table));
    type_keys(&mut session, "644711");
    assert!(session.path_buffer.iter().all(|path| path.len() == 1));
    assert_eq!(front(&session).map(|row| row.0).as_deref(), Some("你"));
}
