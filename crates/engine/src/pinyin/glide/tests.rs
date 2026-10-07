use std::collections::HashMap;

use super::*;

const KEY_WIDTH: f32 = 100.0;
const KEY_HEIGHT: f32 = 150.0;

/// 各移动宿主画的 26 键布局：第二行缩进半个键，第三行越过 Shift 缩进一个半键。
pub(crate) fn keyboard() -> GlideKeyboard {
    let rows: [(&[u8], f32); 3] = [(b"qwertyuiop", 0.0), (b"asdfghjkl", 0.5), (b"zxcvbnm", 1.5)];
    let mut centers = [(0.0, 0.0); 26];
    for (row, (keys, indent)) in rows.iter().enumerate() {
        for (column, &key) in keys.iter().enumerate() {
            centers[usize::from(key - b'a')] = (
                (column as f32 + indent + 0.5) * KEY_WIDTH,
                (row as f32 + 0.5) * KEY_HEIGHT,
            );
        }
    }
    GlideKeyboard {
        centers,
        key_width: KEY_WIDTH,
        key_height: KEY_HEIGHT,
    }
}

fn center(keyboard: &GlideKeyboard, letter: u8) -> (f32, f32) {
    keyboard.centers[usize::from(letter - b'a')]
}

/// `-amplitude..amplitude` 之间的确定性抖动，让带噪声的笔画每次运行都相同。
struct Jitter(u32);

impl Jitter {
    fn next(&mut self, amplitude: f32) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.0 >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0) * amplitude
    }
}

/// 经过 `word` 各键中心的笔画，每十像素、8 毫秒一个点，每点偏移至多 `noise` 个键；`dwell_on` 里的字母上停留 200 毫秒。
pub(crate) fn stroke(word: &str, noise: f32, dwell_on: &[usize]) -> Vec<GlidePoint> {
    let keyboard = keyboard();
    let mut jitter = Jitter(word.len() as u32 * 7919 + 17);
    let mut points = Vec::new();
    let mut time = 0;
    let letters = word.as_bytes();
    let mut push = |x: f32, y: f32, time: u32, jitter: &mut Jitter| {
        points.push(GlidePoint {
            x: x + jitter.next(noise) * KEY_WIDTH,
            y: y + jitter.next(noise) * KEY_HEIGHT,
            time_ms: Some(time),
        });
    };
    for (index, pair) in letters.windows(2).enumerate() {
        let (ax, ay) = center(&keyboard, pair[0]);
        let (bx, by) = center(&keyboard, pair[1]);
        if dwell_on.contains(&index) {
            for _ in 0..25 {
                push(ax, ay, time, &mut jitter);
                time += 8;
            }
        }
        let steps = (((bx - ax).powi(2) + (by - ay).powi(2)).sqrt() / 10.0)
            .ceil()
            .max(1.0) as usize;
        for step in 0..steps {
            let t = step as f32 / steps as f32;
            push(ax + (bx - ax) * t, ay + (by - ay) * t, time, &mut jitter);
            time += 8;
        }
    }
    let (x, y) = center(&keyboard, *letters.last().expect("non-empty word"));
    if dwell_on.contains(&(letters.len() - 1)) {
        for _ in 0..25 {
            push(x, y, time, &mut jitter);
            time += 8;
        }
    }
    push(x, y, time, &mut jitter);
    points
}

fn decoded(points: &[GlidePoint]) -> Vec<String> {
    decode_glide(&keyboard(), points, 16)
        .into_iter()
        .map(|hypothesis| hypothesis.letters)
        .collect()
}

#[test]
fn clean_strokes_decode_to_the_letters_they_trace() {
    for word in [
        "nihao", "shi", "women", "xian", "xiang", "wo", "de", "shenme", "pinyin", "shurufa",
        "huanying", "qing", "jintian", "mei",
    ] {
        let result = decoded(&stroke(word, 0.0, &[]));
        assert_eq!(
            result.first().map(String::as_str),
            Some(word),
            "{word}: {result:?}"
        );
    }
}

