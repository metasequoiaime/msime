//! `V` mode: an arithmetic expression evaluated by `exmex`, a number written out in Chinese numerals by `chinese-number`, a `YYYY.M.D` date, or a measurement converted to another unit (`3jin'g`, see `units`). Rows are computed on the key that changed the input and read nothing from disk; an input that is none of these (an operator still waiting for its operand) has no rows, so the session shows the raw text instead.
//!
//! Arithmetic follows the usual precedence: `^` first, then `*`, `/` and `%` left to right, then `+` and `-` left to right. exmex only has left-associative binary operators and signs that bind tighter than any of them, so the two inputs where that disagrees with written mathematics, a chained power (`2^3^2`) and a sign in front of a power (`-2^2`), get no rows rather than a disputed number; brackets make either one unambiguous.

use chinese_number::{ChineseCase, ChineseCountMethod, ChineseVariant, NumberToChinese};
use exmex::{ops_factory, BinOp, Express, FlatEx, MakeOperators, Operator};
use time::{Date, Month};

use super::date_time::{chinese_number, lunar_date, year_digits, LocalDateTime, WEEKDAYS};
use super::units::query_units;
use crate::types::{CandidateSource, WordItem};

/// Digits and the arithmetic the mode spells with; letters are not part of it. A unit's letters are taken by the session only once a digit is in the input, and never change what a host sends as a character.
pub const SPELLING_SYMBOLS: &str = "0123456789+-*/.()%^";
/// No input produces more rows than one page of the nine-row Windows candidate window.
pub const RESULT_LIMIT: usize = 9;
/// Numbers from here on are not written out in Chinese numerals: past 万亿 the readings stop being something people write.
const CHINESE_NUMERAL_LIMIT: u64 = 1_000_000_000_000_000;
/// Fractional results are shown to this many significant digits, the most an f64 always carries, which hides binary rounding: 0.1+0.2 reads 0.3.
const SIGNIFICANT_DIGITS: usize = 15;
/// 2^53: integers below it are exact in an f64 and are shown in full.
const EXACT_INTEGER_LIMIT: f64 = 9_007_199_254_740_992.0;

// The default float operators minus the named functions and constants, which the mode cannot spell, plus `%` as the remainder. `*`, `/` and `%` share a priority, as do `+` and `-`, so each group evaluates left to right. None is marked commutative: exmex ranks a commutative operator between two literals above its own priority, which would compute `2*6%4` as `2*(6%4)`.
ops_factory!(
    ArithmeticOps,
    f64,
    Operator::make_bin(
        "^",
        BinOp {
            apply: f64::powf,
            prio: 4,
            is_commutative: false,
        }
    ),
    Operator::make_bin(
        "*",
        BinOp {
            apply: |a, b| a * b,
            prio: 3,
            is_commutative: false,
        }
    ),
    Operator::make_bin(
        "/",
        BinOp {
            apply: |a, b| a / b,
            prio: 3,
            is_commutative: false,
        }
    ),
    Operator::make_bin(
        "%",
        BinOp {
            apply: |a, b| a % b,
            prio: 3,
            is_commutative: false,
        }
    ),
    Operator::make_bin_unary(
        "+",
        BinOp {
            apply: |a, b| a + b,
            prio: 1,
            is_commutative: false,
        },
        |a| a
    ),
    Operator::make_bin_unary(
        "-",
        BinOp {
            apply: |a, b| a - b,
            prio: 1,
            is_commutative: false,
        },
        |a| -a
    )
);

/// Generated rows for the input after `V`, weight `count - index`, at most `RESULT_LIMIT`.
pub fn query_expression(code: &str) -> Vec<WordItem> {
    if let Some(texts) = query_units(code) {
        return generated_rows(texts);
    }
    if code.is_empty()
        || !code
            .chars()
            .all(|character| SPELLING_SYMBOLS.contains(character))
    {
        return Vec::new();
    }
    let texts = if let Some(date) = parse_date(code) {
        date_rows(date)
    } else if code
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        number_rows(code)
    } else {
        expression_rows(code)
    };
    generated_rows(texts)
}

