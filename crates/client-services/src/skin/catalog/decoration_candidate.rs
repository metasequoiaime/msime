use super::{number_in, optional_bool, optional_string, DecorationAlign};
use serde::Serialize;
use toml::Value;

/// 装饰框内的候选词叠加；0 跟随高亮，1 起为当前页的候选序号。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinDecorationCandidate {
    pub index: u8,
    pub font_size_dip: f64,
    pub min_font_size_dip: f64,
    /// 0 使用装饰框宽度；最小字号仍放不下时允许文字超出框。
    pub max_width_dip: f64,
    /// 与来源一样保留安全的颜色声明；宿主解析失败时使用白色。
    pub color: String,
    /// 宿主可直接读取的八位十六进制颜色；原始声明仍由 `color` 保留。
    pub resolved_color: String,
    pub align_h: DecorationAlign,
    pub align_v: DecorationVerticalAlign,
    pub offset_x_dip: f64,
    pub offset_y_dip: f64,
    pub strip_badge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DecorationVerticalAlign {
    Top,
    Center,
    Bottom,
}

impl Default for SkinDecorationCandidate {
    fn default() -> Self {
        Self {
            index: 1,
            font_size_dip: 16.0,
            min_font_size_dip: 8.0,
            max_width_dip: 0.0,
            color: String::new(),
            resolved_color: "#FFFFFFFF".into(),
            align_h: DecorationAlign::Center,
            align_v: DecorationVerticalAlign::Center,
            offset_x_dip: 0.0,
            offset_y_dip: 0.0,
            strip_badge: true,
        }
    }
}

/// 按来源的数值流取色，统一量化给原生宿主，避免六个平台各自解释浮点 rgb。
fn resolved_color(value: &str) -> String {
    let value = value.trim();
    if value == "transparent" {
        return "#00000000".into();
    }
    let hex = value.strip_prefix('#').unwrap_or(value);
    if matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut hex = if hex.len() <= 4 {
            hex.chars().flat_map(|ch| [ch, ch]).collect::<String>()
        } else {
            hex.to_owned()
        };
        if hex.len() == 6 {
            hex.push_str("FF");
        }
        return format!("#{}", hex.to_ascii_uppercase());
    }
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        if let (Some(open), Some(close)) = (value.find('('), value.rfind(')')) {
            if close > open {
                let inner = value[open + 1..close].replace(',', " ");
                let mut remaining = inner.as_str();
                let mut take = || {
                    remaining = remaining.trim_start();
                    // 只取能解析成有限浮点数的最长前缀，与数值流保留后续字符的行为一致。
                    let end = remaining
                        .find(|ch: char| !ch.is_ascii_digit() && !".-eE".contains(ch))
                        .unwrap_or(remaining.len());
                    for length in (1..=end).rev() {
                        if let Ok(number) = remaining[..length].parse::<f64>() {
                            if number.is_finite() {
                                remaining = &remaining[length..];
                                return Some(number);
                            }
                        }
                    }
                    None
                };
                if let (Some(red), Some(green), Some(blue)) = (take(), take(), take()) {
                    let alpha = take().unwrap_or(1.0);
                    let byte = |value: f64, maximum: f64| {
                        (value.clamp(0.0, maximum) * 255.0 / maximum).round() as u8
                    };
                    return format!(
                        "#{:02X}{:02X}{:02X}{:02X}",
                        byte(red, 255.0),
                        byte(green, 255.0),
                        byte(blue, 255.0),
                        byte(alpha, 1.0)
                    );
                }
            }
        }
    }
    "#FFFFFFFF".into()
}

fn bounded(
    table: &toml::map::Map<String, Value>,
    key: &str,
    fallback: f64,
    minimum: f64,
    maximum: f64,
) -> Result<f64, String> {
    table.get(key).map_or(Ok(fallback), |value| {
        number_in(value, minimum..=maximum).ok_or_else(|| format!("invalid {key}"))
    })
}

/// 固定来源 `0978b5c2` 的 `ReadDecorationCandidate`；关闭时不读取其余键。
pub(super) fn read(
    table: &toml::map::Map<String, Value>,
) -> Result<Option<SkinDecorationCandidate>, String> {
    if !optional_bool(table, "candidate_overlay")?.unwrap_or(false) {
        return Ok(None);
    }
    let mut result = SkinDecorationCandidate::default();
    if let Some(value) = table.get("candidate_overlay_index") {
        result.index = value
            .as_integer()
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| *value <= 100)
            .ok_or("invalid candidate_overlay_index")?;
    }
    result.font_size_dip = bounded(table, "candidate_overlay_font_size_dip", 16.0, 0.0, 200.0)?;
    result.min_font_size_dip = bounded(
        table,
        "candidate_overlay_min_font_size_dip",
        8.0,
        0.0,
        200.0,
    )?;
    if result.font_size_dip <= 0.0 || result.min_font_size_dip <= 0.0 {
        return Err("invalid candidate overlay font size".into());
    }
    result.max_width_dip = bounded(table, "candidate_overlay_max_width_dip", 0.0, 0.0, 2000.0)?;
    if let Some(value) = table.get("candidate_overlay_color") {
        result.color = value
            .as_str()
            .filter(|value| {
                value.len() <= 80
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"#(),.% -/".contains(&byte))
            })
            .ok_or("invalid candidate_overlay_color")?
            .to_owned();
    }
    result.align_h = match optional_string(table, "candidate_overlay_align_h", 16)?.as_deref() {
        None | Some("center") => DecorationAlign::Center,
        Some("left") => DecorationAlign::Left,
        Some("right") => DecorationAlign::Right,
        Some(_) => return Err("invalid candidate_overlay_align_h".into()),
    };
    result.align_v = match optional_string(table, "candidate_overlay_align_v", 16)?.as_deref() {
        None | Some("center") => DecorationVerticalAlign::Center,
        Some("top") => DecorationVerticalAlign::Top,
        Some("bottom") => DecorationVerticalAlign::Bottom,
        Some(_) => return Err("invalid candidate_overlay_align_v".into()),
    };
    result.offset_x_dip = bounded(
        table,
        "candidate_overlay_offset_x_dip",
        0.0,
        -2000.0,
        2000.0,
    )?;
    result.offset_y_dip = bounded(
        table,
        "candidate_overlay_offset_y_dip",
        0.0,
        -2000.0,
        2000.0,
    )?;
    result.strip_badge = optional_bool(table, "candidate_overlay_strip_badge")?.unwrap_or(true);
    result.resolved_color = resolved_color(&result.color);
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    #[test]
    fn declaration_colors_follow_the_source_palette_parser() {
        for (value, expected) in [
            ("", "#FFFFFFFF"),
            ("unknown-safe-color", "#FFFFFFFF"),
            ("transparent", "#00000000"),
            ("abc", "#AABBCCFF"),
            ("#1234", "#11223344"),
            ("11223344", "#11223344"),
            ("rgb(10,20,30)", "#0A141EFF"),
            ("rgba(10.5 20 30 0.5)", "#0B141E80"),
            ("rgba(0 0 0 0)", "#00000000"),
            ("rgb(300 -1 0)", "#FF0000FF"),
            ("rgb(10 20)", "#FFFFFFFF"),
        ] {
            assert_eq!(super::resolved_color(value), expected, "{value}");
        }
    }
}
