//! 全局主题的 id 和五套内置调色板，以及从颜色推出可读文字色的几个小函数。
//!
//! 偏好文档的 `global_theme` 和自定义主题的 `base` 要用 [`GlobalTheme`]，偏好校验要用 [`is_selectable_id`]，放在皮肤模块里会让 `preferences` 依赖皮肤模块，所以单独放在这里；`skin::theme` 原样导回这些名字，解析主题、读皮肤包仍在那里。

use crate::host_surface::HostPlatform;
use serde::{Deserialize, Serialize};

/// The global theme id stored in `Preferences::global_theme`. The picker lists them in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GlobalTheme {
    /// 跟随系统：不带调色板，按宿主当前的明暗绘制。桌面宿主画平台自己的 token；触屏键盘（Android、iOS、鸿蒙）按应用主题当前季节取色，键盘与应用同色。
    #[default]
    System,
    /// 原生：不带调色板，只画宿主系统自带键盘的 token，不跟季节。只在 [`GlobalTheme::platforms`] 列出的宿主上提供，其他宿主收到它时按 `system` 绘制。
    Native,
    Shuishan,
    Light,
    Paper,
    Night,
    Ink,
    /// The user's own theme, assembled from `Preferences::custom_theme`.
    Custom,
}

impl GlobalTheme {
    pub const ALL: [GlobalTheme; 8] = [
        GlobalTheme::System,
        GlobalTheme::Native,
        GlobalTheme::Shuishan,
        GlobalTheme::Light,
        GlobalTheme::Paper,
        GlobalTheme::Night,
        GlobalTheme::Ink,
        GlobalTheme::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            GlobalTheme::System => "system",
            GlobalTheme::Native => "native",
            GlobalTheme::Shuishan => "shuishan",
            GlobalTheme::Light => "light",
            GlobalTheme::Paper => "paper",
            GlobalTheme::Night => "night",
            GlobalTheme::Ink => "ink",
            GlobalTheme::Custom => "custom",
        }
    }

    /// The exact id, or `None` for anything else. The preference document is strict and uses serde; this is for string boundaries (a synced value, an App Group key, a manifest `base`) that must refuse a bad id. There is deliberately no lenient form: a retired or misspelt id is an error at every boundary, so a host that still writes one finds out.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.id() == id)
    }

    /// 自定义主题或皮肤包能否以这个主题为底：`system` 或内置主题。`custom` 自身不行；`native` 也不行，因为它只在部分宿主上提供，而自定义主题的底和皮肤包清单的 `base` 在每个宿主上都要画得出来。
    pub fn is_base(self) -> bool {
        !matches!(self, GlobalTheme::Custom | GlobalTheme::Native)
    }

    /// 提供这个主题的宿主；`None` 表示所有宿主都提供。其他宿主的选择器不列出它，收到它时按 `system` 绘制。
    pub fn platforms(self) -> Option<&'static [HostPlatform]> {
        match self {
            GlobalTheme::Native => Some(&[HostPlatform::Ios]),
            _ => None,
        }
    }

    /// `platform` 的选择器是否列出这个主题。
    pub fn is_offered_on(self, platform: HostPlatform) -> bool {
        self.platforms()
            .is_none_or(|platforms| platforms.contains(&platform))
    }

    /// The picker title, in the product's language.
    pub fn title(self) -> &'static str {
        match self {
            GlobalTheme::System => "跟随系统",
            GlobalTheme::Native => "原生",
            GlobalTheme::Shuishan => "水杉",
            GlobalTheme::Light => "浅色",
            GlobalTheme::Paper => "纸白",
            GlobalTheme::Night => "夜青",
            GlobalTheme::Ink => "墨",
            GlobalTheme::Custom => "自定义",
        }
    }

    /// The built-in palette, for the five themes that have one.
    pub fn builtin(self) -> Option<&'static BuiltinTheme> {
        BUILTIN_THEMES.iter().find(|theme| theme.id == self)
    }
}

/// Whether a palette is drawn on a light or a dark surface. Hosts use it for chrome they cannot colour directly (the iOS keyboard appearance, scrollbars, system menus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeAppearance {
    Light,
    Dark,
}

/// One built-in theme, exactly as the design's `THEMES` table lists it (dc.html L1484-1491).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTheme {
    pub id: GlobalTheme,
    pub appearance: ThemeAppearance,
    /// `bg`: the theme card and settings preview background.
    pub background: &'static str,
    /// `panel`: the candidate window, toolbar and menu surface.
    pub panel: &'static str,
    pub accent: &'static str,
    /// `text`: candidate and menu text.
    pub text: &'static str,
    /// `kb.bg`
    pub keyboard_background: &'static str,
    /// `kb.key`
    pub keyboard_key: &'static str,
    /// `kb.spec`: function keys (shift, delete, symbols, space row modifiers).
    pub keyboard_function_key: &'static str,
    /// `kb.fg`
    pub keyboard_text: &'static str,
    /// `kb.sub`: key hints, candidate numbers and secondary candidate text.
    pub keyboard_secondary: &'static str,
}

