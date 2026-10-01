//! `V` mode unit conversion: a number followed by a unit and, optionally, the unit to convert it to (`3jin'g`, `3jintog`, `5kmmi`, `100cf`, `1mu`). The letters are matched against a closed alias table of pinyin and abbreviations; only an input the table spells completely is converted, and what reaches rink-core is a query this module assembles from the table's own unit names and the typed digits, never the typed text itself. Anything else is left to the expression path.
//!
//! The 市制 units are added to rink's definitions here: rink's own `catty`, `liang` and `li` are the older or Japanese measures, not the ones in use in mainland China. Temperature is computed directly, because its scales differ by an offset as well as a factor and a plain ratio of units would be wrong.
//!
//! Loading rink's definitions takes long enough that it must not happen on a key: the context lives in a `OnceLock`, and `warm_up` (spawned in the background when the `V` mode is enabled) builds it ahead of the first conversion.

use std::sync::{Once, OnceLock};

use rink_core::output::QueryReply;
use rink_core::parsing::text_query;
use rink_core::Context;

use super::command::TEXT_UTF16_LIMIT;
use super::expression::{format_number, RESULT_LIMIT};

/// The 市制 units as defined since 1959 (国务院《关于统一我国计量制度的命令》), in rink's definition syntax.
const SHIZHI_DEFINITIONS: &str =
    "shijin 500 g\nshiliang 50 g\nshimu 10000|15 m^2\nshili 500 m\nshichi 1|3 m\nshicun 1|30 m\n";

/// Digits in the number in front of a unit; more than this is not a measurement anyone types.
const NUMBER_LIMIT: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quantity {
    Mass,
    Length,
    Area,
    Volume,
    Temperature,
}

/// One unit of the closed table: the spellings that name it, the rink expression it stands for, the label a row shows, and whether it is listed when no target is typed.
struct Unit {
    aliases: &'static [&'static str],
    rink: &'static str,
    label: &'static str,
    quantity: Quantity,
    default_target: bool,
}

const fn unit(
    aliases: &'static [&'static str],
    rink: &'static str,
    label: &'static str,
    quantity: Quantity,
    default_target: bool,
) -> Unit {
    Unit {
        aliases,
        rink,
        label,
        quantity,
        default_target,
    }
}

/// Every unit the mode converts. An alias names exactly one unit: `mi` is the mile, as in the abbreviation typed after a number (`5kmmi`), not pinyin 米, which is `m`.
const UNITS: &[Unit] = &[
    unit(&["jin", "shijin"], "shijin", "斤", Quantity::Mass, true),
    unit(
        &["liang", "shiliang"],
        "shiliang",
        "两",
        Quantity::Mass,
        false,
    ),
    unit(&["g", "ke"], "g", "克", Quantity::Mass, true),
    unit(
        &["kg", "gongjin", "qianke"],
        "kg",
        "千克",
        Quantity::Mass,
        true,
    ),
    unit(&["mg", "haoke"], "mg", "毫克", Quantity::Mass, false),
    unit(&["t", "dun"], "tonne", "吨", Quantity::Mass, false),
    unit(&["lb", "bang"], "lb", "磅", Quantity::Mass, true),
    unit(&["oz", "angsi"], "oz", "盎司", Quantity::Mass, false),
    unit(&["li", "shili"], "shili", "里", Quantity::Length, true),
    unit(&["chi", "shichi"], "shichi", "尺", Quantity::Length, true),
    unit(&["cun", "shicun"], "shicun", "寸", Quantity::Length, false),
    unit(&["m"], "m", "米", Quantity::Length, true),
    unit(
        &["km", "gongli", "qianmi"],
        "km",
        "千米",
        Quantity::Length,
        true,
    ),
    unit(&["cm", "limi"], "cm", "厘米", Quantity::Length, true),
    unit(&["mm", "haomi"], "mm", "毫米", Quantity::Length, false),
    unit(
        &["mi", "mile", "yingli"],
        "mile",
        "英里",
        Quantity::Length,
        true,
    ),
    unit(&["ft", "yingchi"], "ft", "英尺", Quantity::Length, true),
    unit(&["inch", "yingcun"], "inch", "英寸", Quantity::Length, true),
    unit(&["yd", "ma"], "yard", "码", Quantity::Length, false),
    unit(&["mu", "shimu"], "shimu", "亩", Quantity::Area, true),
    unit(
        &["sqm", "pingfangmi"],
        "m^2",
        "平方米",
        Quantity::Area,
        true,
    ),
    unit(
        &["sqkm", "pingfanggongli"],
        "km^2",
        "平方千米",
        Quantity::Area,
        false,
    ),
    unit(&["ha", "gongqing"], "hectare", "公顷", Quantity::Area, true),
    unit(&["acre", "yingmu"], "acre", "英亩", Quantity::Area, true),
    unit(&["l", "sheng"], "liter", "升", Quantity::Volume, true),
    unit(&["ml", "haosheng"], "mL", "毫升", Quantity::Volume, true),
    unit(
        &["gal", "jialun"],
        "usgallon",
        "加仑",
        Quantity::Volume,
        true,
    ),
    unit(&["c", "sheshidu"], "", "℃", Quantity::Temperature, true),
    unit(&["f", "huashidu"], "", "℉", Quantity::Temperature, true),
    unit(&["k", "kaierwen"], "", "K", Quantity::Temperature, true),
];