/// At most `RESULT_LIMIT` Generated rows, weight `count - index`.
fn generated_rows(texts: Vec<String>) -> Vec<WordItem> {
    let count = texts.len().min(RESULT_LIMIT);
    texts
        .into_iter()
        .take(count)
        .enumerate()
        .map(|(index, text)| {
            WordItem::new(
                "",
                text,
                (count - index) as i64,
                CandidateSource::Generated,
                "",
            )
        })
        .collect()
}

/// The result, the equation, and the amount in capitals when the result is one.
fn expression_rows(code: &str) -> Vec<String> {
    if power_is_disputed(code) {
        return Vec::new();
    }
    let Some(value) = evaluate(code) else {
        return Vec::new();
    };
    let Some(result) = format_number(value) else {
        return Vec::new();
    };
    let mut rows = vec![result.clone(), format!("{code}={result}")];
    rows.extend(amount_in_capitals(&result));
    rows
}

/// A chained power (`2^3^2`, which exmex reads left to right and mathematics right to left) or a sign in front of a power (`-2^2`, where exmex applies the sign first).
fn power_is_disputed(code: &str) -> bool {
    let bytes = code.as_bytes();
    // The end of the operand starting at `index` after any signs: a number or a bracketed group.
    let operand_end = |mut index: usize| {
        while matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        if bytes.get(index) == Some(&b'(') {
            let mut depth = 0usize;
            while let Some(&byte) = bytes.get(index) {
                index += 1;
                match byte {
                    b'(' => depth += 1,
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        } else {
            while bytes
                .get(index)
                .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'.')
            {
                index += 1;
            }
        }
        index
    };
    bytes.iter().enumerate().any(|(index, &byte)| {
        let sign = byte == b'-'
            && (index == 0
                || matches!(
                    bytes[index - 1],
                    b'+' | b'-' | b'*' | b'/' | b'%' | b'^' | b'('
                ));
        (byte == b'^' || sign) && bytes.get(operand_end(index + 1)) == Some(&b'^')
    })
}

fn evaluate(code: &str) -> Option<f64> {
    let expression = FlatEx::<f64, ArithmeticOps>::parse(code).ok()?;
    // The mode cannot spell a name, but a variable would make `eval` fail rather than answer anyway.
    if !expression.var_names().is_empty() {
        return None;
    }
    expression
        .eval(&[])
        .ok()
        .filter(|value: &f64| value.is_finite())
}

/// An exact integer in full and anything else to `SIGNIFICANT_DIGITS` significant digits, positional between one millionth and 10^16 and in exponent form outside it; `None` for infinity and NaN.
pub(super) fn format_number(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let rounded: f64 = if value.fract() == 0.0 && value.abs() < EXACT_INTEGER_LIMIT {
        value
    } else {
        let text = format!("{value:.precision$e}", precision = SIGNIFICANT_DIGITS - 1);
        let rounded: f64 = text.parse().ok()?;
        // 有限极值的十进制舍入可能超过 f64 范围，直接保留文本，避免显示成无穷值。
        if !rounded.is_finite() {
            return Some(text);
        }
        rounded
    };
    // Adding zero turns -0 into 0, so 1-1 and -0 both read 0.
    let rounded = rounded + 0.0;
    let magnitude = rounded.abs();
    Some(if magnitude != 0.0 && !(1e-6..1e16).contains(&magnitude) {
        format!("{rounded:e}")
    } else {
        rounded.to_string()
    })
}

/// A plain number: lowercase, capitals and the amount for an integer; the reading with 点 and, for at most two decimals, the amount for a decimal.
fn number_rows(code: &str) -> Vec<String> {
    let (integer, fraction) = match code.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (code, None),
    };
    if integer.is_empty()
        || fraction.is_some_and(|fraction| fraction.is_empty() || fraction.contains('.'))
    {
        return Vec::new();
    }
    let Some(value) = integer
        .parse::<u64>()
        .ok()
        .filter(|value| *value < CHINESE_NUMERAL_LIMIT)
    else {
        return Vec::new();
    };
    let Some(lower) = chinese_integer(value, ChineseCase::Lower) else {
        return Vec::new();
    };
    let Some(fraction) = fraction else {
        let mut rows = vec![lower];
        rows.extend(chinese_integer(value, ChineseCase::Upper));
        rows.extend(amount_in_capitals(code));
        return rows;
    };
    let digits: Option<String> = fraction
        .bytes()
        .map(|digit| chinese_digit(digit - b'0', ChineseCase::Lower))
        .collect();
    let mut rows: Vec<String> = digits
        .map(|digits| format!("{lower}点{digits}"))
        .into_iter()
        .collect();
    rows.extend(amount_in_capitals(code));
    rows
}