/// The five built-in palettes, in picker order.
pub const BUILTIN_THEMES: [BuiltinTheme; 5] = [
    BuiltinTheme {
        id: GlobalTheme::Shuishan,
        appearance: ThemeAppearance::Dark,
        background: "#1E1F1C",
        panel: "#2A2B27",
        accent: "#7FE08E",
        text: "#FFFFFF",
        keyboard_background: "#1E1F1C",
        keyboard_key: "#2F302C",
        keyboard_function_key: "#23241F",
        keyboard_text: "#FFFFFF",
        keyboard_secondary: "#9FB5A3",
    },
    BuiltinTheme {
        id: GlobalTheme::Light,
        appearance: ThemeAppearance::Light,
        background: "#E9E9E9",
        panel: "#FFFFFF",
        accent: "#005FB8",
        text: "#1A1A1A",
        keyboard_background: "#E4E6EA",
        keyboard_key: "#FFFFFF",
        keyboard_function_key: "#C8CCD3",
        keyboard_text: "#1A1A1A",
        keyboard_secondary: "#6A6F76",
    },
    BuiltinTheme {
        id: GlobalTheme::Paper,
        appearance: ThemeAppearance::Light,
        background: "#E8E4DA",
        panel: "#F7F5F0",
        accent: "#2C7A4B",
        text: "#1A1E1B",
        keyboard_background: "#E6E1D5",
        keyboard_key: "#FBF9F4",
        keyboard_function_key: "#D3CCBC",
        keyboard_text: "#1A1E1B",
        keyboard_secondary: "#6E6A5E",
    },
    BuiltinTheme {
        id: GlobalTheme::Night,
        appearance: ThemeAppearance::Dark,
        background: "#0F1B22",
        panel: "#16262F",
        accent: "#4FD1C5",
        text: "#E6F1F4",
        keyboard_background: "#0F1B22",
        keyboard_key: "#1D3340",
        keyboard_function_key: "#15252E",
        keyboard_text: "#E6F1F4",
        keyboard_secondary: "#86A6B0",
    },
    BuiltinTheme {
        id: GlobalTheme::Ink,
        appearance: ThemeAppearance::Dark,
        background: "#0B0B0B",
        panel: "#1A1A1A",
        accent: "#FFFFFF",
        text: "#9A9A9A",
        keyboard_background: "#111111",
        keyboard_key: "#2A2A2A",
        keyboard_function_key: "#1C1C1C",
        keyboard_text: "#F2F2F2",
        keyboard_secondary: "#9A9A9A",
    },
];

/// The candidate window border every built-in theme draws: the design's `rgba(0,0,0,.12)`.
pub const BUILTIN_CANDIDATE_BORDER: &str = "#0000001F";
/// Alpha appended to the accent for the selected candidate row: the design's `accent + '24'` (about 14%).
pub const SELECTED_ALPHA: &str = "24";
/// Alpha appended to the text colour for a hovered candidate row. The design draws no hover state; 6% of the text colour keeps hover visibly weaker than selection on both light and dark panels.
pub const HOVER_ALPHA: &str = "0F";
/// Alpha appended to a custom keyboard's key text for its secondary text (key hints). The keyboard editor has no secondary colour; 60% matches how far `kb.sub` sits from `kb.fg` in the built-in themes.
pub const CUSTOM_KEYBOARD_SECONDARY_ALPHA: &str = "99";
/// Alpha appended to a custom text picker colour for the index numbers (and so the secondary text) when the number picker is unset: an explicit text colour also sets the numbers, about 62% opaque, as every host drew it before the global theme.
pub const PICKED_NUMBER_ALPHA: &str = "9D";

/// Colours for the candidate window, and by derivation the floating toolbar and the menus (surface = `surface`, item text = `text`, hovered item = `hover`, checked or highlighted item = `selected` fill with `selected_text`, separators and outline = `border`).
///
/// A `None` slot is the host's native token for that slot. `show_selected_bar` is `None` unless an external package says otherwise, leaving the selection indicator to the platform.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateThemePalette {
    pub surface: Option<String>,
    pub border: Option<String>,
    /// Candidate text.
    pub text: Option<String>,
    /// Candidate index labels.
    pub number: Option<String>,
    /// Secondary candidate text: comments, translations, pinyin hints.
    pub secondary: Option<String>,
    pub accent: Option<String>,
    /// Selected row fill.
    pub selected: Option<String>,
    /// Text of the selected candidate.
    pub selected_text: Option<String>,
    /// Index label of the selected candidate.
    pub selected_number: Option<String>,
    /// Hovered row fill.
    pub hover: Option<String>,
    pub show_selected_bar: Option<bool>,
}

