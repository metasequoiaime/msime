//! 把识别结果里用汉字写的数字改成打字时习惯的阿拉伯数字（二零二四年 → 2024年，百分之十二 → 12%，三百二十块 → 320块）。
//!
//! 只用于自己不做反向文本规整的模型：X-ASR transducer（`online_transducer`、`offline_transducer`）一个阿拉伯数字都不写，而 SenseVoice 和 FunASR-nano 在 sherpa-onnx 里已经打开了模型自带的 ITN。见 [`applies_to_kind`]。
//!
//! 原则是宁可少改：汉字数字大量出现在不表示数量的词里（一个、十分、万一、一点儿），改错一个比漏改一个难受得多。所以只在下面这些读起来确定是数的场合改写，其余一律保留汉字：
//!
//! - 百分数：百分之十二 → 12%，百分之三点五 → 3.5%。
//! - 年份：两到四个数字字后接「年」（二零二四年、九八年），两个数字字后接「后」或「年代」（八零后）。
//! - 完整日期：某月某日（号）。
//! - 时刻：小时后接「点」，再接「X分」、「半」或「整」；只有小时时，前面要有早上、下午这类时段词。
//! - 小数：后面跟着单位（元、公斤、万、亿……），或整数部分是零（零点五）。
//! - 带位的整数：含十、百、千、万、亿并且读得通的数（三百二十、一千四百万、两万五）。只有一个「十」时，要后接量词或前有「第」。
//! - 三个及以上的数字字连写（电话、编号）：一三八零零 → 13800。
//!
//! 约数（几十、二十几、十来个）、并列的两个数（三四个）、叠词（三三两两）和几个固定说法（二百五、一五一十）保留汉字。

/// 这种模型的最终文本是否要经过 [`convert`]。按清单里的 `kind` 判断，所以在这一规则出现之前装好的模型同样适用，不必重装。
pub fn applies_to_kind(kind: &str) -> bool {
    matches!(kind, "online_transducer" | "offline_transducer")
}

/// 读起来像数、其实不是的固定说法，整段保留。
const KEEP_AS_WRITTEN: &[&str] = &[
    "二百五",
    "一五一十",
    "十全十美",
    "五十步笑百步",
    "三十而立",
    "四十不惑",
    "十万火急",
    "十万八千里",
    "七十二变",
    "三十六计",
    "十八般",
    "十有八九",
    "九九归一",
    "百分之百",
];

/// 一天中的时段词：后面的「X点」即使没有分钟也是钟点。
const TIMES_OF_DAY: &[&str] = &[
    "凌晨", "早上", "早晨", "上午", "中午", "下午", "傍晚", "晚上", "夜里", "半夜", "今晚", "明早",
    "明晚",
];

/// 小数后面跟着这些字时一定是一个量。
const DECIMAL_UNITS: &[&str] = &[
    "万", "亿", "元", "块", "角", "毛", "米", "公里", "千米", "厘米", "毫米", "公斤", "千克", "克",
    "斤", "吨", "升", "毫升", "度", "秒", "倍", "寸", "英寸", "分",
];

/// 单独一个「十」后面跟着这些量词时才写成 10。
const MEASURES: &[&str] = &[
    "个", "人", "位", "名", "次", "遍", "天", "年", "月", "日", "号", "岁", "分钟", "秒", "小时",
    "元", "块", "米", "公里", "斤", "公斤", "件", "条", "张", "本", "页", "家", "款", "台", "辆",
    "届", "期", "章", "节", "层", "楼", "点",
];

fn digit_of(c: char) -> Option<u64> {
    let value = match c {
        '零' | '〇' => 0,
        '一' => 1,
        '二' | '两' => 2,
        '三' => 3,
        '四' => 4,
        '五' => 5,
        '六' => 6,
        '七' => 7,
        '八' => 8,
        '九' => 9,
        _ => return None,
    };
    Some(value)
}

fn place_of(c: char) -> Option<u64> {
    match c {
        '十' => Some(10),
        '百' => Some(100),
        '千' => Some(1_000),
        _ => None,
    }
}