/// 壹佰贰拾叁元整, 壹拾元零伍分, 伍角: the capitals written on invoices and cheques, for a non-negative plain number with at most two decimals below `CHINESE_NUMERAL_LIMIT`.
fn amount_in_capitals(number: &str) -> Option<String> {
    let (integer, fraction) = number.split_once('.').unwrap_or((number, ""));
    if integer.is_empty()
        || fraction.len() > 2
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let yuan = integer
        .parse::<u64>()
        .ok()
        .filter(|value| *value < CHINESE_NUMERAL_LIMIT)?;
    let mut digits = fraction.bytes().map(|digit| digit - b'0');
    let jiao = digits.next().unwrap_or(0);
    let fen = digits.next().unwrap_or(0);
    let mut text = String::new();
    if yuan > 0 || (jiao == 0 && fen == 0) {
        text.push_str(&chinese_integer(yuan, ChineseCase::Upper)?);
        text.push('元');
    }
    if jiao == 0 && fen == 0 {
        text.push('整');
        return Some(text);
    }
    if jiao > 0 {
        text.push_str(&chinese_digit(jiao, ChineseCase::Upper)?);
        text.push('角');
    } else if yuan > 0 {
        text.push('零');
    }
    if fen > 0 {
        text.push_str(&chinese_digit(fen, ChineseCase::Upper)?);
        text.push('分');
    }
    Some(text)
}

/// Capitals spell out the leading one of 10-19 (壹拾肆, not 拾肆), as amounts are written so that nothing can be added in front; everyday numerals drop it (十四). Counting is 中數, where 兆 is 10^16, so everything below `CHINESE_NUMERAL_LIMIT` reads in 万 and 亿 (一万亿, not 一兆) as amounts are written.
fn chinese_integer(value: u64, case: ChineseCase) -> Option<String> {
    let text = value
        .to_chinese(ChineseVariant::Simple, case, ChineseCountMethod::Middle)
        .ok()?;
    Some(if case == ChineseCase::Upper && text.starts_with('拾') {
        format!("壹{text}")
    } else {
        text
    })
}

fn chinese_digit(digit: u8, case: ChineseCase) -> Option<String> {
    digit.to_chinese_naive(ChineseVariant::Simple, case).ok()
}

/// `YYYY.M.D` naming a real day.
fn parse_date(code: &str) -> Option<Date> {
    let mut parts = code.split('.');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    let digits = |part: &str, lengths: std::ops::RangeInclusive<usize>| {
        lengths.contains(&part.len()) && part.bytes().all(|byte| byte.is_ascii_digit())
    };
    if parts.next().is_some()
        || !digits(year, 4..=4)
        || !digits(month, 1..=2)
        || !digits(day, 1..=2)
    {
        return None;
    }
    let month = Month::try_from(month.parse::<u8>().ok()?).ok()?;
    Date::from_calendar_date(year.parse().ok()?, month, day.parse().ok()?).ok()
}

