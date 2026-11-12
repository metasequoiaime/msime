use super::*;
use crate::types::NineKeyShuangpinKind;
use rusqlite::Connection;

// 合成词库：够拼出「今天天气怎么样」「你好」，另有几条只为让 77、99、7777、5555 有简拼行。
const MAIN: &str = "CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_j VALUES('jin','j','今',600);CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_t VALUES('tian','t','天',500);CREATE TABLE tbl_1_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_q VALUES('qi','q','气',400);CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',100);CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_m VALUES('mi','m','米',50);CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',100);CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_l VALUES('li','l','里',100);CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_j VALUES('jin''tian','jt','今天',1000);CREATE TABLE tbl_2_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_t VALUES('tian''qi','tq','天气',900);CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);CREATE TABLE tbl_2_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_q VALUES('qi''shi','qs','骑士',300);CREATE TABLE tbl_2_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_y VALUES('yi''yang','yy','一样',300);CREATE TABLE tbl_3_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_3_z VALUES('zen''me''yang','zmy','怎么样',900);CREATE TABLE tbl_4_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_4_j VALUES('jin''tian''tian''qi','jttq','今天天气',800),('jin''jin''ji''jiao','jjjj','斤斤计较',200);CREATE TABLE tbl_4_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_4_q VALUES('qing''qing''shuang''shuang','qqss','清清爽爽',200);";
const ENGLISH: &str = "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);INSERT INTO english_words VALUES('good','good',900);INSERT INTO english_words VALUES('old','old',1000);INSERT INTO english_words VALUES('older','older',800);";

/// 正确的整句和它的几种错按。
const SENTENCE: &str = "5468426842674936639264";
const EXTRA_KEY_INSIDE: &str = "54684268426774936639264";
const EXTRA_KEYS_AT_END: &str = "546842684267493663926477";
const WRONG_FIRST_KEY: &str = "4468426842674936639264";

struct Fixture {
    _directory: tempfile::TempDir,
    paths: RuntimePaths,
}

fn fixture() -> Fixture {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().to_path_buf();
    Connection::open(root.join(assets::MAIN_DICTIONARY))
        .and_then(|db| db.execute_batch(MAIN))
        .expect("main fixture");
    Connection::open(root.join(assets::ENGLISH_DICTIONARY))
        .and_then(|db| db.execute_batch(ENGLISH))
        .expect("english fixture");
    let paths = RuntimePaths {
        resources: root.clone(),
        user_data: root.clone(),
        cache: root.clone(),
        dictionaries: root,
    };
    Fixture {
        _directory: directory,
        paths,
    }
}

/// 出厂的整句设置；`english` 是混入英文。
fn open(paths: &RuntimePaths, english: bool) -> NineKeySession {
    NineKeySession::new(
        paths,
        false,
        FrequencyAdjustmentOptions::default(),
        FuzzyPinyinOptions::default(),
        EnglishInputOptions {
            mixed_candidates: english,
            ..EnglishInputOptions::default()
        },
        true,
        false,
    )
}

fn without_sentences(paths: &RuntimePaths) -> NineKeySession {
    let mut session = open(paths, false);
    session.set_sentence_options(
        SentenceAssociationOptions {
            word_lattice: false,
            ..SentenceAssociationOptions::default()
        },
        false,
    );
    session
}

fn detached() -> NineKeySession {
    NineKeySession::new(
        &RuntimePaths::default(),
        false,
        FrequencyAdjustmentOptions::default(),
        FuzzyPinyinOptions::default(),
        EnglishInputOptions::default(),
        true,
        false,
    )
}

fn type_keys(session: &mut NineKeySession, keys: &str) {
    for key in keys.bytes() {
        assert!(session.character(key).handled, "key {key} unhandled");
    }
}

fn reading(session: &NineKeySession) -> String {
    session.snapshot().nine_key_reading
}

fn row(word: &str, code: &str, canonical: &str, source: CandidateSource) -> WordItem {
    let mut row = WordItem::new(code, word, 1, source, "");
    row.canonical_pinyin = canonical.into();
    row
}