fn is_numeral(c: char) -> bool {
    digit_of(c).is_some() || place_of(c).is_some() || c == '万' || c == '亿'
}

fn ascii_digit(value: u64) -> char {
    char::from_digit((value % 10) as u32, 10).unwrap_or('0')
}

/// 文本按字符切开后的视图，所有规则都在它上面按位置匹配。
struct Text {
    chars: Vec<char>,
}

impl Text {
    fn at(&self, index: usize) -> Option<char> {
        self.chars.get(index).copied()
    }

    fn has(&self, index: usize, word: &str) -> bool {
        word.chars()
            .enumerate()
            .all(|(offset, c)| self.at(index + offset) == Some(c))
    }

    fn has_any<'a>(&self, index: usize, words: &[&'a str]) -> Option<&'a str> {
        words.iter().copied().find(|word| self.has(index, word))
    }

    fn ends_with_any(&self, end: usize, words: &[&str]) -> bool {
        words.iter().any(|word| {
            let length = word.chars().count();
            end >= length && self.has(end - length, word)
        })
    }

    /// 从 `start` 起连续的数字字和位字的结束位置。
    fn numeral_end(&self, start: usize) -> usize {
        let mut end = start;
        while self.at(end).is_some_and(is_numeral) {
            end += 1;
        }
        end
    }

    /// 从 `start` 起连续的数字字（不含位字）的结束位置：小数点后的部分只由它们组成。
    fn digit_end(&self, start: usize) -> usize {
        let mut end = start;
        while self.at(end).and_then(digit_of).is_some() {
            end += 1;
        }
        end
    }

    fn slice(&self, start: usize, end: usize) -> &[char] {
        &self.chars[start..end]
    }
}

/// 一段只含数字字的串写成数字（二零二四 → 2024）。
fn digit_string(chars: &[char]) -> Option<String> {
    chars
        .iter()
        .map(|c| digit_of(*c).map(ascii_digit))
        .collect()
}

/// 万以下的一节：千、百、十与数字，允许中间的「零」和末尾省略的位（三千五 = 3500）。
fn section_value(chars: &[char]) -> Option<u64> {
    if chars.is_empty() {
        return None;
    }
    let mut total = 0u64;
    let mut pending: Option<u64> = None;
    let mut last_place = 10_000u64;
    let mut zero_after_place = false;
    for (index, c) in chars.iter().copied().enumerate() {
        if let Some(place) = place_of(c) {
            if place >= last_place {
                return None;
            }
            let count = match pending.take() {
                Some(digit) => digit,
                // 开头单独的「十」就是一十；其他位前面必须有数字。
                None if place == 10 && index == 0 => 1,
                None => return None,
            };
            total += count * place;
            last_place = place;
            zero_after_place = false;
        } else {
            let digit = digit_of(c)?;
            if pending.is_some() {
                // 两个数字并列（三四）读不成一个数。
                return None;
            }
            if digit == 0 {
                if zero_after_place || last_place == 10_000 {
                    return None;
                }
                zero_after_place = true;
            } else {
                pending = Some(digit);
            }
        }
    }
    match pending {
        None if zero_after_place => None,
        None => Some(total),
        Some(digit) if last_place == 10_000 => Some(digit),
        Some(digit) if zero_after_place || last_place == 10 => Some(total + digit),
        // 末尾省略了位：三千五是 3500。
        Some(digit) => Some(total + digit * last_place / 10),
    }
}

/// 整个数的值：先按亿、再按万切开，每一节交给 [`section_value`]。读不通时为 None。
fn cardinal_value(chars: &[char]) -> Option<u64> {
    if let [only] = chars {
        if digit_of(*only) == Some(0) {
            return Some(0);
        }
    }
    for (unit_char, unit) in [('亿', 100_000_000u64), ('万', 10_000u64)] {
        if let Some(split) = chars.iter().position(|c| *c == unit_char) {
            let rest = &chars[split + 1..];
            // 低位再含同级单位时必然不小于当前单位，递归前拒绝；每条路径最多经过亿、万、节三层。
            if rest.contains(&unit_char) {
                return None;
            }
            let high = cardinal_value(&chars[..split])?;
            let low = match rest {
                [] => 0,
                // 两万五：位后只剩一个数字时，它是下一位。
                [single] if digit_of(*single).is_some_and(|d| d > 0) => {
                    digit_of(*single)? * (unit / 10)
                }
                ['零', tail @ ..] => cardinal_value(tail)?,
                _ => cardinal_value(rest)?,
            };
            if high == 0 || low >= unit {
                return None;
            }
            return high.checked_mul(unit)?.checked_add(low);
        }
    }
    section_value(chars)
}

