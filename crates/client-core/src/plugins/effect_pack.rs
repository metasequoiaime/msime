//! `kind = "effect"`: parameters for the typing effect a host draws on keys and commits.
//!
//! ```toml
//! [effect]
//! style = "sparks"            # required: "flash" | "sparks" | "power_mode"
//! intensity = 70              # optional, 0-100, default 50
//! colors = ["#FFB000", "#FF4060"]   # optional, 1-4 entries, each "#RRGGBB"
//! duration_ms = 400           # optional, 60-1500
//! particles = 24              # optional, 0-64
//! ```
//!
//! Nothing here is drawn by the pack: every style is built into the hosts, and an effect pack only picks one of them and tunes it within fixed bounds, so it carries no files besides its manifest and text notices. Every optional parameter is a hint a host may honour or ignore: a host that draws only a flash has no particles to count, and Linux shows only the combo count. A parameter left out means the host's own default for the style.

use serde::Serialize;
use std::path::Path;
use toml::Value;

use super::{only_keys, EffectStyle, PluginContent, PluginKind};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["effect"];

/// The keys of the `[effect]` table.
const EFFECT_KEYS: [&str; 5] = ["style", "intensity", "colors", "duration_ms", "particles"];

/// `intensity` when the manifest leaves it out, the same as a fresh profile's `effect_intensity`.
pub const DEFAULT_INTENSITY: u8 = 50;
/// The largest `intensity`.
pub const MAX_INTENSITY: u8 = 100;
/// Entries in `colors`, when it is given.
pub const COLOR_COUNT: std::ops::RangeInclusive<usize> = 1..=4;
/// `duration_ms`: how long one effect is drawn.
pub const DURATION_MILLIS: std::ops::RangeInclusive<u16> = 60..=1_500;
/// The largest `particles`: sparks emitted per key.
pub const MAX_PARTICLES: u8 = 64;

/// One effect pack's parameters, every value already range-checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EffectPack {
    /// Never `Off`: a pack that draws nothing would be a pack nobody needs.
    pub style: EffectStyle,
    pub intensity: u8,
    /// `#RRGGBB` strings as the manifest wrote them; empty for the host's own colours.
    pub colors: Vec<String>,
    pub duration_ms: Option<u16>,
    pub particles: Option<u8>,
}

pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<EffectPack, String> {
    let effect = table
        .get("effect")
        .and_then(Value::as_table)
        .ok_or("特效包缺少 effect 表")?;
    only_keys(effect, &EFFECT_KEYS, "effect")?;
    let style = match effect.get("style").and_then(Value::as_str) {
        Some("flash") => EffectStyle::Flash,
        Some("sparks") => EffectStyle::Sparks,
        Some("power_mode") => EffectStyle::PowerMode,
        _ => return Err("effect.style 只能是 flash、sparks 或 power_mode".into()),
    };
    let integer = |key: &str, maximum: i64| -> Result<Option<i64>, String> {
        match effect.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_integer()
                .filter(|value| (0..=maximum).contains(value))
                .map(Some)
                .ok_or_else(|| format!("effect.{key} 必须是 0 到 {maximum} 之间的整数")),
        }
    };
    let intensity = integer("intensity", i64::from(MAX_INTENSITY))?
        .map_or(DEFAULT_INTENSITY, |value| value as u8);
    let particles = integer("particles", i64::from(MAX_PARTICLES))?.map(|value| value as u8);
    let duration_ms = match effect.get("duration_ms") {
        None => None,
        Some(value) => Some(
            value
                .as_integer()
                .and_then(|value| u16::try_from(value).ok())
                .filter(|value| DURATION_MILLIS.contains(value))
                .ok_or_else(|| {
                    format!(
                        "effect.duration_ms 必须是 {} 到 {} 之间的整数",
                        DURATION_MILLIS.start(),
                        DURATION_MILLIS.end()
                    )
                })?,
        ),
    };
    let colors = match effect.get("colors") {
        None => Vec::new(),
        Some(value) => {
            let items = value.as_array().ok_or("effect.colors 必须是数组")?;
            if !COLOR_COUNT.contains(&items.len()) {
                return Err(format!(
                    "effect.colors 必须有 {} 到 {} 个颜色",
                    COLOR_COUNT.start(),
                    COLOR_COUNT.end()
                ));
            }
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .filter(|color| is_color(color))
                        .map(str::to_owned)
                        .ok_or_else(|| "effect.colors 里的颜色必须写成 #RRGGBB".to_owned())
                })
                .collect::<Result<_, _>>()?
        }
    };
    Ok(EffectPack {
        style,
        intensity,
        colors,
        duration_ms,
        particles,
    })
}

/// `#` and exactly six hexadecimal digits, either case. No shorthand, no alpha, no names: every host parses the same seven bytes.
pub fn is_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

/// The typing effect a host draws, after the selected effect pack has been resolved against the preferences. Hosts get this, not the pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TypingEffect {
    /// The effect pack selected in `plugins.effect_pack`; `None` when none is, and the style and intensity are the preferences' own.
    pub pack: Option<String>,
    /// Why the selected pack could not be used, in Chinese for a log line. The effect is then `Off` rather than another style: a host does not fall back to a pack the user did not pick, as with the sound packs.
    pub issue: Option<String>,
    pub style: EffectStyle,
    pub intensity: u8,
    pub colors: Vec<String>,
    pub duration_ms: Option<u16>,
    pub particles: Option<u8>,
}

impl Default for TypingEffect {
    fn default() -> Self {
        Self::from_preferences(EffectStyle::Off, DEFAULT_INTENSITY)
    }
}

impl TypingEffect {
    /// No pack: the style and intensity the preferences hold, and the host's own colours, duration and particles.
    pub fn from_preferences(style: EffectStyle, intensity: u8) -> Self {
        Self {
            pack: None,
            issue: None,
            style,
            intensity: intensity.min(MAX_INTENSITY),
            colors: Vec::new(),
            duration_ms: None,
            particles: None,
        }
    }

    /// Resolve `plugins.effect_pack`: empty selects no pack and leaves `style` and `intensity` (the preferences' `effect_style` and `effect_intensity`) in force; anything else is loaded from `<root>/effect/<pack>` by the rules `scan` lists packs by, and its parameters replace them. `root` is the plugins directory, `None` when the host named no state root, which no installed pack can be found without. Reads one small manifest: not for the key path.
    pub fn resolve(root: Option<&Path>, pack: &str, style: EffectStyle, intensity: u8) -> Self {
        if pack.is_empty() {
            return Self::from_preferences(style, intensity);
        }
        let loaded = match root {
            Some(root) => super::load_package(root, None, PluginKind::Effect, pack),
            None => Err("没有插件目录，无法载入特效包".to_owned()),
        };
        match loaded {
            Ok(package) => {
                let PluginContent::Effect(effect) = package.content else {
                    unreachable!("load_package checks the kind");
                };
                Self {
                    pack: Some(package.id),
                    issue: None,
                    style: effect.style,
                    intensity: effect.intensity,
                    colors: effect.colors,
                    duration_ms: effect.duration_ms,
                    particles: effect.particles,
                }
            }
            Err(issue) => Self {
                pack: Some(pack.to_owned()),
                issue: Some(issue),
                ..Self::from_preferences(EffectStyle::Off, intensity)
            },
        }
    }
}
