//! 对着发布的 `msime-zhuyin.db` 跑的九键检查，默认忽略：用 `MSIME_TEST_ZHUYIN_DICTIONARY` 指向词库文件，`cargo test -p msime-engine --release -- --ignored zhuyin::shipped_dictionary_tests` 单独运行。
//!
//! 构建器改单字权重（`msime-dict-build` 的 `break_character_ties` 按 `phrase.occ` 打破并列）之后，要确认 `AMBIGUOUS_WORD_FLOOR` 当初挑选时用的三句仍然挡得住 監聽器、趕明兒、禮教，并且打印三句的转换结果和抽样常用词在九键下还原的条数，拿新旧两份词库各跑一次对照。

use super::layout::TONE_MARKS;
use super::nine_key::{self, TONE_KEYS};
use super::scheme::{ZhuyinKey, ZhuyinScheme};
use crate::language_dictionary::open_read_only;
use rusqlite::Connection;

/// 一句话的九键按键：每个音节先按符号所在的数字键，再按声调键（一声用空格）。
fn nine_key_keys(reading: &str) -> String {
    let mut keys = String::new();
    for syllable in reading.split(' ') {
        let (symbols, tone) = match TONE_MARKS[1..]
            .iter()
            .position(|mark| syllable.ends_with(mark))
        {
            Some(index) => (
                &syllable[..syllable.len() - TONE_MARKS[index + 1].len()],
                TONE_KEYS[index + 1],
            ),
            None => (syllable, b' '),
        };
        for symbol in symbols.chars() {
            let digit = nine_key::digit(symbol).unwrap_or_else(|| panic!("{symbol} 不在九键上"));
            keys.push(char::from(digit));
        }
        keys.push(char::from(tone));
    }
    keys
}

fn convert(scheme: &mut ZhuyinScheme, reading: &str) -> String {
    scheme.reset();
    for byte in nine_key_keys(reading).bytes() {
        scheme.handle_key(ZhuyinKey::Char(byte)).unwrap();
    }
    scheme.converted_text()
}

#[test]
fn nine_key_keys_follow_the_keypad() {
    // ㄋ 在 2、ㄧ 在 8，ˇ 是 c；ㄏㄠˇ 是 3 9 c；一声用空格。
    assert_eq!(nine_key_keys("ㄋㄧˇ ㄏㄠˇ"), "28c39c");
    assert_eq!(nine_key_keys("ㄐㄧㄣ ㄊㄧㄢ"), "480 280 ");
    assert_eq!(nine_key_keys("ㄇㄚ˙"), "17b");
}

#[test]
#[ignore = "需要通过 MSIME_TEST_ZHUYIN_DICTIONARY 指定发布的注音词库，用 --release 单独运行"]
fn shipped_dictionary_keeps_the_ambiguous_word_floor_sentences() {
    let path = std::env::var_os("MSIME_TEST_ZHUYIN_DICTIONARY").expect("指定发布的注音词库");
    let path = std::path::PathBuf::from(path);
    let mut scheme = ZhuyinScheme::new(open_read_only(&path).unwrap());
    scheme.set_nine_key(true);
    // `AMBIGUOUS_WORD_FLOOR` 文档里的三句和它们分别挡下的冷僻词。
    for (reading, sentence, held_back) in [
        (
            "ㄨㄛˇ ㄇㄣ˙ ㄐㄧㄣ ㄊㄧㄢ ㄑㄩˋ ㄒㄩㄝˊ ㄒㄧㄠˋ",
            "我們今天去學校",
            "監聽器",
        ),
        (
            "ㄓㄜˋ ㄍㄜ˙ ㄉㄨㄥ ㄒㄧ ㄏㄣˇ ㄆㄧㄢˊ ㄧˊ",
            "這個東西很便宜",
            "趕明兒",
        ),
        (
            "ㄑㄧㄥˇ ㄨㄣˋ ㄋㄧˇ ㄐㄧㄠˋ ㄕㄜˊ ㄇㄛˊ ㄇㄧㄥˊ ㄗˋ",
            "請問你叫什麼名字",
            "禮教",
        ),
    ] {
        let converted = convert(&mut scheme, reading);
        // 整句不一定还原：九键下 48v 同时是 ㄑㄩˋ 和 ㄒㄧˋ，dict-v2.0.56 就把 去 打成 系。这里只守下限挡下的词，整句打印出来供新旧词库对照。
        println!("{sentence}: {converted}");
        assert!(!converted.contains(held_back), "{sentence} -> {converted}");
    }

    // 抽样：2 到 4 个音节、字数等于音节数、有词频的词，每个键只取最重的一条，按权重从大到小等距取 300 条，数九键下整串还原成它自己的有几条。
    let connection = Connection::open(&path).unwrap();
    let mut rows = connection
        .prepare(
            "SELECT key, text FROM entries e WHERE weight > 0 AND weight = (SELECT max(weight) FROM entries WHERE key = e.key) ORDER BY weight DESC, key ASC, text ASC",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .filter(|(key, text)| {
            let syllables = key.split(' ').count();
            (2..=4).contains(&syllables) && text.chars().count() == syllables
        })
        .collect::<Vec<_>>();
    rows.dedup_by(|left, right| left.0 == right.0);
    let step = (rows.len() / 300).max(1);
    let sample = rows.iter().step_by(step).take(300).collect::<Vec<_>>();
    let restored = sample
        .iter()
        .filter(|(key, text)| convert(&mut scheme, key) == *text)
        .count();
    println!(
        "sampled {} of {} words, {restored} convert to themselves on nine-key",
        sample.len(),
        rows.len()
    );
}