/// 一段整数的写法：以万或亿结尾、前面没有更大的位时保留这个字（一千四百万 → 1400万），否则写成完整的数。
fn cardinal_written(chars: &[char]) -> Option<String> {
    let value = cardinal_value(chars)?;
    if let Some((last, body)) = chars.split_last() {
        if (*last == '万' || *last == '亿') && !body.contains(&'万') && !body.contains(&'亿') {
            return Some(format!("{}{}", cardinal_value(body)?, last));
        }
    }
    Some(value.to_string())
}

/// 一个匹配：从当前位置到 `end` 的文字换成 `written`。
struct Match {
    end: usize,
    written: String,
}

/// 百分之 + 数（可带小数）。
fn percent(text: &Text, start: usize) -> Option<Match> {
    let number_start = start + 3;
    let run_end = text.numeral_end(number_start);
    if run_end == number_start {
        return None;
    }
    // 整段必须读得通，不能只改写「百分之三四十」开头的合法前缀。
    let value = cardinal_value(text.slice(number_start, run_end))?;
    let mut end = run_end;
    let mut written = value.to_string();
    if text.at(end) == Some('点') {
        let fraction_end = text.digit_end(end + 1);
        if let Some(fraction) =
            digit_string(text.slice(end + 1, fraction_end)).filter(|f| !f.is_empty())
        {
            written = format!("{written}.{fraction}");
            end = fraction_end;
        }
    }
    if matches!(text.at(end), Some('几' | '多')) {
        return None;
    }
    Some(Match {
        end,
        written: format!("{written}%"),
    })
}

/// 年份、年代：二零二四年、九八年、八零后、九零年代。
fn year(text: &Text, start: usize, run_end: usize) -> Option<Match> {
    let run = text.slice(start, run_end);
    // 年份不用「两」来读；带「两」的是约数（一两年、两三年）。相邻递增的两个数字（三四年、七八年）同样是约数：把它们当成年份的代价（三四年前 → 34年前）比漏改二三年、八九年这类简称大。
    if run.contains(&'两') {
        return None;
    }
    if let [first, second] = run {
        if digit_of(*second)
            .zip(digit_of(*first))
            .is_some_and(|(b, a)| b == a + 1)
        {
            return None;
        }
    }
    let digits = digit_string(run)?;
    let after_year = text.at(run_end) == Some('年') && !text.has(run_end, "年级");
    let decade = run.len() == 2 && (text.at(run_end) == Some('后') || text.has(run_end, "年代"));
    ((2..=4).contains(&run.len()) && (after_year || decade)).then_some(Match {
        end: run_end,
        written: digits,
    })
}

/// 月和日都在的日期：十月一日、三月十五号。
fn date(text: &Text, start: usize, run_end: usize) -> Option<Match> {
    let month = cardinal_value(text.slice(start, run_end))?;
    if text.at(run_end) != Some('月') || !(1..=12).contains(&month) {
        return None;
    }
    let day_start = run_end + 1;
    let day_end = text.numeral_end(day_start);
    let day = cardinal_value(text.slice(day_start, day_end))?;
    let mark = text.at(day_end).filter(|c| matches!(c, '日' | '号'))?;
    if !(1..=31).contains(&day) {
        return None;
    }
    Some(Match {
        end: day_end + 1,
        written: format!("{month}月{day}{mark}"),
    })
}