/// 宿主把读音和已按的键逐位对上的规则（Android、iOS 的 sameKey）：去掉 `'` 后一位对一键，字母印在那个键上；14／18 键的组码本身就是字母，同字符也算。全拼九键另外不许有数字。对不上时返回原因。
fn misaligned(reading: &str, codes: &str, grid: KeyGrid) -> Option<String> {
    if reading.is_empty() {
        return Some("读音为空".into());
    }
    if reading.split('\'').any(str::is_empty) {
        return Some("有空的一段".into());
    }
    let letters: Vec<u8> = reading.bytes().filter(|&byte| byte != b'\'').collect();
    if letters.len() != codes.len() {
        return Some(format!("{} 个字符对 {} 个键", letters.len(), codes.len()));
    }
    for (position, (&letter, &code)) in letters.iter().zip(codes.as_bytes()).enumerate() {
        let printed = grid.letters(code).as_bytes().contains(&letter);
        let same = letter == code && !matches!(grid, KeyGrid::NineKey);
        if !printed && !same {
            return Some(format!(
                "第 {position} 位的 {} 不在 {} 键上",
                char::from(letter),
                char::from(code)
            ));
        }
    }
    None
}

#[track_caller]
fn assert_aligned(reading: &str, codes: &str, grid: KeyGrid) {
    if let Some(reason) = misaligned(reading, codes, grid) {
        panic!("{codes} 的读音 {reading:?}：{reason}");
    }
}

#[track_caller]
fn assert_session_aligned(session: &NineKeySession) -> String {
    let reading = reading(session);
    assert_aligned(&reading, &session.digits, session.grid);
    reading
}

/// 读音里每一段结束的键位。
fn boundaries(reading: &str) -> Vec<usize> {
    let mut end = 0;
    reading
        .split('\'')
        .map(|segment| {
            end += segment.len();
            end
        })
        .collect()
}

fn lenient(
    table: &SpellingTable,
    codes: &str,
    splits: &[usize],
    prior: Option<&SyllablePrior<'_>>,
    initial: Option<u8>,
) -> String {
    let mut reading = String::new();
    assert!(
        table.lenient_split(codes, splits, prior, initial, &mut reading),
        "{codes} 没有切出读音"
    );
    reading
}

// ---- 拼写表 ----

/// 长度 1 到 5 的全部 2–9 数字串：容错切分总有结果、逐位落在键上、没有数字；多字母的段都是拼音里有的片段，不在末尾的必须是完整音节；严格切分切得出时两者是同一条。
#[test]
fn every_short_digit_string_reads_as_letters_on_its_keys() {
    let table = KeyGrid::NineKey.spelling_table();
    let shipped = SyllablePrior::shipped(table);
    let flat = SyllablePrior::default();
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
            for (prior, strict_prior) in [(Some(&shipped), &shipped), (None, &flat)] {
                let reading = lenient(table, &codes, &[], prior, None);
                assert_aligned(&reading, &codes, KeyGrid::NineKey);
                let segments: Vec<&str> = reading.split('\'').collect();
                for (position, segment) in segments.iter().enumerate() {
                    if segment.len() == 1 {
                        continue;
                    }
                    let complete = table
                        .by_code
                        .get(&KeyGrid::NineKey.encode(segment))
                        .and_then(|pieces| pieces.iter().find(|(piece, _)| piece == segment))
                        .map(|(_, complete)| *complete);
                    let last = position + 1 == segments.len();
                    assert!(
                        complete == Some(true) || (last && complete == Some(false)),
                        "{codes} 的读音 {reading:?}：{segment} 不是这个位置能有的片段"
                    );
                }
                let strict = table.split_paths_limited(&codes, &[], strict_prior, 1);
                if let Some(path) = strict.first() {
                    assert_eq!(reading, path.join("'"), "{codes}");
                }
            }
        }
    }
}

/// 切不开的那一键单独成段，显示这个键上频度最高的声母，两边能成音节的照常成音节。
#[test]
fn a_stray_key_stands_alone_between_whole_syllables() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    let read = |codes: &str| lenient(table, codes, &[], Some(&prior), None);
    assert_eq!(read("777"), "s's's");
    assert_eq!(read("8426777"), "tian's's's");
    // 末段按前缀的频度排（shan 沾 shang 的光），与严格切分相同。
    assert_eq!(read("877426"), "t's'shan");
    assert_eq!(read("7754"), "s's'ji");
    assert_eq!(read("4468"), "g'hou");
    // 整串能切时不出单键段。
    assert_eq!(read("8426"), "tian");
    assert_eq!(read("84268426"), "tian'tian");
}