static CONTEXT: OnceLock<Option<Context>> = OnceLock::new();
static WARM_UP: Once = Once::new();

/// Build the conversion context if it is not built yet. Blocks for as long as loading rink's definitions takes, so call it off the key path.
pub fn warm_up() {
    context();
}

/// Start `warm_up` on a background thread, once per process.
pub fn warm_up_in_background() {
    WARM_UP.call_once(|| {
        // A failed spawn leaves the first conversion to build the context itself, which is slower but still correct.
        let _ = std::thread::Builder::new()
            .name("msime-unit-context".to_owned())
            .spawn(warm_up);
    });
}

fn context() -> Option<&'static Context> {
    CONTEXT
        .get_or_init(|| {
            let mut context = rink_core::simple_context().ok()?;
            context.load_definitions(SHIZHI_DEFINITIONS).ok()?;
            Some(context)
        })
        .as_ref()
}

/// Rows for an input the alias table spells as `NUMBER UNIT [to|'] [UNIT]`, or `None` when it does not, so the caller can try the expression path. Each row is at most `TEXT_UTF16_LIMIT` UTF-16 units; at most `RESULT_LIMIT` are returned.
pub fn query_units(code: &str) -> Option<Vec<String>> {
    let number_end = code
        .bytes()
        .position(|byte| !(byte.is_ascii_digit() || byte == b'.'))?;
    let (number, letters) = code.split_at(number_end);
    if number.is_empty()
        || number.len() > NUMBER_LIMIT
        || number.starts_with('.')
        || number.ends_with('.')
        || number.matches('.').count() > 1
    {
        return None;
    }
    let value: f64 = number.parse().ok()?;
    let (source, target) = parse_units(letters)?;
    let targets: Vec<&Unit> = match target {
        Some(target) => vec![target],
        None => UNITS
            .iter()
            .filter(|unit| {
                unit.default_target
                    && unit.quantity == source.quantity
                    && !std::ptr::eq(*unit, source)
            })
            .collect(),
    };
    let mut rows: Vec<String> = Vec::with_capacity(RESULT_LIMIT);
    let mut push = |row: String| {
        if rows.len() < RESULT_LIMIT
            && row.encode_utf16().count() <= TEXT_UTF16_LIMIT
            && !rows.contains(&row)
        {
            rows.push(row);
        }
    };
    let explicit = target.is_some();
    for target in targets {
        let Some(converted) = convert(number, value, source, target) else {
            continue;
        };
        let Some(text) = format_number(converted) else {
            continue;
        };
        push(format!("{text}{}", target.label));
        if explicit {
            push(text.clone());
            push(format!("{number}{}={text}{}", source.label, target.label));
        }
    }
    (!rows.is_empty()).then_some(rows)
}

/// The source unit and the optional target: an apostrophe or a literal `to` may stand between them. A whole input naming one unit is read as that unit; otherwise the longest source that leaves a unit (or nothing) behind wins.
fn parse_units(letters: &str) -> Option<(&'static Unit, Option<&'static Unit>)> {
    if letters.is_empty()
        || !letters
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
    {
        return None;
    }
    if let Some((source, target)) = letters.split_once('\'') {
        let source = lookup(source)?;
        let target = if target.is_empty() {
            None
        } else {
            Some(lookup(
                target
                    .strip_prefix("to")
                    .filter(|rest| lookup(rest).is_some())
                    .unwrap_or(target),
            )?)
        };
        return compatible(source, target);
    }
    if let Some(source) = lookup(letters) {
        return Some((source, None));
    }
    for split in (1..letters.len()).rev() {
        let (source, rest) = letters.split_at(split);
        let Some(source) = lookup(source) else {
            continue;
        };
        let target = lookup(rest).or_else(|| rest.strip_prefix("to").and_then(lookup));
        if let Some(target) = target {
            if let Some(pair) = compatible(source, Some(target)) {
                return Some(pair);
            }
        }
    }
    None
}