/// 时刻：小时 + 点 + 分钟/半/整，或者时段词后的「X点」。
fn clock(text: &Text, start: usize, run_end: usize) -> Option<Match> {
    let hour = cardinal_value(text.slice(start, run_end))?;
    if text.at(run_end) != Some('点') || hour > 24 {
        return None;
    }
    let after_point = run_end + 1;
    let after_time_of_day = text.ends_with_any(start, TIMES_OF_DAY);
    let minute_run = text.numeral_end(after_point);
    if minute_run > after_point {
        // 分钟：数 + 分（不是「分钟」）。点后跟着数却没有「分」时，可能是小数也可能是时刻，交给小数规则或保留。
        let said = text.slice(after_point, minute_run);
        let padded = said.len() > 1 && said[0] == '零';
        let minute = cardinal_value(if padded { &said[1..] } else { said }).filter(|m| *m < 60)?;
        if text.at(minute_run) != Some('分') || text.has(minute_run, "分钟") {
            return None;
        }
        // 「X点十分」也是「这一点十分重要」里的副词「十分」，没有时段词时分不清，保留汉字。
        if said == ['十'] && !after_time_of_day {
            return None;
        }
        let written = if padded {
            format!("{hour}点{minute:02}分")
        } else {
            format!("{hour}点{minute}分")
        };
        return Some(Match {
            end: minute_run + 1,
            written,
        });
    }
    // 半、整：「一点半」常是「一点儿」开头，所以小时是一时要有时段词。
    if let Some(mark @ ('半' | '整')) = text.at(after_point) {
        return (hour != 1 || after_time_of_day).then(|| Match {
            end: after_point + 1,
            written: format!("{hour}点{mark}"),
        });
    }
    after_time_of_day.then(|| Match {
        end: after_point,
        written: format!("{hour}点"),
    })
}

/// 小数：整数 + 点 + 数字串，后面跟单位，或整数部分为零。
fn decimal(text: &Text, start: usize, run_end: usize) -> Option<Match> {
    let whole = cardinal_value(text.slice(start, run_end))?;
    if text.at(run_end) != Some('点') {
        return None;
    }
    let fraction_end = text.digit_end(run_end + 1);
    let fraction = digit_string(text.slice(run_end + 1, fraction_end)).filter(|f| !f.is_empty())?;
    // 「分之」属于百分数，不是单位「分」。
    let unit = text
        .has_any(fraction_end, DECIMAL_UNITS)
        .filter(|u| *u != "分" || !text.has(fraction_end, "分之"));
    if unit.is_none() && text.at(start) != Some('零') {
        return None;
    }
    // 万、亿并进数里：三点五亿 → 3.5亿。
    Some(match unit {
        Some(big @ ("万" | "亿")) => Match {
            end: fraction_end + 1,
            written: format!("{whole}.{fraction}{big}"),
        },
        _ => Match {
            end: fraction_end,
            written: format!("{whole}.{fraction}"),
        },
    })
}

/// 整个数字字连写的串：三个以上写成数字（电话、编号），叠词和带「两」的不算。
fn digit_run(text: &Text, start: usize, run_end: usize) -> Option<Match> {
    let run = text.slice(start, run_end);
    if run.len() < 3 || run.contains(&'两') {
        return None;
    }
    // 三三两两、七七八八这样的 AABB 叠词。
    if run.len() == 4 && run[0] == run[1] && run[2] == run[3] {
        return None;
    }
    Some(Match {
        end: run_end,
        written: digit_string(run)?,
    })
}

/// 带位的整数。
fn cardinal(text: &Text, start: usize, run_end: usize) -> Option<Match> {
    let run = text.slice(start, run_end);
    if !run
        .iter()
        .any(|c| place_of(*c).is_some() || *c == '万' || *c == '亿')
    {
        return None;
    }
    // 约数：几十、数百、二十几、十来个。
    let before = start.checked_sub(1).and_then(|index| text.at(index));
    if matches!(before, Some('几' | '数')) || matches!(text.at(run_end), Some('几' | '来')) {
        return None;
    }
    // 单独的十和十一到十九常是别的词（十分、十全、十一长假），后面跟着量词或前面有「第」才确定是数。
    let teens = run.first() == Some(&'十') && run.len() <= 2;
    if teens && text.has_any(run_end, MEASURES).is_none() && before != Some('第') {
        return None;
    }
    Some(Match {
        end: run_end,
        written: cardinal_written(run)?,
    })
}