#[test]
fn a_key_on_the_line_between_two_others_is_left_to_the_dictionary() {
    // `h` 几乎在 `z` 到 `o` 的连线上，`y` 在 `e` 到 `i` 的连线上，`d` 在 `i` 到 `a` 的连线上，所以每一对的笔画几乎相同；两个都要返回，留给词典来选。
    for (word, twin) in [
        ("zhong", "zong"),
        ("zhongguo", "zongguo"),
        ("keyi", "kei"),
        ("zhidao", "zhiao"),
    ] {
        let result = decoded(&stroke(word, 0.0, &[]));
        assert!(result.contains(&word.to_owned()), "{word}: {result:?}");
        assert!(result.contains(&twin.to_owned()), "{word}: {result:?}");
    }
}

/// `letters` 能否切成完整音节。
fn splits_into_syllables(letters: &str) -> bool {
    letters.is_empty()
        || (1..=letters.len().min(6)).any(|end| {
            crate::pinyin::syllables::is_intact(&letters[..end])
                && splits_into_syllables(&letters[end..])
        })
}

#[test]
fn noisy_strokes_still_decode() {
    for word in ["nihao", "women", "xiang", "shenme", "huanying", "jintian"] {
        let result = decoded(&stroke(word, 0.3, &[]));
        assert_eq!(
            result.first().map(String::as_str),
            Some(word),
            "{word}: {result:?}"
        );
    }
}

#[test]
fn a_corner_the_stroke_turns_on_is_a_letter() {
    // `shi` 在 `h` 上拐弯；从 `s` 到 `i` 的直线会读成 `si`。
    let result = decoded(&stroke("shi", 0.0, &[]));
    assert_eq!(result[0], "shi");
    let straight = decoded(&stroke("si", 0.0, &[]));
    assert_eq!(straight[0], "si");
    assert!(!straight.iter().take(1).any(|letters| letters == "shi"));
}

#[test]
fn a_key_passed_over_is_not_typed_but_a_dwell_on_it_is() {
    // `u` 在 `s` 到 `i` 的直线上。
    let passed = decoded(&stroke("sui", 0.0, &[]));
    assert_eq!(passed[0], "si", "{passed:?}");
    assert!(passed.contains(&"sui".to_owned()), "{passed:?}");
    let rested = decoded(&stroke("sui", 0.0, &[1]));
    assert_eq!(rested[0], "sui", "{rested:?}");
}

#[test]
fn every_hypothesis_is_complete_quanpin() {
    let hypotheses = decode_glide(&keyboard(), &stroke("zhongguo", 0.2, &[]), 8);
    assert!(!hypotheses.is_empty());
    for hypothesis in &hypotheses {
        let letters = &hypothesis.letters;
        assert!(splits_into_syllables(letters), "{letters}");
    }
    assert!(hypotheses
        .windows(2)
        .all(|pair| pair[0].cost <= pair[1].cost));
}

#[test]
fn coordinates_are_measured_in_keys() {
    let mut scaled = keyboard();
    scaled.key_width *= 3.0;
    scaled.key_height *= 0.5;
    for center in &mut scaled.centers {
        center.0 *= 3.0;
        center.1 *= 0.5;
    }
    let points: Vec<GlidePoint> = stroke("women", 0.1, &[])
        .into_iter()
        .map(|p| GlidePoint {
            x: p.x * 3.0,
            y: p.y * 0.5,
            ..p
        })
        .collect();
    let original = decode_glide(&keyboard(), &stroke("women", 0.1, &[]), 4);
    let rescaled = decode_glide(&scaled, &points, 4);
    let letters =
        |list: &[GlideHypothesis]| list.iter().map(|h| h.letters.clone()).collect::<Vec<_>>();
    assert_eq!(letters(&original), letters(&rescaled));
}

#[test]
fn unusable_input_decodes_to_nothing() {
    let points = stroke("nihao", 0.0, &[]);
    assert!(decode_glide(&keyboard(), &points[..1], 8).is_empty());
    assert!(decode_glide(&keyboard(), &points, 0).is_empty());
    let mut broken = keyboard();
    broken.key_width = 0.0;
    assert!(decode_glide(&broken, &points, 8).is_empty());
    let mut nan = points.clone();
    nan[3].x = f32::NAN;
    assert!(decode_glide(&keyboard(), &nan, 8).is_empty());
}

#[test]
fn a_very_long_stroke_is_bounded() {
    let word = "zhonghuarenmingongheguowansui";
    let points = stroke(word, 0.0, &[]);
    let started = std::time::Instant::now();
    let hypotheses = decode_glide(&keyboard(), &points, 8);
    let elapsed = started.elapsed();
    assert!(!hypotheses.is_empty());
    eprintln!(
        "{} letters, {} points: {elapsed:?}, best {:?}",
        word.len(),
        points.len(),
        hypotheses[0]
    );
}