/// Colours for a touch keyboard, and on touch hosts for the candidate strip above it (strip background = `background`, candidates = `text`, hints = `secondary`, the selected candidate = `accent` text with no fill, as the design draws it: dc.html `mobCands` `mFg`).
///
/// The return key while composing is not a theme colour: the design fills it with the platform accent and white text in every theme (dc.html L2211), so hosts keep their own token there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardThemePalette {
    pub background: String,
    pub key: String,
    pub function_key: String,
    pub text: String,
    pub secondary: String,
    /// The selected candidate in the strip (text, no fill), and the accent marks a host draws on its keys (hints, toggled keys, focus).
    pub accent: String,
    /// Text readable on an `accent` fill, for hosts that fill a key or chip with the accent: black or white by the accent's luminance.
    pub on_accent: String,
}

/// The theme card preview: the design's `bg`, `panel`, `accent` and `text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemePreview {
    pub background: String,
    pub panel: String,
    pub accent: String,
    pub text: String,
}

/// One entry of `catalog()`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ThemeCatalogEntry {
    pub id: GlobalTheme,
    pub title: &'static str,
    /// 提供这个主题的宿主（见 [`GlobalTheme::platforms`]），`None` 表示所有宿主都提供。
    pub platforms: Option<&'static [HostPlatform]>,
    /// `None` for `system`, `native` and `custom`, whose appearance depends on the platform or on what the user assembled.
    pub appearance: Option<ThemeAppearance>,
    pub preview: Option<ThemePreview>,
    pub candidate: Option<CandidateThemePalette>,
    pub keyboard: Option<KeyboardThemePalette>,
}

impl BuiltinTheme {
    pub fn preview(&self) -> ThemePreview {
        ThemePreview {
            background: self.background.to_owned(),
            panel: self.panel.to_owned(),
            accent: self.accent.to_owned(),
            text: self.text.to_owned(),
        }
    }

    /// The design's non-system candidate override: `candBg = panel`, `candBorder = rgba(0,0,0,.12)`, `candSelBg = accent + '24'`, `candSelFg = accent`, secondary text in `kb.sub`.
    pub fn candidate(&self) -> CandidateThemePalette {
        CandidateThemePalette {
            surface: Some(self.panel.to_owned()),
            border: Some(BUILTIN_CANDIDATE_BORDER.to_owned()),
            text: Some(self.text.to_owned()),
            number: Some(self.keyboard_secondary.to_owned()),
            secondary: Some(self.keyboard_secondary.to_owned()),
            accent: Some(self.accent.to_owned()),
            selected: Some(format!("{}{SELECTED_ALPHA}", self.accent)),
            selected_text: Some(self.accent.to_owned()),
            selected_number: Some(self.keyboard_secondary.to_owned()),
            hover: Some(format!("{}{HOVER_ALPHA}", self.text)),
            show_selected_bar: None,
        }
    }

    pub fn keyboard(&self) -> KeyboardThemePalette {
        KeyboardThemePalette {
            background: self.keyboard_background.to_owned(),
            key: self.keyboard_key.to_owned(),
            function_key: self.keyboard_function_key.to_owned(),
            text: self.keyboard_text.to_owned(),
            secondary: self.keyboard_secondary.to_owned(),
            accent: self.accent.to_owned(),
            on_accent: rgb(readable_text(
                u32::from_str_radix(&self.accent[1..7], 16)
                    .expect("built-in theme colours are #RRGGBB"),
            )),
        }
    }
}

/// `color` 的低 24 位写成 `#RRGGBB`（大写）。
pub fn rgb(color: u32) -> String {
    format!("#{:06X}", color & 0xFF_FFFF)
}

/// sRGB 颜色 `rgb`（`0xRRGGBB`）的相对亮度，按 WCAG 的公式计算。
pub fn luminance(rgb: u32) -> f64 {
    let channel = |shift: u32| {
        let value = ((rgb >> shift) & 0xFF) as f64 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

/// 画在 `background`（`0xRRGGBB`）上时可读的文字色：按亮度取黑（`0`）或白（`0xFFFFFF`）。
pub fn readable_text(background: u32) -> u32 {
    if luminance(background) > 0.179 {
        0
    } else {
        0xFFFFFF
    }
}

/// 偏好里保存的皮肤选择（`custom_theme.candidate_skin`）要满足的规则：文件夹名的形状，且不是全局主题 ID。它比 `skin::catalog::is_external_id` 宽：被 msime-windows 内置外观或 `default` 占用的名字以前可以当皮肤文件夹，旧设置和其他设备同步来的设置可能还写着它们；它们仍是合法的设置值，只是目录里不会有这个包，自定义主题照常画它的 base。若按 `is_external_id` 校验，这样的设置会让整份偏好被拒绝。后加的全局主题 ID `native` 同理：加它之前 `native` 是合法的皮肤文件夹名，所以仍可保存。
pub fn is_selectable_id(id: &str) -> bool {
    crate::text::safe_id(id)
        && GlobalTheme::from_id(id).is_none_or(|theme| theme == GlobalTheme::Native)
}