#[test]
fn fallback_letters_come_from_the_pressed_key() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    for code in b'2'..=b'9' {
        let letter = table.fallback_letter(code, Some(&prior)).expect("letter");
        assert!(
            KeyGrid::NineKey.letters(code).contains(letter),
            "{letter} on {}",
            char::from(code)
        );
        // i、u、v 不起头任何音节，不当兜底字母。
        assert!(table.starts_syllable(letter.as_bytes()[0]), "{letter}");
    }
    assert_eq!(table.fallback_letter(b'7', Some(&prior)), Some("s"));
    assert_eq!(table.fallback_letter(b'8', Some(&prior)), Some("t"));
    assert_eq!(table.fallback_letter(b'4', Some(&prior)), Some("g"));
    // 没有频度时按键面上的字母顺序。
    assert_eq!(table.fallback_letter(b'7', None), Some("p"));
    assert_eq!(table.fallback_letter(b'1', Some(&prior)), None);
    assert_eq!(table.fallback_letter(b'N', Some(&prior)), None);
}

/// 表外片段的频度查出来是 0.0，比任何真实音节的负分都高：单键段不计分，先比单键段的个数，所以 `426` 是一个音节而不是三个单键段，哪怕这个音节极少用。
#[test]
fn stray_segments_never_outscore_a_real_syllable() {
    let table = SpellingTable::build(
        KeyGrid::NineKey,
        &["hao", "gan", "a", "o", "shao", "tian", "ti", "an"],
    );
    let weights: HashMap<String, i64> = [("hao", 1), ("gan", 900_000), ("a", 5_000_000), ("o", 3)]
        .into_iter()
        .map(|(syllable, weight)| (syllable.to_owned(), weight))
        .collect();
    let prior = SyllablePrior::from_weights(&table.syllables, &weights);
    let read = |codes: &str| lenient(&table, codes, &[], Some(&prior), None);
    assert_eq!(read("426"), "gan");
    assert_eq!(read("42677426"), "gan's'shao");
    assert_eq!(read("4267"), "gan's");
    // 两种切法都不用单键段时，音节少的在前。
    assert_eq!(read("8426"), "tian");
    let uniform = SyllablePrior::default();
    assert_eq!(
        lenient(&table, "426", &[], Some(&uniform), None),
        "gan",
        "频度相同时按字典序"
    );
}

#[test]
fn lenient_split_keeps_user_splits_and_the_chosen_initial() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    // 切分处必须有一段在那里结束，单键段不跨切分。
    let reading = lenient(table, "42677", &[2], Some(&prior), None);
    assert_aligned(&reading, "42677", KeyGrid::NineKey);
    assert!(boundaries(&reading).contains(&2), "{reading}");
    let reading = lenient(table, "426", &[1], Some(&prior), None);
    assert_eq!(boundaries(&reading), [1, 3], "{reading}");
    assert!(reading.starts_with("g'"), "{reading}");
    // 选了首字母：第一段以它开头，切不成音节时第一位就是它。
    assert_eq!(lenient(table, "426", &[], Some(&prior), Some(b'g')), "gao");
    assert_eq!(lenient(table, "77", &[], Some(&prior), Some(b'r')), "r's");
    assert_eq!(lenient(table, "7", &[], Some(&prior), Some(b'p')), "p");
}

#[test]
fn lenient_split_does_not_allocate() {
    let table = KeyGrid::NineKey.spelling_table();
    let prior = SyllablePrior::shipped(table);
    let mut reading = String::with_capacity(128);
    let (found, allocations) = crate::ime::personal_rerank::allocations::count(|| {
        table.lenient_split(
            "84268426774936639264",
            &[],
            Some(&prior),
            None,
            &mut reading,
        )
    });
    assert!(found);
    assert_eq!(allocations, 0);
    assert_aligned(&reading, "84268426774936639264", KeyGrid::NineKey);
}