/// 出货词典里的权重量级：单字几百万，常用词几十万。
fn shipped_weights(keys: &[String]) -> HashMap<String, i64> {
    let table: &[(&str, i64)] = &[
        ("zhong", 7_680_869),
        ("zong", 1_273_655),
        ("kei", 192),
        ("ke", 3_000_000),
        ("yi", 20_000_000),
        ("ke'yi", 505_814),
        ("shi", 31_422_712),
        ("si", 1_979_349),
        ("sui", 1_211_358),
        ("ni'hao", 332_885),
        ("ni", 9_000_000),
        ("hao", 6_000_000),
        ("zhong'guo", 505_879),
        ("guo", 8_000_000),
        ("de", 40_000_000),
        ("e", 400_000),
        ("wo", 20_000_000),
        ("wu", 3_000_000),
        ("o", 300_000),
        ("you", 12_000_000),
        ("yi'ou", 1_200),
        ("ou", 400_000),
        ("zhi'dao", 480_000),
        ("zhi", 5_000_000),
        ("dao", 3_000_000),
        ("ao", 300_000),
    ];
    keys.iter()
        .filter_map(|key| {
            table
                .iter()
                .find(|(known, _)| known == key)
                .map(|&(_, weight)| (key.clone(), weight))
        })
        .collect()
}

fn ranked(points: &[GlidePoint]) -> Vec<String> {
    rank_by_dictionary(decode_glide(&keyboard(), points, 16), shipped_weights)
        .into_iter()
        .map(|hypothesis| hypothesis.letters)
        .collect()
}

#[test]
fn the_dictionary_settles_what_the_stroke_cannot() {
    for (word, dwell) in [
        ("zhong", &[][..]),
        ("zong", &[]),
        ("keyi", &[]),
        ("zhongguo", &[]),
        ("nihao", &[]),
        ("de", &[]),
        ("wo", &[]),
        ("you", &[]),
        ("zhidao", &[]),
        ("si", &[]),
        ("shi", &[]),
        ("sui", &[1]),
    ] {
        let result = ranked(&stroke(word, 0.15, dwell));
        assert_eq!(
            result.first().map(String::as_str),
            Some(word),
            "{word}: {result:?}"
        );
    }
}

#[test]
fn a_doubled_key_needs_more_than_a_touch() {
    for word in ["de", "ta", "le"] {
        let result = decoded(&stroke(word, 0.15, &[]));
        let doubled = format!("{word}{}", &word[word.len() - 1..]);
        let plain = result.iter().position(|letters| letters == word);
        let twice = result.iter().position(|letters| *letters == doubled);
        assert_eq!(plain, Some(0), "{word}: {result:?}");
        assert!(twice.is_none_or(|at| at > 0), "{word}: {result:?}");
    }
}

#[test]
fn ranking_scores_a_string_by_its_best_word_split() {
    let hypotheses = vec![
        GlideHypothesis {
            letters: "xian".into(),
            cost: 2.0,
        },
        GlideHypothesis {
            letters: "xiaan".into(),
            cost: 2.0,
        },
    ];
    let ranked = rank_by_dictionary(hypotheses, |keys| {
        assert!(
            keys.contains(&"xi'an".to_owned()) && keys.contains(&"xian".to_owned()),
            "{keys:?}"
        );
        HashMap::from([("xi'an".to_owned(), 100_000), ("xi".to_owned(), 1_000_000)])
    });
    assert_eq!(ranked[0].letters, "xian");
}

#[test]
fn a_stroke_that_doubles_back_is_not_one_pass() {
    // 这几笔都在一条线上来回走；读成只走一遍就会是 `lao`、`keng`、`shou` 和 `gen`。
    for (word, single) in [
        ("laishuo", "lao"),
        ("keneng", "keng"),
        ("shihou", "shou"),
        ("geren", "gen"),
    ] {
        let result = decoded(&stroke(word, 0.0, &[]));
        let found = result.iter().position(|letters| letters == word);
        let one_pass = result.iter().position(|letters| letters == single);
        assert_eq!(found, Some(0), "{word}: {result:?}");
        assert!(one_pass.is_none_or(|at| at > 0), "{word}: {result:?}");
    }
}
