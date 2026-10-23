use super::*;
use rusqlite::Connection;

const LETTERS: &[u8] = b"abcdefghiqwertyABCDQW";
const DISPLAYS: [&str; 6] = ["", "'", "!", "中", "S", "é"];
const WEIGHTS: [i64; 9] = [-3, 0, 0, 1, 5, 5, 50, 50, 900];

/// 小字母表上的密集词库：同权、零权、负权、大写字母、带标点的词、空展示和多字节展示都有，按键码排序的每个分支都会碰到。
fn dense_dictionary(directory: &std::path::Path) {
    let connection = Connection::open(directory.join(assets::ENGLISH_DICTIONARY)).unwrap();
    connection
        .execute_batch("CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);")
        .unwrap();
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state % bound as u64) as usize
    };
    let mut insert = connection
        .prepare("INSERT INTO english_words VALUES(?1,?2,?3)")
        .unwrap();
    for _ in 0..1500 {
        let length = 1 + next(7);
        let mut word = String::new();
        // 首字母小写，否则落不进小写前缀的范围。
        word.push(char::from(LETTERS[next(15)]));
        for _ in 1..length {
            word.push(char::from(LETTERS[next(LETTERS.len())]));
        }
        if next(40) == 0 {
            word.push('\'');
        }
        let display = match next(4) {
            0 => format!("{word}{}", DISPLAYS[next(DISPLAYS.len())]),
            1 => word.to_ascii_uppercase(),
            _ => word.clone(),
        };
        let display = (next(200) != 0).then_some(display);
        insert
            .execute(rusqlite::params![
                word,
                display,
                WEIGHTS[next(WEIGHTS.len())]
            ])
            .unwrap();
    }
}

fn session(directory: &std::path::Path, grid: KeyGrid) -> NineKeySession {
    let paths = RuntimePaths {
        resources: directory.to_path_buf(),
        user_data: directory.to_path_buf(),
        cache: directory.to_path_buf(),
        dictionaries: directory.to_path_buf(),
    };
    let mut session = NineKeySession::new(
        &paths,
        false,
        FrequencyAdjustmentOptions::default(),
        FuzzyPinyinOptions::default(),
        EnglishInputOptions::default(),
        true,
        false,
    );
    session.set_grid(grid);
    session.set_english_only(true);
    session
}

/// 缓存之前的做法：每个前缀一条带排序和 LIMIT 的 SQL。
fn reference(session: &NineKeySession) -> Vec<WordItem> {
    let english = session.english.as_ref().unwrap();
    let digits = session.digits.as_str();
    let prefixes = letter_prefixes(session.grid, digits, ENGLISH_PREFIX_BUDGET);
    let capacity = prefixes.len() * ENGLISH_LIMIT;
    let mut words = Vec::new();
    for prefix in &prefixes {
        english.append_keypad_prefix(
            prefix,
            &session.english_pattern,
            digits.len(),
            ENGLISH_LIMIT,
            &mut words,
            capacity,
        );
    }
    deduplicate_english_words(&mut words);
    rank_english(&mut words, digits.len());
    words
}

/// 深度优先打字：下一层是末尾加一位（复用缓存），回到上一层再换一位是改写（缓存作废）。每一步都和逐前缀查询逐行相同。
fn walk(
    session: &mut NineKeySession,
    digits: &mut String,
    codes: &[u8],
    depth: usize,
    hits: &mut usize,
) {
    for &code in codes {
        digits.push(char::from(code));
        session.digits.clone_from(digits);
        let words = session.english_candidates(false);
        let expected = reference(session);
        assert_eq!(words, expected, "digits {digits}");
        assert_eq!(words.capacity(), expected.capacity(), "digits {digits}");
        if session
            .english_rows
            .as_ref()
            .is_some_and(|cache| cache.rows.is_some())
        {
            *hits += 1;
        }
        if depth > 1 {
            walk(session, digits, codes, depth - 1, hits);
        }
        digits.pop();
    }
}