/// 把 `text` 里读起来确定是数的汉字数字改写成阿拉伯数字，其余原样返回。
pub fn convert(text: &str) -> String {
    let text = Text {
        chars: text.chars().collect(),
    };
    let mut out = String::with_capacity(text.chars.len() * 3);
    let mut index = 0;
    while index < text.chars.len() {
        // 固定说法后面紧跟着数字字时它只是一个更长的数的开头（二百五十块），交给下面的规则整段读。
        if let Some(kept) = text.has_any(index, KEEP_AS_WRITTEN).filter(|kept| {
            !text
                .at(index + kept.chars().count())
                .is_some_and(is_numeral)
        }) {
            out.push_str(kept);
            index += kept.chars().count();
            continue;
        }
        if let Some(found) = text
            .has(index, "百分之")
            .then(|| percent(&text, index))
            .flatten()
        {
            out.push_str(&found.written);
            index = found.end;
            continue;
        }
        if !is_numeral(text.chars[index]) {
            out.push(text.chars[index]);
            index += 1;
            continue;
        }
        let run_end = text.numeral_end(index);
        let found = year(&text, index, run_end)
            .or_else(|| date(&text, index, run_end))
            .or_else(|| clock(&text, index, run_end))
            .or_else(|| decimal(&text, index, run_end))
            .or_else(|| digit_run(&text, index, run_end))
            .or_else(|| cardinal(&text, index, run_end));
        match found {
            Some(found) => {
                out.push_str(&found.written);
                index = found.end;
            }
            None => {
                out.extend(text.slice(index, run_end));
                index = run_end;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn becomes(said: &str, written: &str) {
        assert_eq!(convert(said), written, "{said}");
    }

    #[track_caller]
    fn stays(said: &str) {
        becomes(said, said);
    }

    fn value(said: &str) -> Option<u64> {
        cardinal_value(&said.chars().collect::<Vec<_>>())
    }

    #[test]
    fn only_transducer_models_take_rule_based_numbers() {
        assert!(applies_to_kind("online_transducer"));
        assert!(applies_to_kind("offline_transducer"));
        assert!(!applies_to_kind("offline_sense_voice"));
        assert!(!applies_to_kind("offline_funasr_nano"));
        assert!(!applies_to_kind(""));
    }

    #[test]
    fn reads_whole_numbers() {
        assert_eq!(value("零"), Some(0));
        assert_eq!(value("十"), Some(10));
        assert_eq!(value("三百二十"), Some(320));
        assert_eq!(value("一千零八"), Some(1008));
        assert_eq!(value("三千五"), Some(3500));
        assert_eq!(value("两万五"), Some(25_000));
        assert_eq!(value("十二万零三百"), Some(120_300));
        assert_eq!(value("一千四百万"), Some(14_000_000));
        assert_eq!(value("三亿五千万"), Some(350_000_000));
        for nonsense in [
            "万一",
            "千万",
            "三四",
            "十十",
            "百",
            "一千零",
            "零零五",
            "九亿亿",
        ] {
            assert_eq!(value(nonsense), None, "{nonsense}");
        }
    }

    #[test]
    fn repeated_large_units_fit_a_small_stack() {
        const CHILD: &str = "MSIME_NUMBER_SMALL_STACK_CHILD";
        const COMPLETE: &str = "MSIME_NUMBER_SMALL_STACK_COMPLETE";
        let name = concat!(module_path!(), "::repeated_large_units_fit_a_small_stack")
            .split_once("::")
            .unwrap()
            .1;
        if let Some(target) = std::env::var_os(CHILD) {
            assert_eq!(target, name);
            std::thread::Builder::new()
                .stack_size(1024 * 1024)
                .spawn(|| {
                    for unit in ["一万", "一亿"] {
                        let said = unit.repeat(5_000);
                        assert_eq!(convert(&said), said);
                    }
                    let digits = "一".repeat(10_000);
                    assert_eq!(convert(&digits), "1".repeat(10_000));
                    let invalid = "一千".repeat(5_000);
                    assert_eq!(convert(&invalid), invalid);
                })
                .unwrap()
                .join()
                .unwrap();
            eprintln!("{COMPLETE}");
            return;
        }
        // 栈溢出会直接终止进程，子进程隔离避免拖垮其余测试。
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "子进程失败：{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .any(|line| line == COMPLETE),
            "子进程未完成目标测试"
        );
    }

    #[test]
    fn large_units_keep_valid_sections_and_reject_repetition() {
        assert_eq!(value("一万亿"), Some(1_000_000_000_000));
        assert_eq!(value("一万亿一万"), Some(1_000_000_010_000));
        assert_eq!(value("一亿零三万零五"), Some(100_030_005));
        for said in ["一万一万", "一亿一亿", "一亿零一亿", "一万零一万"] {
            assert_eq!(value(said), None);
            stays(said);
        }
        stays("一万一万月三号");
        stays("一亿一亿点五元");
        stays("百分之一万一万");
    }

    #[test]
    fn years_and_dates() {
        becomes("我二零二四年毕业", "我2024年毕业");
        becomes("九八年那场洪水", "98年那场洪水");
        becomes("八零后的父母", "80后的父母");
        becomes("九零年代的歌", "90年代的歌");
        becomes("十月一日放假", "10月1日放假");
        becomes("三月十五号见", "3月15号见");
        stays("三年级的学生");
        stays("五月天的演唱会");
        stays("过一两年再说");
        stays("两三年前");
        stays("三四年前的事");
        stays("七八年没见");
    }

    #[test]
    fn clock_times() {
        becomes("上午十点半开会", "上午10点半开会");
        becomes("晚上八点见", "晚上8点见");
        becomes("两点十五分出发", "2点15分出发");
        becomes("九点零五分到站", "9点05分到站");
        becomes("下午一点半到", "下午1点半到");
        becomes("十二点整吃饭", "12点整吃饭");
        stays("有一点累");
        stays("一点一点来");
        stays("这一点很重要");
        stays("一点半信半疑");
        stays("等五分钟");
        stays("这一点十分重要");
        stays("这两点十分关键");
        becomes("晚上八点十分到", "晚上8点10分到");
    }

    #[test]
    fn percentages_and_decimals() {
        becomes("涨了百分之十二", "涨了12%");
        becomes("利率百分之三点五", "利率3.5%");
        becomes("零点五公斤", "0.5公斤");
        becomes("售价九点九元", "售价9.9元");
        becomes("营收三点五亿", "营收3.5亿");
        stays("百分之百确定");
        stays("百分之二十几");
        stays("百分之三四十");
    }

    #[test]
    fn amounts_and_counts() {
        becomes("一共三百二十块", "一共320块");
        becomes("人口一千四百万", "人口1400万");
        becomes("预算两万五", "预算25000");
        becomes("月薪三千五", "月薪3500");
        becomes("第十二届", "第12届");
        becomes("十个苹果", "10个苹果");
        becomes("第十名", "第10名");
        becomes("十五个人", "15个人");
        becomes("第十一届", "第11届");
        stays("十一放假");
        stays("十一长假去哪");
        becomes("等了二十分钟", "等了20分钟");
        becomes("电话一三八零零", "电话13800");
        becomes("二百五十块", "250块");
        becomes("二百五十万", "250万");
    }

    #[test]
    fn words_that_are_not_amounts() {
        for word in [
            "一个人",
            "三四个",
            "十分好看",
            "万一下雨",
            "千万别去",
            "几十个",
            "二十几岁",
            "十来个人",
            "一五一十",
            "三三两两",
            "七七八八",
            "一一说明",
            "十全十美",
            "二百五",
            "五十步笑百步",
            "三十而立",
            "成千上万",
            "第三点",
            "两人",
        ] {
            stays(word);
        }
    }

    #[test]
    fn leaves_other_text_alone() {
        becomes(
            "下午三点十分 meeting，room 3",
            "下午3点10分 meeting，room 3",
        );
        stays("昨天是 Monday，today is 礼拜二。");
        stays("九亿亿");
        stays("零");
        becomes("凌晨零点出发", "凌晨0点出发");
        stays("");
    }
}