/// 只有全拼网格有容错切分；空串和超过上限的串不写任何东西。
#[test]
fn lenient_split_leaves_the_reading_alone_when_it_does_not_apply() {
    let table = KeyGrid::NineKey.spelling_table();
    let mut reading = String::from("ni'");
    assert!(!table.lenient_split("", &[], None, None, &mut reading));
    assert!(!table.lenient_split(&"7".repeat(DIGIT_LIMIT + 1), &[], None, None, &mut reading));
    let shuangpin = crate::nine_key_shuangpin::resolve(NineKeyShuangpinKind::Migala3, None)
        .expect("builtin table");
    let grid = KeyGrid::NineKeyShuangpin(shuangpin);
    assert!(!grid
        .spelling_table()
        .lenient_split("77", &[], None, None, &mut reading));
    assert_eq!(reading, "ni'");
    assert!(table.lenient_split(&"7".repeat(DIGIT_LIMIT), &[], None, None, &mut reading));
    assert_aligned(&reading[3..], &"7".repeat(DIGIT_LIMIT), KeyGrid::NineKey);
}

/// 14 键的 UI 键、十八键的 U、V 键不起头任何音节，单独成段时显示键面首字母；精确码的位置只显示它指定的那个字母。
#[test]
fn letter_grids_read_stray_keys_and_exact_letters() {
    let fourteen = KeyGrid::FourteenKey.spelling_table();
    let prior = SyllablePrior::shipped(fourteen);
    let read = |codes: &str| {
        let reading = lenient(fourteen, codes, &[], Some(&prior), None);
        assert_aligned(&reading, codes, KeyGrid::FourteenKey);
        reading
    };
    assert_eq!(read("uu"), "u'u");
    assert_eq!(read("uuu"), "u'u'u");
    // 单键段越少越好：`ou` 是音节，`bugaouu` 只剩最后一个 UI 键单独成段。
    assert_eq!(boundaries(&read("bugaouu")), [2, 4, 6, 7]);
    assert_eq!(read("uI"), "u'i");
    assert_eq!(read("uNuHao"), "u'ni'hao");
    // 精确码排除了同键的另一个字母：`bn` 键上的 B 只能读 b。
    assert!(read("BuHaou").starts_with("b"));

    let eighteen = KeyGrid::EighteenKey.spelling_table();
    let prior = SyllablePrior::shipped(eighteen);
    let reading = lenient(eighteen, "uvu", &[], Some(&prior), None);
    assert_eq!(reading, "u'v'u");
    assert_aligned(&reading, "uvu", KeyGrid::EighteenKey);
}

// ---- 会话 ----

/// 打错一键的长串读音不出数字，一位对一键：错键之前和之后能成音节的照常成音节，错键单独成段。整句开着（出厂设置）和关着都一样，这份夹具下首行只覆盖开头的 今 或 你，后面整段靠尾段切分；第一键按错时没有首行，整串按键拼写。`wen` 而不是 `zen` 是尾段按音节频度切、不按候选切的既有行为。
#[test]
fn a_mistyped_sentence_never_reads_as_digits() {
    let fixture = fixture();
    for (codes, expected) in [
        (SENTENCE, "jin'tian'tian'qi'wen'me'yang"),
        (EXTRA_KEY_INSIDE, "jin'tian'tian's'qi'wen'me'yang"),
        (EXTRA_KEYS_AT_END, "jin'tian'tian'qi'wen'me'yang's's"),
        ("5468426777", "jin'tian's's's"),
        ("546877426", "jin't's'shan"),
        ("6442677", "ni'hao's's"),
        (WRONG_FIRST_KEY, "g'hou'hao'tian'qi'wen'me'yang"),
        ("775468426", "s's'jin'tian"),
    ] {
        for mut session in [
            open(&fixture.paths, false),
            open(&fixture.paths, true),
            without_sentences(&fixture.paths),
        ] {
            type_keys(&mut session, codes);
            assert_eq!(assert_session_aligned(&session), expected, "{codes}");
        }
    }
}

/// 一键一键打：每一步的读音都对得上已按的键，错键之前的读音不因后面切不开而变成数字。
#[test]
fn every_keystroke_of_a_mistyped_sentence_reads_as_letters() {
    let fixture = fixture();
    for codes in [EXTRA_KEY_INSIDE, EXTRA_KEYS_AT_END, WRONG_FIRST_KEY] {
        let mut session = open(&fixture.paths, false);
        for key in codes.bytes() {
            assert!(session.character(key).handled);
            assert_session_aligned(&session);
        }
    }
}