fn date_rows(date: Date) -> Vec<String> {
    let year = date.year();
    let month = u32::from(u8::from(date.month()));
    let day = u32::from(date.day());
    let weekday = WEEKDAYS[usize::from(date.weekday().number_days_from_sunday())];
    let mut rows = vec![
        format!("{year}年{month}月{day}日"),
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{year}年{month}月{day}日 {weekday}"),
        format!(
            "{}年{}月{}日",
            year_digits(year.unsigned_abs()),
            chinese_number(month),
            chinese_number(day)
        ),
        weekday.to_owned(),
    ];
    // A date the lunar calendar does not cover drops the row, as the date/time mode does.
    rows.extend(lunar_date(&LocalDateTime {
        year,
        month,
        day,
        ..LocalDateTime::default()
    }));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(code: &str) -> Vec<String> {
        query_expression(code)
            .into_iter()
            .map(|row| row.word.into_string())
            .collect()
    }

    #[test]
    fn arithmetic_shows_the_result_the_equation_and_the_amount() {
        assert_eq!(words("1+2"), ["3", "1+2=3", "叁元整"]);
        assert_eq!(words("0.1+0.2"), ["0.3", "0.1+0.2=0.3", "叁角"]);
        assert_eq!(words("2*(3+4)"), ["14", "2*(3+4)=14", "壹拾肆元整"]);
        assert_eq!(words("2^10"), ["1024", "2^10=1024", "壹仟零贰拾肆元整"]);
        assert_eq!(words("7%3"), ["1", "7%3=1", "壹元整"]);
        assert_eq!(words("1/3"), ["0.333333333333333", "1/3=0.333333333333333"]);
        assert_eq!(words("10-12"), ["-2", "10-12=-2"]);
        assert_eq!(words("-(1-1)"), ["0", "-(1-1)=0", "零元整"]);
        assert_eq!(words("10^20"), ["1e20", "10^20=1e20"]);
    }

    #[test]
    fn rows_are_generated_with_descending_weights() {
        let rows = query_expression("1+2");
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(row.source, CandidateSource::Generated);
            assert_eq!(row.weight, (rows.len() - index) as i64);
            assert!(row.pinyin.is_empty());
        }
    }

    #[test]
    fn incomplete_or_undefined_expressions_have_no_rows() {
        for code in [
            "", "1+", "(1+2", "1/0", "0/0", "*3", "1..2", "2^99999", "1+a", "１+2",
        ] {
            assert!(words(code).is_empty(), "{code:?}");
        }
    }

    #[test]
    fn integers_are_written_in_chinese_numerals() {
        assert_eq!(words("123"), ["一百二十三", "壹佰贰拾叁", "壹佰贰拾叁元整"]);
        assert_eq!(words("10"), ["十", "壹拾", "壹拾元整"]);
        assert_eq!(words("14"), ["十四", "壹拾肆", "壹拾肆元整"]);
        assert_eq!(
            words("100200"),
            ["十万零二百", "壹拾万零贰佰", "壹拾万零贰佰元整"]
        );
        assert_eq!(
            words("1010"),
            ["一千零一十", "壹仟零壹拾", "壹仟零壹拾元整"]
        );
        assert_eq!(words("0"), ["零", "零", "零元整"]);
        assert_eq!(words("1000000000000")[..2], ["一万亿", "壹万亿"]);
        assert_eq!(words("999999999999999").len(), 3);
        assert!(words("1000000000000000").is_empty());
    }

    #[test]
    fn decimals_read_with_the_point_and_as_amounts() {
        assert_eq!(
            words("123.45"),
            ["一百二十三点四五", "壹佰贰拾叁元肆角伍分"]
        );
        assert_eq!(words("10.05"), ["十点零五", "壹拾元零伍分"]);
        assert_eq!(words("0.5"), ["零点五", "伍角"]);
        assert_eq!(words("12.00"), ["十二点零零", "壹拾贰元整"]);
        assert_eq!(words("3.14159"), ["三点一四一五九"]);
        assert!(words("1.").is_empty());
        assert!(words(".5").is_empty());
    }

    #[test]
    fn dates_read_in_chinese_with_the_weekday_and_the_lunar_date() {
        assert_eq!(
            words("2026.10.1"),
            [
                "2026年10月1日",
                "2026-10-01",
                "2026年10月1日 星期四",
                "二〇二六年十月一日",
                "星期四",
                "丙午年八月二十一日",
            ]
        );
        assert_eq!(words("2024.2.29")[0], "2024年2月29日");
        // Not a day, and not arithmetic either.
        for code in ["2026.2.30", "2026.13.1", "26.1.1", "2026.1.1.1"] {
            assert!(words(code).is_empty(), "{code:?}");
        }
    }

    #[test]
    fn exact_results_keep_every_digit() {
        assert_eq!(
            words("1234567890123+1"),
            [
                "1234567890124",
                "1234567890123+1=1234567890124",
                "壹万贰仟叁佰肆拾伍亿陆仟柒佰捌拾玖万零壹佰贰拾肆元整"
            ]
        );
        assert_eq!(
            words("98765432101.55+1"),
            [
                "98765432102.55",
                "98765432101.55+1=98765432102.55",
                "玖佰捌拾柒亿陆仟伍佰肆拾叁万贰仟壹佰零贰元伍角伍分"
            ]
        );
    }

    #[test]
    fn operators_of_one_priority_evaluate_left_to_right() {
        assert_eq!(words("2*6%4")[0], "0");
        assert_eq!(words("3*10%7")[0], "2");
        assert_eq!(words("6%4*2")[0], "4");
        assert_eq!(words("12/4*3")[0], "9");
        assert_eq!(words("1-2+3")[0], "2");
        assert_eq!(words("2+3*4^2")[0], "50");
    }

    #[test]
    fn disputed_powers_have_no_rows() {
        for code in ["2^3^2", "-2^2", "3*-2^2", "-(1+1)^2", "2^-3^2"] {
            assert!(words(code).is_empty(), "{code:?}");
        }
        assert_eq!(words("2^(3^2)")[0], "512");
        assert_eq!(words("(2^3)^2")[0], "64");
        assert_eq!(words("(-2)^2")[0], "4");
        assert_eq!(words("-(2^2)")[0], "-4");
        assert_eq!(words("2^-2")[0], "0.25");
        assert_eq!(words("-2*3")[0], "-6");
    }

    #[test]
    fn finite_limits_remain_decimal_results_after_rounding() {
        assert_eq!(
            format_number(f64::MAX).as_deref(),
            Some("1.79769313486232e308")
        );
        assert_eq!(
            format_number(-f64::MAX).as_deref(),
            Some("-1.79769313486232e308")
        );
        for (code, expected) in [
            ("(2^1023)*1.9999999999999998", "1.79769313486232e308"),
            ("(2^1023)*-1.9999999999999998", "-1.79769313486232e308"),
        ] {
            assert_eq!(
                words(code),
                [expected.to_owned(), format!("{code}={expected}")]
            );
        }
        assert!(words("(2^1023)*2").is_empty());
    }

    #[test]
    fn numbers_format_to_fifteen_significant_digits() {
        assert_eq!(format_number(0.1 + 0.2).as_deref(), Some("0.3"));
        assert_eq!(format_number(0.1 * 3.0).as_deref(), Some("0.3"));
        assert_eq!(format_number(-0.0).as_deref(), Some("0"));
        assert_eq!(format_number(1e-7).as_deref(), Some("1e-7"));
        assert_eq!(
            format_number(123456789012.6).as_deref(),
            Some("123456789012.6")
        );
        assert_eq!(
            format_number(1.2345678901234).as_deref(),
            Some("1.2345678901234")
        );
        assert_eq!(
            format_number(9_007_199_254_740_991.0).as_deref(),
            Some("9007199254740991")
        );
        assert_eq!(format_number(f64::INFINITY), None);
        assert_eq!(format_number(f64::NAN), None);
    }
}