/// 像真人一样连着打 `length` 位，偶尔退一格；返回走缓存的刷新次数和总刷新次数。
fn type_sequences(
    session: &mut NineKeySession,
    codes: &[u8],
    sequences: usize,
    length: usize,
    seed: u64,
) -> (usize, usize) {
    let mut state = seed;
    let (mut hits, mut refreshes) = (0, 0);
    for _ in 0..sequences {
        let mut digits = String::new();
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            if state.is_multiple_of(9) && !digits.is_empty() {
                digits.pop();
            } else {
                digits.push(char::from(codes[(state % codes.len() as u64) as usize]));
            }
            session.digits.clone_from(&digits);
            let words = session.english_candidates(false);
            let expected = reference(session);
            assert_eq!(words, expected, "digits {digits}");
            assert_eq!(words.capacity(), expected.capacity(), "digits {digits}");
            refreshes += 1;
            if session
                .english_rows
                .as_ref()
                .is_some_and(|cache| cache.rows.is_some())
            {
                hits += 1;
            }
        }
    }
    (hits, refreshes)
}

#[test]
fn cached_keypad_rows_match_per_prefix_queries_on_nine_key() {
    let directory = tempfile::tempdir().unwrap();
    dense_dictionary(directory.path());
    let mut session = session(directory.path(), KeyGrid::NineKey);
    // 深度优先时下一个节点多半不是上一个的延长，覆盖缓存作废后重来。
    let mut hits = 0;
    walk(&mut session, &mut String::new(), b"249", 5, &mut hits);
    assert!(hits > 0);
    // 九键第四位起前缀组不再变化，连打七位里大约一半的刷新走缓存。
    let (hits, refreshes) = type_sequences(&mut session, b"2349", 120, 7, 0x2545_f491_4f6c_dd1d);
    assert!(hits * 3 > refreshes, "{hits}/{refreshes} cached refreshes");
}

#[test]
fn cached_keypad_rows_match_per_prefix_queries_on_fourteen_key() {
    let directory = tempfile::tempdir().unwrap();
    dense_dictionary(directory.path());
    let mut session = session(directory.path(), KeyGrid::FourteenKey);
    // 两个字母一键，前缀组到第六位才固定，第七位起走缓存。
    let (hits, refreshes) = type_sequences(&mut session, b"qet", 60, 12, 0x9e37_79b9_7f4a_7c15);
    assert!(hits * 6 > refreshes, "{hits}/{refreshes} cached refreshes");
}

#[test]
fn consuming_digits_drops_rows_that_learning_may_have_changed() {
    let directory = tempfile::tempdir().unwrap();
    dense_dictionary(directory.path());
    let mut session = session(directory.path(), KeyGrid::NineKey);
    for length in 1..=5 {
        session.digits = "22222"[..length].into();
        session.english_candidates(false);
    }
    assert!(session
        .english_rows
        .as_ref()
        .is_some_and(|cache| cache.rows.is_some()));
    Connection::open(directory.path().join(assets::ENGLISH_DICTIONARY))
        .unwrap()
        .execute_batch("UPDATE english_words SET weight=-weight;")
        .unwrap();
    session.consume(0);
    assert!(session.english_rows.is_none());
    session.digits = "222222".into();
    let words = session.english_candidates(false);
    assert_eq!(words, reference(&session));
}

/// 用真实词库核对：`MSIME_ENGLISH_DICTIONARY_DIR=<含 msime-english.db 的目录> cargo test -p msime-engine --release real_dictionary -- --ignored`。
#[test]
#[ignore = "needs the shipped msime-english.db"]
fn cached_keypad_rows_match_per_prefix_queries_on_the_real_dictionary() {
    let directory = std::env::var_os("MSIME_ENGLISH_DICTIONARY_DIR").expect("dictionary directory");
    let mut session = session(std::path::Path::new(&directory), KeyGrid::NineKey);
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut cached = 0;
    let mut checked = 0;
    for _ in 0..400 {
        let mut digits = String::new();
        for _ in 0..9 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let digit = b'2' + (state % 8) as u8;
            // 偶尔退一格再打，测改写后的重建。
            if state.is_multiple_of(7) && !digits.is_empty() {
                digits.pop();
            } else {
                digits.push(char::from(digit));
            }
            session.digits.clone_from(&digits);
            let words = session.english_candidates(false);
            assert_eq!(words, reference(&session), "digits {digits}");
            checked += 1;
            if session
                .english_rows
                .as_ref()
                .is_some_and(|cache| cache.rows.is_some())
            {
                cached += 1;
            }
        }
    }
    eprintln!("{checked} refreshes, {cached} from cached rows");
    assert!(cached > checked / 3);
}