/// 整串切得开（`liu's`），但首行只覆盖 `li`，余下的 `87` 切不成音节。
#[test]
fn an_unsplittable_tail_behind_a_short_leading_row_reads_as_letters() {
    let fixture = fixture();
    let mut session = without_sentences(&fixture.paths);
    type_keys(&mut session, "5487");
    assert_eq!(
        session.candidates.first().map(|row| row.word.as_str()),
        Some("里")
    );
    assert_eq!(reading(&session), "li't's");
}

/// 简拼行的读音走原来的分支，逐字不变。
#[test]
fn initials_rows_read_as_before() {
    let fixture = fixture();
    for (codes, expected) in [
        ("77", "q's"),
        ("99", "y'y"),
        ("7777", "q'q's's"),
        ("5555", "j'j'j'j"),
    ] {
        let mut session = open(&fixture.paths, false);
        type_keys(&mut session, codes);
        assert_eq!(reading(&session), expected, "{codes}");
    }
}

/// 全是同一个键：简拼展开在 `INITIALS_CODE_LIMIT` 之内和之外都有读音。
#[test]
fn runs_of_one_key_read_as_letters_on_both_sides_of_the_initials_limit() {
    let fixture = fixture();
    for (codes, expected) in [
        ("666666", "mo'mo'mo"),
        ("6666666", "mo'mo'non"),
        ("77777", "s's's's's"),
        ("777777", "s's's's's's"),
        ("88888", "tu'tu't"),
        ("888888888888", "tu'tu'tu'tu'tu'tu"),
    ] {
        let mut session = open(&fixture.paths, false);
        type_keys(&mut session, codes);
        assert_eq!(assert_session_aligned(&session), expected, "{codes}");
    }
}

#[test]
fn the_reading_keeps_user_splits_around_a_stray_key() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, "6442'677");
    let reading = assert_session_aligned(&session);
    assert!(boundaries(&reading).contains(&4), "{reading}");
    session.command(Command::Cancel);
    type_keys(&mut session, "77'5468");
    let reading = assert_session_aligned(&session);
    assert!(boundaries(&reading).contains(&2), "{reading}");
}

#[test]
fn the_reading_starts_with_the_locked_spelling() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, "6442677");
    let ni = session
        .spellings
        .iter()
        .position(|spelling| spelling == "ni")
        .expect("ni in the spelling column");
    assert!(session.choose_spelling(ni).handled);
    assert_eq!(session.locked, ["ni"]);
    let reading = assert_session_aligned(&session);
    assert!(reading.starts_with("ni'"), "{reading}");

    // 首行给不出读音时也从锁定的拼写接起。
    let mut bare = detached();
    bare.digits = "6477".into();
    bare.locked = vec!["ni".into()];
    assert_eq!(bare.reading_for(None), "ni'p'p");
    // 数字全部锁定后没有要兜底的键。
    bare.digits = "64".into();
    assert_eq!(bare.reading_for(None), "");
}

#[test]
fn the_reading_starts_with_the_chosen_initial() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    type_keys(&mut session, "6442677");
    let letter = session
        .spellings
        .iter()
        .position(|spelling| spelling == "M")
        .expect("M in the spelling column");
    assert!(session.choose_spelling(letter).handled);
    let reading = assert_session_aligned(&session);
    assert!(reading.starts_with('m'), "{reading}");

    let mut bare = detached();
    bare.digits = "77".into();
    bare.initial = Some(Initial {
        letter: b'r',
        typed: 2,
        splits: 0,
    });
    assert_eq!(bare.reading_for(None), "r'p");
}

/// 打到上限的串带着错键：读音仍是 48 个字母。
#[test]
fn a_mistyped_run_at_the_digit_limit_reads_as_letters() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, false);
    let codes = format!("{}77{}", "5468426".repeat(3), "5468426".repeat(4));
    let codes = &codes[..DIGIT_LIMIT];
    type_keys(&mut session, codes);
    assert_eq!(session.digits.len(), DIGIT_LIMIT);
    assert_session_aligned(&session);
}