fn compatible(
    source: &'static Unit,
    target: Option<&'static Unit>,
) -> Option<(&'static Unit, Option<&'static Unit>)> {
    match target {
        Some(target) if target.quantity != source.quantity || std::ptr::eq(target, source) => None,
        _ => Some((source, target)),
    }
}

fn lookup(alias: &str) -> Option<&'static Unit> {
    UNITS.iter().find(|unit| unit.aliases.contains(&alias))
}

/// `value` of `source` in `target`. `number` is the typed digits, which go into the rink query as written so a decimal stays exact.
fn convert(number: &str, value: f64, source: &Unit, target: &Unit) -> Option<f64> {
    if source.quantity == Quantity::Temperature {
        let kelvin = match source.label {
            "℃" => value + 273.15,
            "℉" => (value - 32.0) * 5.0 / 9.0 + 273.15,
            _ => value,
        };
        let converted = match target.label {
            "℃" => kelvin - 273.15,
            "℉" => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
            _ => kelvin,
        };
        return converted.is_finite().then_some(converted);
    }
    let context = context()?;
    // The query is built from the table's unit names and the validated digits only.
    let query = format!("({number} {}) / ({})", source.rink, target.rink);
    let mut tokens = text_query::TokenIterator::new(&query).peekable();
    let parsed = text_query::parse_query(&mut tokens);
    let QueryReply::Number(parts) = context.eval_query(&parsed).ok()? else {
        return None;
    };
    let number = parts.raw_value?;
    // A ratio of two units of one quantity has no dimension left; anything else means the table paired the wrong units.
    if !number.unit.is_empty() {
        return None;
    }
    let converted = number.value.to_f64();
    converted.is_finite().then_some(converted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(code: &str) -> Vec<String> {
        query_units(code).unwrap_or_default()
    }

    #[test]
    fn a_unit_and_a_target_convert_with_the_equation() {
        assert_eq!(rows("3jin'g"), ["1500克", "1500", "3斤=1500克"]);
        assert_eq!(rows("3jintog"), ["1500克", "1500", "3斤=1500克"]);
        assert_eq!(rows("3jing"), ["1500克", "1500", "3斤=1500克"]);
        assert_eq!(rows("5kmmi")[0], "3.10685596118667英里");
        assert_eq!(rows("1mile'km")[0], "1.609344千米");
        assert_eq!(rows("2liang'g")[0], "100克");
        assert_eq!(rows("1chi'cm")[0], "33.3333333333333厘米");
        assert_eq!(rows("1li'm")[0], "500米");
        assert_eq!(rows("1kg'lb")[0], "2.20462262184878磅");
    }

    #[test]
    fn temperatures_convert_with_their_offsets() {
        assert_eq!(rows("100cf"), ["212℉", "212", "100℃=212℉"]);
        assert_eq!(rows("32f'c")[0], "0℃");
        assert_eq!(rows("0c'k")[0], "273.15K");
    }

    #[test]
    fn a_unit_alone_lists_the_common_units_of_its_quantity() {
        let mu = rows("1mu");
        assert_eq!(
            mu,
            [
                "666.666666666667平方米",
                "0.0666666666666667公顷",
                "0.16473692097811英亩"
            ]
        );
        let jin = rows("2jin");
        assert_eq!(jin[..3], ["1000克", "1千克", "2.20462262184878磅"]);
        assert!(rows("1m").len() <= RESULT_LIMIT);
    }

    #[test]
    fn inputs_the_table_does_not_spell_are_left_to_the_expression_path() {
        for code in [
            "",
            "3",
            "1+2",
            "jin",
            "3xyz",
            "3jin'xyz",
            "3jin'kg'g",
            "3jin'm",
            "3cm'g",
            ".5jin",
            "5.jin",
            "1.2.3jin",
            "3jinjin",
            "3mm'mm",
        ] {
            assert_eq!(query_units(code), None, "{code:?}");
        }
    }

    #[test]
    fn decimals_stay_exact() {
        assert_eq!(rows("1.5jin'g")[0], "750克");
        assert_eq!(rows("0.1kg'g")[0], "100克");
    }
}