/// 九键首行是英文词时读音是这个词覆盖已按键数的小写字母，与 14 键一致；词对不上已按的键时退回按键拼写。
#[test]
fn an_english_leading_row_reads_as_its_letters_on_nine_key() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, true);
    type_keys(&mut session, "4663");
    let front = session.candidates.first().expect("leading row");
    assert_eq!(front.word.as_str(), "good");
    assert_eq!(front.source, CandidateSource::EnglishDictionary);
    assert_eq!(reading(&session), "good");

    let mut bare = detached();
    bare.digits = "6533".into();
    let older = row("older", "older", "", CandidateSource::EnglishDictionary);
    assert_eq!(bare.reading_for(Some(&older)), "olde");
    bare.digits = "77".into();
    let capitals = row("QQ", "qq", "", CandidateSource::EnglishDictionary);
    assert_eq!(bare.reading_for(Some(&capitals)), "qq");
    // 词比已按的键短，或者有一位不在键上：按键拼写。
    let unrelated = row("ok", "ok", "", CandidateSource::EnglishDictionary);
    assert_eq!(bare.reading_for(Some(&unrelated)), "p'p");
    bare.digits = "777".into();
    assert_eq!(bare.reading_for(Some(&capitals)), "p'p'p");
}

/// 没有首行、首行不是从拼音读出来的：读音是已按的键切出的拼写。
#[test]
fn a_missing_or_unreadable_leading_row_reads_as_the_typed_keys() {
    let mut bare = detached();
    bare.digits = "6442677".into();
    assert_eq!(bare.reading_for(None), "mi'gan'p'p");
    let emoji = row("😀", "6442677", "", CandidateSource::Emoji);
    assert_eq!(bare.reading_for(Some(&emoji)), "mi'gan'p'p");
    // 带着用户的切分。
    bare.splits = vec![1];
    let reading = bare.reading_for(None);
    assert_aligned(&reading, "6442677", KeyGrid::NineKey);
    assert_eq!(boundaries(&reading)[0], 1, "{reading}");
}

/// 14 键首行覆盖不到的尾段也走容错切分；14 键没有首行时读音照旧为空（它的组码本来就是字母）。
#[test]
fn fourteen_key_reads_an_unsplittable_tail_and_keeps_its_empty_reading() {
    let mut session = detached();
    session.grid = KeyGrid::FourteenKey;
    session.digits = "bugaouu".into();
    let front = row("你好", "bugao", "ni'hao", CandidateSource::Database);
    assert_eq!(session.reading_for(Some(&front)), "ni'hao'u'u");
    assert_eq!(session.reading_for(None), "");
    let emoji = row("😀", "bugaouu", "", CandidateSource::Emoji);
    assert_eq!(session.reading_for(Some(&emoji)), "");
}

// ---- 不在范围内的网格和模式：钉住现状 ----

/// 九键双拼一个音节两位，不做单键兜底：切不成对的尾段照旧原样显示键码。
#[test]
fn nine_key_shuangpin_readings_are_unchanged() {
    let fixture = fixture();
    let table = crate::nine_key_shuangpin::resolve(NineKeyShuangpinKind::Migala3, None)
        .expect("builtin table");
    let mut session = open(&fixture.paths, false);
    session.set_grid(KeyGrid::NineKeyShuangpin(table));
    // 米旮旯 3：ni 是 64，hao 是 47。
    for (codes, expected) in [
        ("6447", "ni'hao"),
        ("64477", "ni'hao's"),
        ("644711", "ni'4711"),
        ("6411", "ni'11"),
    ] {
        type_keys(&mut session, codes);
        assert_eq!(reading(&session), expected, "{codes}");
        session.command(Command::Cancel);
    }

    let mut bare = detached();
    bare.grid = KeyGrid::NineKeyShuangpin(table);
    bare.digits = "644711".into();
    let front = row("你好", "6447", "ni'hao", CandidateSource::Database);
    assert_eq!(bare.reading_for(Some(&front)), "ni'hao'11");
    assert_eq!(bare.reading_for(None), "");
    let mut untouched = String::from("ni'hao'");
    assert!(!bare.write_lenient_tail(4, &mut untouched));
    assert_eq!(untouched, "ni'hao'");
}

#[test]
fn english_only_mode_has_no_reading() {
    let fixture = fixture();
    let mut session = open(&fixture.paths, true);
    session.set_english_only(true);
    type_keys(&mut session, "4663");
    assert_eq!(
        session.candidates.first().map(|row| row.word.as_str()),
        Some("good")
    );
    assert_eq!(reading(&session), "");
}
