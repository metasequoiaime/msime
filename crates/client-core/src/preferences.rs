//! Versioned local preferences. Hosts supply a private application data directory.
//! All writers coordinate through the stable lock file, not the replaced data file.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(any(not(unix), test))]
use std::fs;
use std::fs::File;
#[cfg(any(not(unix), test))]
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The largest preference document accepted by the shared host boundary. This
/// covers a validated custom skin photo while preventing a damaged local file
/// from forcing an unbounded allocation during startup or recovery.
const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;

fn valid_font_family(value: &str) -> bool {
    !value.is_empty() && crate::text::is_bounded_text(value, 128)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InputScheme {
    #[default]
    Quanpin,
    Shuangpin,
    Wubi,
    Japanese,
    /// Korean Hangul on the Dubeolsik layout. The Engine ordinal is 4.
    Korean,
    /// Cantonese in toneless Jyutping, read from `msime-cantonese.db`. A Chinese scheme. The Engine ordinal is 5.
    Cantonese,
    /// Bopomofo on the Dachen layout, read from `msime-zhuyin.db`. A Chinese scheme. The Engine ordinal is 6.
    Zhuyin,
    /// Vietnamese through Telex or VNI, set in `vietnamese`. The Engine ordinal is 7.
    Vietnamese,
    /// 藏文：在拉丁字母键盘上按 EWTS（扩展威利转写）输入，不用词库，也不是中文方案。Engine 序号为 8。
    Tibetan,
    /// 笔画：按横竖撇点折（h s p n z，x 为通配）笔顺输入单字，读取 `msime-stroke.db`。是中文方案。Engine 序号为 9。
    Stroke,
}

/// Presentation layout for touch keyboard hosts. Desktop hosts preserve but ignore it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TouchKeyboardLayout {
    #[default]
    TwentySixKey,
    NineKey,
    Handwriting,
}

/// 触屏九宫格数字层的排列。桌面宿主原样保留、不使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NumberKeypadOrder {
    /// 电话拨号盘：1 2 3 在最上面。
    #[default]
    Phone,
    /// 计算器：7 8 9 在最上面，1 2 3 在最下面。
    Calculator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchSkinKeyShape {
    Rounded,
    Capsule,
    Ticket,
    Pebble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchSkinKeyMaterial {
    Flat,
    Raised,
    Glass,
    Paper,
}

/// Apple-compatible keyboard editor design: the keyboard half of the custom theme. Named designs live in a separate bounded library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct TouchKeyboardSkinDesign {
    pub background: u32,
    pub key_background: u32,
    pub key_foreground: u32,
    pub accent: u32,
    pub action_background: u32,
    pub corner_radius: f64,
    pub border_width: f64,
    pub shadow: f64,
    pub pattern: u8,
    pub monospaced: bool,
    pub key_shape: Option<TouchSkinKeyShape>,
    pub key_material: Option<TouchSkinKeyMaterial>,
    pub key_opacity: Option<f64>,
    pub gradient_end: Option<u32>,
    pub gradient_horizontal: Option<bool>,
    pub pattern_opacity: Option<f64>,
    pub custom_border_color: Option<u32>,
    /// Base64 image bytes, matching Swift JSONEncoder's Data representation.
    pub photo: Option<String>,
    pub photo_shade: Option<f64>,
    pub photo_position: Option<f64>,
}

impl Default for TouchKeyboardSkinDesign {
    fn default() -> Self {
        Self {
            background: 0xE8F0EB,
            key_background: 0xFFFFFF,
            key_foreground: 0x17251D,
            accent: 0x185C47,
            action_background: 0x185C47,
            corner_radius: 8.0,
            border_width: 0.0,
            shadow: 0.0,
            pattern: 0,
            monospaced: false,
            key_shape: None,
            key_material: None,
            key_opacity: None,
            gradient_end: None,
            gradient_horizontal: None,
            pattern_opacity: None,
            custom_border_color: None,
            photo: None,
            photo_shade: None,
            photo_position: None,
        }
    }
}

impl TouchKeyboardSkinDesign {
    /// 薄荷晨光：薄荷绿渐变底、圆角白键、深绿文字，即皮肤社区里同名的水杉精选设计。曾是触屏键盘新装的默认皮肤（#2178），现在新装跟随系统，它仍排在各端皮肤模板的第一位。
    pub fn mint_morning() -> Self {
        Self {
            background: 0xD8F0E4,
            key_background: 0xFAFFF9,
            key_foreground: 0x173D30,
            accent: 0x245A43,
            action_background: 0x245A43,
            corner_radius: 14.0,
            border_width: 0.5,
            shadow: 0.08,
            pattern: 0,
            monospaced: false,
            gradient_end: Some(0xEEF6DD),
            custom_border_color: Some(0xB6D8C5),
            ..Self::default()
        }
    }

    pub(crate) fn validate(&self) -> bool {
        let colors = [
            Some(self.background),
            Some(self.key_background),
            Some(self.key_foreground),
            Some(self.accent),
            Some(self.action_background),
            self.gradient_end,
            self.custom_border_color,
        ];
        if colors.into_iter().flatten().any(|color| color > 0xFFFFFF)
            || !self.corner_radius.is_finite()
            || !(0.0..=20.0).contains(&self.corner_radius)
            || !self.border_width.is_finite()
            || !(0.0..=2.0).contains(&self.border_width)
            || !self.shadow.is_finite()
            || !(0.0..=0.4).contains(&self.shadow)
            || self.pattern > 3
            || !valid_optional_number(self.key_opacity, 0.25, 1.0)
            || !valid_optional_number(self.pattern_opacity, 0.0, 0.5)
            || !valid_optional_number(self.photo_shade, 0.0, 0.8)
            || !valid_optional_number(self.photo_position, 0.0, 1.0)
        {
            return false;
        }
        let Some(photo) = self.photo.as_ref() else {
            return true;
        };
        if photo.len() > 682_668 {
            return false;
        }
        let Ok(bytes) = BASE64.decode(photo) else {
            return false;
        };
        bytes.len() <= 512_000 && supported_skin_photo(&bytes)
    }

    /// Apple-compatible normalization used when a named design is loaded from its library.
    pub fn normalized(mut self) -> Self {
        self.background &= 0xFFFFFF;
        self.key_background &= 0xFFFFFF;
        self.key_foreground &= 0xFFFFFF;
        self.accent &= 0xFFFFFF;
        self.action_background &= 0xFFFFFF;
        self.corner_radius = normalized_number(self.corner_radius, 0.0, 20.0, 8.0);
        self.border_width = normalized_number(self.border_width, 0.0, 2.0, 0.0);
        self.shadow = normalized_number(self.shadow, 0.0, 0.4, 0.0);
        if self.pattern > 3 {
            self.pattern = 0;
        }
        self.key_opacity = self
            .key_opacity
            .map(|value| normalized_number(value, 0.25, 1.0, 1.0));
        self.gradient_end = self.gradient_end.map(|color| color & 0xFFFFFF);
        self.pattern_opacity = self
            .pattern_opacity
            .map(|value| normalized_number(value, 0.0, 0.5, 0.15));
        self.custom_border_color = self.custom_border_color.map(|color| color & 0xFFFFFF);
        self.photo_shade = self
            .photo_shade
            .map(|value| normalized_number(value, 0.0, 0.8, 0.25));
        self.photo_position = self
            .photo_position
            .map(|value| normalized_number(value, 0.0, 1.0, 0.5));
        if self.photo.is_some() && !self.validate() {
            self.photo = None;
        }
        self
    }
}

fn normalized_number(value: f64, minimum: f64, maximum: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}

fn valid_optional_number(value: Option<f64>, minimum: f64, maximum: f64) -> bool {
    value.is_none_or(|value| value.is_finite() && (minimum..=maximum).contains(&value))
}

fn supported_skin_photo(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"\x89PNG\r\n\x1A\n")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
}

/// What the `custom` global theme is made of. It is kept while another theme is selected, so switching back restores it.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomTheme {
    /// 自定义主题的底：`system` 或五个内置主题之一，不能是 `custom`，也不能是只在部分宿主上提供的 `native`（见 `GlobalTheme::is_base`）。皮肤包和取色器没设的候选颜色由它补上；没有键盘设计时，键盘也画它的。应用了皮肤包时，改用包清单自己的 `base`。
    #[serde(skip_serializing_if = "is_system_theme")]
    pub base: crate::skin::theme::GlobalTheme,
    /// The external candidate skin package (a folder name in the host's skin root) whose colours and decoration the custom theme uses. Never a global theme id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_skin: Option<String>,
    /// The candidate colour pickers, drawn over the package's colours.
    #[serde(skip_serializing_if = "CustomCandidateColors::is_empty")]
    pub candidate_colors: CustomCandidateColors,
    /// The keyboard editor design; community, saved and AI-generated keyboard skins are applied by writing it here. `None` until the user designs or applies one: the custom theme then draws the base theme's keyboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<TouchKeyboardSkinDesign>,
}

fn is_system_theme(theme: &crate::skin::theme::GlobalTheme) -> bool {
    *theme == crate::skin::theme::GlobalTheme::System
}

impl CustomTheme {
    /// The same checks `Preferences::validate` applies to `custom_theme`, for hosts that receive a custom theme outside a preferences document.
    pub fn validate(&self) -> Result<(), PreferencesError> {
        if !self.base.is_base() {
            return Err(PreferencesError::InvalidCustomThemeBase);
        }
        if self
            .keyboard
            .as_ref()
            .is_some_and(|design| !design.validate())
        {
            return Err(PreferencesError::InvalidTouchKeyboardSkinDesign);
        }
        let colors = &self.candidate_colors;
        for (color, error) in [
            (&colors.text, PreferencesError::InvalidCandidateTextColor),
            (
                &colors.number,
                PreferencesError::InvalidCandidateNumberColor,
            ),
            (
                &colors.accent,
                PreferencesError::InvalidCandidateAccentColor,
            ),
            (
                &colors.selected,
                PreferencesError::InvalidCandidateSelectedColor,
            ),
            (&colors.hover, PreferencesError::InvalidCandidateHoverColor),
            (
                &colors.surface,
                PreferencesError::InvalidCandidateSurfaceColor,
            ),
            (
                &colors.border,
                PreferencesError::InvalidCandidateBorderColor,
            ),
        ] {
            if color
                .as_deref()
                .is_some_and(|color| !crate::is_hex_color(color, &[6]))
            {
                return Err(error);
            }
        }
        // 与皮肤目录的文件夹名同一形状，但沿用较宽的 `is_selectable_id`：被 msime-windows 内置外观占用的旧皮肤名仍可保存，只是目录里找不到它。
        if self
            .candidate_skin
            .as_deref()
            .is_some_and(|skin| !crate::skin::catalog::is_selectable_id(skin))
        {
            return Err(PreferencesError::InvalidCandidateSkin);
        }
        Ok(())
    }
}

/// The seven candidate colour pickers, each `#RRGGBB` or unset.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CustomCandidateColors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
}

impl CustomCandidateColors {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Stable Apple-compatible entries shown by touch-keyboard scheme pickers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchKeyboardScheme {
    Quanpin,
    NineKey,
    Xiaohe,
    Ziranma,
    Microsoft,
    Shoudao,
    Wubi,
    JapaneseNineKey,
    Japanese,
    Handwriting,
    Korean,
    /// 粤拼 26 键: toneless Jyutping on the pinyin 26-key letters (`InputScheme::Cantonese`).
    Cantonese,
    /// 大千注音: bopomofo on the four-row Dachen keyboard, each key sending its Dachen ASCII key (`InputScheme::Zhuyin`).
    Zhuyin,
    /// 越南语 26 键: Vietnamese on the Latin 26-key letters, composed by the method in `Preferences::vietnamese` (`InputScheme::Vietnamese`).
    Vietnamese,
    /// 藏文 26 键：在拉丁 26 键字母上按 EWTS 威利转写输入藏文，字母区分大小写（`InputScheme::Tibetan`）。
    Tibetan,
    /// 笔画键盘：横竖撇点折加一个通配键，每个键发送对应的笔画字母 h s p n z 或 x（`InputScheme::Stroke`）。不论选的是 26 键还是九键布局，宿主都画这个笔画键盘。
    Stroke,
    /// 注音 9 键：数字键 1-0 各承载几个注音符号（US 6,009,444 FIG.1 的分组），声调键 ˉ ˊ ˇ ˋ ˙ 结束一个音节，与大千注音一样输出繁体（`InputScheme::Zhuyin` 配 `TouchKeyboardLayout::NineKey`）。
    ZhuyinNineKey,
}

impl TouchKeyboardScheme {
    /// Every touch scheme in picker order. Schemes are appended, never reordered.
    pub const ALL: [Self; 17] = [
        Self::Quanpin,
        Self::NineKey,
        Self::Xiaohe,
        Self::Ziranma,
        Self::Microsoft,
        Self::Shoudao,
        Self::Wubi,
        Self::JapaneseNineKey,
        Self::Japanese,
        Self::Handwriting,
        Self::Korean,
        Self::Cantonese,
        Self::Zhuyin,
        Self::Vietnamese,
        Self::Tibetan,
        Self::Stroke,
        Self::ZhuyinNineKey,
    ];

    /// 新装时键盘显示的方案：只有中文方案（拼音、双拼、五笔和手写）。日文（9 键和 26 键）、韩文、粤拼、注音（大千和 9 键）、越南文、藏文和笔画都由用户自己在「添加语言」里打开：日文词典和语言词库在 Android 上按需下载，默认不启用它们，新装用户就不会看到一个还没有词典的键盘。
    ///
    /// 只用于新装：还没有偏好文件、状态目录也没有被以前的版本准备过的时候（见 `PreferencesStore` 读不到文件时的处理）。已有的文档没存列表时按 [`Self::LEGACY_DEFAULT_ENABLED`]，存过列表的照旧按它保存的列表。
    pub const DEFAULT_ENABLED: [Self; 8] = [
        Self::Quanpin,
        Self::NineKey,
        Self::Xiaohe,
        Self::Ziranma,
        Self::Microsoft,
        Self::Shoudao,
        Self::Wubi,
        Self::Handwriting,
    ];

    /// 默认值改成只有中文之前的默认启用列表：除粤拼、注音（大千和 9 键）、越南文、藏文和笔画以外的全部，日文（9 键和 26 键）和韩文也在里面。
    ///
    /// 以前的版本在启用列表等于这份默认值时不把 `touch_keyboard_schemes` 写进文档，所以已有文档里没有 `enabled` 就表示用户看到的是这份列表。读入这样的文档、以及以前的版本准备过却从没存过偏好的状态目录，都按这份列表，升级不会让日文和韩文键盘消失。现在的版本总是把列表写出来，不会再产生没有列表的文档。与 Android `KeyboardScheme.optIn` 之外的那些方案一致。
    pub const LEGACY_DEFAULT_ENABLED: [Self; 11] = [
        Self::Quanpin,
        Self::NineKey,
        Self::Xiaohe,
        Self::Ziranma,
        Self::Microsoft,
        Self::Shoudao,
        Self::Wubi,
        Self::JapaneseNineKey,
        Self::Japanese,
        Self::Handwriting,
        Self::Korean,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "StoredTouchKeyboardSchemePreferences")]
pub struct TouchKeyboardSchemePreferences {
    pub enabled: BTreeSet<TouchKeyboardScheme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<TouchKeyboardScheme>,
}

/// 文档里可能出现的方案取值：现行方案，加上已经退役的 `thoughtful_reply`。高情商回复曾是一个触屏方案，现在改为键盘工具栏上的工具，旧文档里留下的这个取值在读入时迁移掉，而不是让整份偏好读取失败。其它不认识的取值（比如更新版本写入的方案）仍然报错，保持文档原样不动。
#[derive(Deserialize)]
enum StoredTouchKeyboardScheme {
    #[serde(rename = "thoughtful_reply")]
    RetiredThoughtfulReply,
    #[serde(untagged)]
    Current(TouchKeyboardScheme),
}

impl StoredTouchKeyboardScheme {
    fn current(self) -> Option<TouchKeyboardScheme> {
        match self {
            Self::RetiredThoughtfulReply => None,
            Self::Current(scheme) => Some(scheme),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTouchKeyboardSchemePreferences {
    #[serde(default)]
    enabled: Option<Vec<StoredTouchKeyboardScheme>>,
    #[serde(default)]
    selected: Option<StoredTouchKeyboardScheme>,
}

impl From<StoredTouchKeyboardSchemePreferences> for TouchKeyboardSchemePreferences {
    fn from(stored: StoredTouchKeyboardSchemePreferences) -> Self {
        let enabled = match stored.enabled {
            // 没存列表的文档只可能出自默认值改成只有中文之前的版本，用户当时看到的是那时的默认列表。
            None => TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED
                .into_iter()
                .collect(),
            Some(stored_enabled) => {
                let stored_count = stored_enabled.len();
                let mut enabled: BTreeSet<_> = stored_enabled
                    .into_iter()
                    .filter_map(StoredTouchKeyboardScheme::current)
                    .collect();
                // 列表里只剩退役方案时，按各平台对空列表的约定回到全拼 26 键；原本就存成空列表的文档不在这里补，仍由 `validate` 拒绝。
                if enabled.is_empty() && stored_count > 0 {
                    enabled.insert(TouchKeyboardScheme::Quanpin);
                }
                enabled
            }
        };
        let mut preferences = Self {
            enabled,
            selected: None,
        };
        preferences.selected = stored.selected.map(|selected| {
            selected
                .current()
                .unwrap_or_else(|| preferences.first_enabled())
        });
        preferences
    }
}

impl Default for TouchKeyboardSchemePreferences {
    fn default() -> Self {
        Self {
            enabled: TouchKeyboardScheme::DEFAULT_ENABLED.into_iter().collect(),
            selected: None,
        }
    }
}

impl TouchKeyboardSchemePreferences {
    /// 文档里整个没有 `touch_keyboard_schemes` 时读到的值：与只缺 `enabled` 一样按 [`TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED`]。
    fn without_stored_list() -> Self {
        Self {
            enabled: TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED
                .into_iter()
                .collect(),
            selected: None,
        }
    }

    /// `edition` 的触屏键盘还没被用户改过时启用的方案：[`TouchKeyboardScheme::DEFAULT_ENABLED`] 里本版本提供的那些（见 `Edition::offers_touch_scheme`）。full 得到的就是 `Default`。
    ///
    /// 手写只在提供中文方案的版本里有（手写识别器只认汉字），手写入口写进偏好的 `scheme` 是本版本的默认方案，五笔版里就是五笔。
    ///
    /// 只有一个方案的版本启用这个方案的全部触屏入口：越南文和藏文在 full 里默认停用，单独成为一个版本时它们就是这个版本本身。启用的入口按 `ALL` 顺序第一个是手写时，第一个不是手写的入口同时设为选中，否则第一次打开键盘看到的是手写；现有版本里手写要么没有、要么排在本版本的方案后面，选中留空，与只取缺省集合相同。
    pub fn for_edition(edition: &crate::edition::Edition) -> Self {
        Self::for_edition_from(edition, &TouchKeyboardScheme::DEFAULT_ENABLED)
    }

    /// [`Self::for_edition`]，只是缺省集合换成以前的版本用的 [`TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED`]：以前的版本准备过、却从没存过偏好的状态目录读到的就是它，与这些用户升级前看到的键盘相同。
    fn legacy_for_edition(edition: &crate::edition::Edition) -> Self {
        Self::for_edition_from(edition, &TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED)
    }

    fn for_edition_from(
        edition: &crate::edition::Edition,
        defaults: &[TouchKeyboardScheme],
    ) -> Self {
        let single_scheme = edition.input_schemes.len() == 1;
        let mut preferences = Self {
            enabled: TouchKeyboardScheme::ALL
                .into_iter()
                .filter(|scheme| {
                    edition.offers_touch_scheme(*scheme)
                        && (single_scheme || defaults.contains(scheme))
                })
                .collect(),
            selected: None,
        };
        if preferences.first_enabled() == TouchKeyboardScheme::Handwriting {
            preferences.selected = preferences
                .enabled
                .iter()
                .copied()
                .find(|scheme| *scheme != TouchKeyboardScheme::Handwriting);
        }
        preferences
    }

    /// 选中的方案不可用时退回的方案：按 `ALL` 顺序第一个启用的方案，一个都没有时是全拼 26 键。
    pub fn first_enabled(&self) -> TouchKeyboardScheme {
        TouchKeyboardScheme::ALL
            .into_iter()
            .find(|scheme| self.enabled.contains(scheme))
            .unwrap_or(TouchKeyboardScheme::Quanpin)
    }
}

/// Which state a new focus session starts in.
///
/// Windows starts in English, as the source product does: its factory template (`installer/default_config/config.default.toml`, `[input] default_ime_mode = "english"`) is what a fresh reference install runs with, and `platforms/windows/installer/config.default.toml` ships the same, but the running host reads this document, so the effective first-run value on Windows was Chinese. Both the Server's mode authority and the TIP's own read go through this default when the document has no value yet.
///
/// The other hosts start in Chinese, because that is what this input method is for: opening in English means the first thing a new user does is find the switch. The macOS host already resolved anything but an explicit "english" to Chinese on its own. A stored value is untouched either way; this answers only for a document that does not have the key yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefaultImeMode {
    Chinese,
    English,
}

impl Default for DefaultImeMode {
    fn default() -> Self {
        if cfg!(windows) {
            Self::English
        } else {
            Self::Chinese
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ImeModeScope {
    #[default]
    App,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChineseScheme {
    Quanpin,
    Shuangpin,
    Wubi,
    Cantonese,
    Zhuyin,
    Stroke,
}

impl ChineseScheme {
    /// `scheme` 是中文方案时对应的 `ChineseScheme`，日文、韩文、越南文等方案没有。
    pub fn of(scheme: InputScheme) -> Option<Self> {
        match scheme {
            InputScheme::Quanpin => Some(Self::Quanpin),
            InputScheme::Shuangpin => Some(Self::Shuangpin),
            InputScheme::Wubi => Some(Self::Wubi),
            InputScheme::Cantonese => Some(Self::Cantonese),
            InputScheme::Zhuyin => Some(Self::Zhuyin),
            InputScheme::Stroke => Some(Self::Stroke),
            InputScheme::Japanese
            | InputScheme::Korean
            | InputScheme::Vietnamese
            | InputScheme::Tibetan => None,
        }
    }
}

impl From<ChineseScheme> for InputScheme {
    fn from(scheme: ChineseScheme) -> Self {
        match scheme {
            ChineseScheme::Quanpin => Self::Quanpin,
            ChineseScheme::Shuangpin => Self::Shuangpin,
            ChineseScheme::Wubi => Self::Wubi,
            ChineseScheme::Cantonese => Self::Cantonese,
            ChineseScheme::Zhuyin => Self::Zhuyin,
            ChineseScheme::Stroke => Self::Stroke,
        }
    }
}

/// How Vietnamese letters and tones are typed. The Engine code is the declaration order (`vietnamese_input_method`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VietnameseInputMethod {
    #[default]
    Telex,
    Vni,
}

/// Where the tone mark goes in an `oa`, `oe` or `uy` syllable: modern places it on the second vowel (hoà), classic on the first (hòa). The Engine code is the declaration order (`vietnamese_tone_style`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VietnameseToneStyle {
    #[default]
    Modern,
    Classic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct VietnamesePreferences {
    pub input_method: VietnameseInputMethod,
    pub tone_style: VietnameseToneStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PunctuationLock {
    #[default]
    Follow,
    Chinese,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TranslationTargetLanguage {
    #[default]
    En,
    Fr,
    Ja,
    Es,
    Ru,
    De,
    Ko,
}

/// The character width used by desktop hosts for printable ASCII output.
/// This is separate from `floating_toolbar.fullwidth`, which controls whether
/// the toolbar exposes the width switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CharacterWidthPreference {
    #[default]
    Halfwidth,
    Fullwidth,
}

/// Candidate sentence-association sources. The dictionary lattice keeps its historical default; neural rerankers are opt-in because they add model work while typing or settling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SentenceAssociationPreferences {
    #[serde(default = "enabled_by_default")]
    pub word_lattice: bool,
    /// Runs the desktop sentence model as the input runtime's settled reranker once typing pauses, when a host has installed it; the Engine never loads that model.
    #[serde(default)]
    pub neural_desktop: bool,
    #[serde(default)]
    pub neural_keyboard: bool,
    #[serde(default)]
    pub show_next_on_duplicate: bool,
}

impl Default for SentenceAssociationPreferences {
    fn default() -> Self {
        Self {
            word_lattice: true,
            neural_desktop: false,
            neural_keyboard: false,
            show_next_on_duplicate: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    #[serde(default)]
    pub default_ime_mode: DefaultImeMode,
    #[serde(default)]
    pub ime_mode_scope: ImeModeScope,
    #[serde(default)]
    pub voice_input: VoiceInputPreferences,
    #[serde(default)]
    pub ai_assistant: AiAssistantPreferences,
    /// Controls local whole-sentence candidate sources. Neural reranking remains opt-in until
    /// its model is installed.
    #[serde(default)]
    pub sentence_association: SentenceAssociationPreferences,
    #[serde(default)]
    pub custom_translation: CustomTranslationPreferences,
    #[serde(default)]
    pub tencent_tmt: TencentTmtPreferences,
    #[serde(default)]
    pub niutrans: NiuTransPreferences,
    #[serde(default)]
    pub floating_toolbar: FloatingToolbarPreferences,
    #[serde(default)]
    pub character_width: CharacterWidthPreference,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub settings_theme: SettingsTheme,
    #[serde(default)]
    pub candidate_theme: SettingsTheme,
    #[serde(default)]
    pub toolbar_theme: SettingsTheme,
    #[serde(default)]
    pub screen_keyboard_theme: SettingsTheme,
    #[serde(default)]
    pub handwriting_theme: SettingsTheme,
    #[serde(default)]
    pub voice_theme: SettingsTheme,
    #[serde(default)]
    pub emoji_theme: SettingsTheme,
    /// The tray and candidate context menus. Windows draws its own, so this is
    /// the one surface override the client was missing.
    #[serde(default)]
    pub menu_theme: SettingsTheme,
    /// The one theme that colours the candidate window, toolbar, menus and touch keyboard on every host. `theme` above stays the light/dark mode the `system` theme and the settings window follow.
    #[serde(default)]
    pub global_theme: crate::skin::theme::GlobalTheme,
    #[serde(default)]
    pub custom_theme: CustomTheme,
    #[serde(default)]
    pub candidate_layout: CandidateLayout,
    #[serde(default)]
    pub candidate_preedit_style: CandidatePreeditStyle,
    /// Show the current/total page text in Linux candidate panels, independently of preedit.
    #[serde(default = "enabled_by_default")]
    pub show_candidate_page_number: bool,
    #[serde(default)]
    pub tsf_preedit_style: PreeditStyle,
    #[serde(default)]
    pub diagnostic_log: DiagnosticLogPreferences,
    #[serde(default = "enabled_by_default")]
    pub candidate_follow_cursor: bool,
    /// macOS displays a short, non-activating badge after switching between
    /// Chinese and English input. Other hosts preserve this preference but do
    /// not render the native badge.
    #[serde(default = "enabled_by_default")]
    pub input_mode_hud: bool,
    /// 候选窗和悬浮工具栏左端的水杉 logo。新装默认隐藏（见 `Default`）；已存文档缺这个字段时读成显示，升级沿用原来的样子。
    #[serde(default = "enabled_by_default")]
    pub show_app_logo: bool,
    pub scheme: InputScheme,
    /// Show the Wubi code suffix that remains after the typed prefix.
    #[serde(default = "enabled_by_default")]
    pub wubi_code_hint: bool,
    /// 完整四码且整个列表只有一个候选时，在第四键提交。
    ///
    /// *哪些*码是这样由 Engine 回答；词是否就在那里离开输入法由用户决定，关掉表示它留在候选列表里，等空格或数字键选走。
    #[serde(default = "enabled_by_default")]
    pub wubi_auto_commit_unique: bool,
    /// 五笔拼音混输：五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。
    #[serde(default)]
    pub wubi_mixed_pinyin: bool,
    /// 五笔用 86 还是 98 码表；个人词条和学习记录也按它分开存。
    #[serde(default)]
    pub wubi_profile: WubiProfile,
    #[serde(default)]
    pub touch_keyboard_layout: TouchKeyboardLayout,
    /// 触屏键盘列出哪些方案、选中哪一个；桌面宿主原样保留、不使用。
    ///
    /// 总是写进文档，哪怕等于默认值：以前的版本在等于默认值时省略它，于是文档里没有它只能说明是以前的版本写的，按以前的默认列表读（[`TouchKeyboardScheme::LEGACY_DEFAULT_ENABLED`]）。新装的默认值已经不同，再省略就分不出这两种文档。
    #[serde(default = "TouchKeyboardSchemePreferences::without_stored_list")]
    pub touch_keyboard_schemes: TouchKeyboardSchemePreferences,
    /// Horizontal key gap in tenths of a density-independent pixel.
    #[serde(default = "default_touch_key_spacing_tenths")]
    pub touch_key_spacing_tenths: u8,
    /// Vertical row gap in tenths of a density-independent pixel.
    #[serde(default = "default_touch_row_spacing_tenths")]
    pub touch_row_spacing_tenths: u8,
    /// Touch-keyboard height adjustment in density-independent pixels.
    #[serde(default)]
    pub touch_keyboard_height_adjustment: i8,
    /// Show a direct voice-result entry in touch-keyboard toolbars.
    #[serde(default)]
    pub touch_voice_shortcut: bool,
    /// 九宫格数字层按电话还是计算器排列；字母层不受影响。
    #[serde(default)]
    pub touch_number_keypad_order: NumberKeypadOrder,
    /// The optional buttons on the touch keyboard's toolbar, the counterpart of the floating toolbar's component switches. The voice entry stays under `touch_voice_shortcut`.
    #[serde(default)]
    pub touch_toolbar: TouchToolbarPreferences,
    /// 当前方案是日文、韩文、越南文或藏文时保留，记住要回到的中文方案。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_chinese_scheme: Option<ChineseScheme>,
    #[serde(default)]
    pub shuangpin_profile: ShuangpinProfile,
    #[serde(default = "enabled_by_default")]
    pub shuangpin_preedit_uses_raw: bool,
    /// The Vietnamese input method and tone placement.
    #[serde(default)]
    pub vietnamese: VietnamesePreferences,
    /// 只出单字：全拼、双拼、五笔和粤拼的候选只留单个汉字，选一个字后剩下的拼写接着组字。默认关闭。
    #[serde(default)]
    pub single_character_only: bool,
    pub candidate_page_size: u8,
    /// Linux IBus can release the number row to the application while a
    /// candidate list is visible. Other hosts preserve this preference even
    /// when their native candidate presenter does not expose the switch.
    #[serde(default = "enabled_by_default")]
    pub number_row_selection: bool,
    #[serde(default = "default_candidate_font_size")]
    pub candidate_font_size: u8,
    #[serde(default = "default_candidate_preedit_font_size")]
    pub candidate_preedit_font_size: u8,
    /// Overall size of the floating candidate window, 50-200 percent. The host multiplies `candidate_font_size` and every piece of window geometry (paddings, row heights, header, arrows, insets, shadow) by it.
    #[serde(default = "default_candidate_scale_percent")]
    pub candidate_scale_percent: u16,
    /// Opacity of the candidate card, 50-100 percent. It multiplies only the alpha of the card fill, its border and the skin background image; text, numbers and the selection highlight stay opaque.
    #[serde(default = "default_candidate_opacity_percent")]
    pub candidate_opacity_percent: u8,
    /// Corner radius of the candidate card in points (DIP on Windows), 0-32. It wins over the skin package's `corner_radius_dip`, which wins over the host's own constant; absent means the host or skin decides. Row and selection radii become the smaller of the host's row radius and this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_corner_radius: Option<u8>,
    #[serde(default = "default_candidate_font_family")]
    pub candidate_font_family: String,
    /// Optional leading face for the Windows candidate glyph fallback chain.
    /// The host supplies its default; absent values preserve other hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_english_font: Option<String>,
    #[serde(default = "default_candidate_fallback_fonts")]
    pub candidate_fallback_fonts: Vec<String>,
    pub learning: bool,
    #[serde(default)]
    pub quanpin: QuanpinPreferences,
    #[serde(default)]
    pub fuzzy_pinyin: FuzzyPinyinPreferences,
    #[serde(default = "default_quanpin_helpcode")]
    pub quanpin_helpcode: HelpcodePreferences,
    #[serde(default = "default_shuangpin_helpcode")]
    pub shuangpin_helpcode: HelpcodePreferences,
    /// Render and commit Chinese Engine output in Traditional Chinese at the host boundary.
    #[serde(default)]
    pub traditional_chinese_output: bool,
    pub chinese_punctuation: bool,
    #[serde(default = "smart_punctuation_default")]
    pub smart_punctuation: bool,
    #[serde(default = "smart_punctuation_default")]
    pub smart_punctuation_repeat: bool,
    /// Space after a just-committed Chinese punctuation rewrites it as ASCII. Off by default on every host, like the rest of the family in the source: it changes a character the user already saw land.
    #[serde(default)]
    pub smart_punctuation_space_convert: bool,
    /// Keep `,` `.` `:` as ASCII when they follow a digit.
    #[serde(default = "smart_punctuation_default")]
    pub smart_punctuation_direct_digit: bool,
    /// The same after a letter. Two switches rather than one, because a
    /// version number and an English sentence want different answers.
    ///
    /// Both follow the parent switch's default rather than being off on their
    /// own. The reference has one switch here, and its description - which this
    /// page shows verbatim - promises ASCII after a letter or a digit. Split
    /// into three and with the two halves off, that switch was on out of the
    /// box and did nothing: the sentence under it was false until the user
    /// found two more toggles. A document that already carries the keys is
    /// unaffected, since this answers only for one that does not.
    #[serde(default = "smart_punctuation_default")]
    pub smart_punctuation_direct_letter: bool,
    #[serde(default = "enabled_by_default")]
    pub paired_punctuation: bool,
    #[serde(default)]
    pub punctuation_lock: PunctuationLock,
    #[serde(default)]
    pub navigation: NavigationPreferences,
    #[serde(default)]
    pub keybindings: KeybindingPreferences,
    #[serde(default)]
    pub word_character: WordCharacterPreferences,
    #[serde(default)]
    pub frequency: FrequencyPreferences,
    #[serde(default)]
    pub mixed_input: MixedInputPreferences,
    #[serde(default)]
    pub local_modes: LocalModePreferences,
    /// Sound packs, background music, achievements and the enabled command tables. Left out of the document while every part is at its default, so a build from before plugins still reads a document that never touched them; the packs themselves and the @ name list live under the plugins directory, not here.
    #[serde(default, skip_serializing_if = "PluginPreferences::is_default")]
    pub plugins: PluginPreferences,
    /// Windows 游戏里由水杉显示候选窗的开关和进程表，只有 Windows 的 TSF DLL 读取。等于默认值时不写进文档，没有这个键的旧版本照样能读。
    #[serde(
        default,
        skip_serializing_if = "GameCompatibilityPreferences::is_default"
    )]
    pub game_compatibility: GameCompatibilityPreferences,
    #[serde(default)]
    pub clipboard_history: bool,
    /// 向云候选服务多要一条候选。它会把正在组的拼写发给 `https://inputtools.google.com`，所以新装默认关闭（见 `Default`）；已存文档缺这个字段时读成开启，升级沿用原来的行为。
    #[serde(default = "enabled_by_default")]
    pub cloud_candidates: bool,
    #[serde(default = "enabled_by_default")]
    pub candidate_translations: bool,
    /// Show bounded offline English glosses from the packaged Engine dictionary.
    #[serde(default)]
    pub candidate_english_gloss: bool,
    /// Show how a candidate's English and Japanese gloss lines are read: IPA from the offline
    /// pronunciation table and romaji where the host can produce it. Display only; a committed
    /// gloss column never includes it.
    #[serde(default)]
    pub candidate_pronunciation: bool,
    /// Show read-only English word completions in direct English touch input.
    /// Hosts without a direct English suggestion surface preserve this value.
    #[serde(default = "enabled_by_default")]
    pub english_suggestions: bool,
    #[serde(default)]
    pub translation_target_language: TranslationTargetLanguage,
    /// Optional second language for mobile candidate glosses. `None` shows a single language and is omitted from serialized snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_secondary_language: Option<TranslationTargetLanguage>,
    /// 候选翻译服务选的是水杉账号时为真，候选词会发到 `https://api.msime.app/v1/translate`。新装和缺字段时都是关闭，必须由用户显式选择。
    #[serde(default)]
    pub translation_account: bool,
    /// Send anonymous usage reports (daily activity, session ends, crash summaries; see [`crate::telemetry`] and PRIVACY.md) to `https://api.msime.app/v1/telemetry/events`. On by default; turning it off stops all reporting and clears the local queue. A reader treats an absent key as on.
    #[serde(default = "enabled_by_default")]
    pub usage_reporting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceInputPreferences {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default = "enabled_by_default")]
    pub sound_enabled: bool,
    #[serde(default = "enabled_by_default")]
    pub start_sound: bool,
    #[serde(default = "enabled_by_default")]
    pub end_sound: bool,
    #[serde(default = "source_voice_default")]
    pub mute_system_audio: bool,
    #[serde(default)]
    pub language: String,
    /// Empty values inherit the user-managed Linux recording service defaults.
    #[serde(default)]
    pub capture_backend: String,
    #[serde(default)]
    pub capture_device: String,
    #[serde(default = "default_commit_mode")]
    pub commit_mode: String,
    #[serde(default)]
    pub asr_provider: String,
    #[serde(default)]
    pub asr_app_key: String,
    /// Doubao authentication mode (`api_key` or `legacy`, the two console types Doubao offers). Empty means `api_key`.
    #[serde(default = "default_doubao_auth_mode")]
    pub doubao_auth_mode: String,
    #[serde(default)]
    pub asr_token: String,
    /// One recognition token per provider id.
    ///
    /// A single flat token meant switching provider left the previous
    /// provider's key in the box, so it was sent to the new endpoint until the
    /// user noticed, and the old key was gone the moment they retyped.
    #[serde(default)]
    pub asr_tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub asr_endpoint: String,
    #[serde(default)]
    pub asr_model: String,
    /// Absolute path to the installed model directory the `local` provider runs (one containing `msime-model.json`, see `voice::local_models`). Nothing is uploaded and no endpoint or token applies. Only the path's shape is checked here, since the same document is read on every OS: any absolute form the host OS uses is accepted, and the recognizer finds its files only through `msime-model.json`, so a path without one is a missing model rather than an invalid document.
    #[serde(default)]
    pub asr_model_path: String,
    /// Optional `https://` prefix placed in front of every local model download URL (ghproxy-style), for networks where GitHub release downloads are slow or blocked. Empty downloads from the catalog URLs as they are.
    #[serde(default)]
    pub asr_model_mirror: String,
    #[serde(default)]
    pub asr_resource_id: String,
    #[serde(default)]
    pub polish_enabled: bool,
    #[serde(default = "source_voice_default")]
    pub polish_text: bool,
    #[serde(default)]
    pub polish_provider: String,
    #[serde(default)]
    pub polish_token: String,
    /// One polish token per provider id, for the same reason as `asr_tokens`.
    #[serde(default)]
    pub polish_tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub polish_endpoint: String,
    #[serde(default)]
    pub polish_model: String,
    #[serde(default)]
    pub polish_prompt_id: String,
    /// Show streaming ASR updates in the host preedit while recording.
    #[serde(default = "enabled_by_default")]
    pub stream_inline_preedit: bool,
    #[serde(default)]
    pub polish_prompt_custom_1: String,
    #[serde(default)]
    pub polish_prompt_custom_2: String,
    #[serde(default)]
    pub polish_prompt_custom_3: String,
    #[serde(default = "enabled_by_default")]
    pub hotkey_ralt: bool,
    #[serde(default)]
    pub hotkey_ctrl_win: bool,
    #[serde(default)]
    pub hotkey_rctrl_ralt: bool,
    #[serde(default = "enabled_by_default")]
    pub hotkey_hold_space_lock: bool,
    #[serde(default = "enabled_by_default")]
    pub hotkey_ctrl_f9: bool,
    #[serde(default = "enabled_by_default")]
    pub doubao_enable_itn: bool,
    #[serde(default = "enabled_by_default")]
    pub doubao_enable_punc: bool,
    #[serde(default = "source_voice_default")]
    pub doubao_enable_ddc: bool,
    #[serde(default)]
    pub doubao_boosting_table_id: String,
}

impl Default for VoiceInputPreferences {
    fn default() -> Self {
        let polish = default_polish_service();
        Self {
            enabled: true,
            sound_enabled: true,
            start_sound: true,
            end_sound: true,
            mute_system_audio: source_voice_default(),
            language: "zh-cn".into(),
            capture_backend: String::new(),
            capture_device: String::new(),
            commit_mode: "tsf".into(),
            asr_provider: "doubao".into(),
            asr_app_key: String::new(),
            doubao_auth_mode: default_doubao_auth_mode(),
            asr_token: String::new(),
            asr_tokens: BTreeMap::new(),
            asr_endpoint: "wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_async".into(),
            asr_model: String::new(),
            asr_model_path: String::new(),
            asr_model_mirror: String::new(),
            asr_resource_id: "volc.seedasr.sauc.duration".into(),
            polish_enabled: false,
            polish_text: source_voice_default(),
            polish_provider: polish.provider.into(),
            polish_token: String::new(),
            polish_tokens: BTreeMap::new(),
            polish_endpoint: polish.endpoint.into(),
            polish_model: polish.model.into(),
            polish_prompt_id: "cleanup".into(),
            stream_inline_preedit: true,
            polish_prompt_custom_1: String::new(),
            polish_prompt_custom_2: String::new(),
            polish_prompt_custom_3: String::new(),
            hotkey_ralt: true,
            hotkey_ctrl_win: false,
            hotkey_rctrl_ralt: false,
            hotkey_hold_space_lock: true,
            hotkey_ctrl_f9: true,
            doubao_enable_itn: true,
            doubao_enable_punc: true,
            doubao_enable_ddc: source_voice_default(),
            doubao_boosting_table_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiAssistantPreferences {
    #[serde(default = "source_ai_default")]
    pub enabled: bool,
    /// A missing key falls back to the same provider as a missing section, so
    /// a hand-edited `{"enabled": true}` still loads.
    #[serde(default = "default_ai_provider")]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub tokens: BTreeMap<String, String>,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default = "default_ai_candidate_limit")]
    pub candidate_limit: u8,
    #[serde(default)]
    pub prompt_id: String,
    #[serde(default)]
    pub prompt_custom_1: String,
    #[serde(default)]
    pub prompt_custom_2: String,
    #[serde(default)]
    pub prompt_custom_3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CustomTranslationPreferences {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TencentTmtPreferences {
    pub enabled: bool,
    pub secret_id: String,
    pub secret_key: String,
    pub region: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NiuTransPreferences {
    pub enabled: bool,
    pub app_id: String,
    pub apikey: String,
}

impl Default for TencentTmtPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            secret_id: String::new(),
            secret_key: String::new(),
            region: "ap-guangzhou".into(),
        }
    }
}

fn default_ai_candidate_limit() -> u8 {
    3
}

/// Kept equal to `AiAssistantPreferences::default().provider`.
fn default_ai_provider() -> String {
    "deepseek".into()
}

impl Default for AiAssistantPreferences {
    fn default() -> Self {
        let (endpoint, model) = if source_ai_default() {
            (SOURCE_DEEPSEEK_ENDPOINT, SOURCE_DEEPSEEK_MODEL)
        } else {
            ("", "")
        };
        Self {
            enabled: source_ai_default(),
            provider: "deepseek".into(),
            model: model.into(),
            token: String::new(),
            tokens: BTreeMap::new(),
            endpoint: endpoint.into(),
            candidate_limit: 3,
            prompt_id: "custom_1".into(),
            prompt_custom_1: String::new(),
            prompt_custom_2: String::new(),
            prompt_custom_3: String::new(),
        }
    }
}

/// Diagnostic logging, off unless a user turns it on while reproducing a
/// problem. The two hosts log separately because they are separate processes.
/// Neither records keystrokes, input text or candidates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiagnosticLogPreferences {
    /// Server-side timing and window state: slow request stages, candidate
    /// window, floating toolbar, menus, focus sessions and transport status.
    pub server: bool,
    /// In-process TSF preedit and input latency, buffered and batched out.
    pub tsf: bool,
}

/// Windows 游戏兼容：游戏声明自己画候选却不画时，由水杉的候选窗显示。DLL 在每次激活时读取，对已打开的游戏要重新激活输入法才生效。各项都等于默认值时不写进文档，没有这个键的旧版本照样能读。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameCompatibilityPreferences {
    /// 总开关。关闭时任何进程都不强制使用水杉的候选窗，两张表也不起作用。
    pub candidate_overlay: bool,
    /// 总是由水杉显示候选窗的进程基名（如 `game.exe`），优先于内置规则。
    pub overlay_processes: Vec<String>,
    /// 从不由水杉显示候选窗的进程基名，优先于 `overlay_processes` 和内置规则。
    pub excluded_processes: Vec<String>,
}

impl Default for GameCompatibilityPreferences {
    fn default() -> Self {
        Self {
            candidate_overlay: true,
            overlay_processes: Vec::new(),
            excluded_processes: Vec::new(),
        }
    }
}

impl GameCompatibilityPreferences {
    /// 两张表合计的条目上限，限制这两张表给偏好文档增加的体积：配合 `MAX_PROCESS_NAME_CHARS`，最坏情况（每项 64 个四字节 UTF-8 字符）约 8KB，ASCII 程序名约 2KB。整份偏好能否留在 Windows Server 偏好快照的 16KiB 上限以内（`PreferenceSnapshot.h`）还取决于其他字段，这里不做保证。
    pub const MAX_PROCESSES: usize = 32;
    /// 单个进程名的字符数上限。
    pub const MAX_PROCESS_NAME_CHARS: usize = 64;

    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// 每项都是带 `.exe` 后缀（ASCII 不分大小写）的进程基名，不含路径分隔符、通配符和控制字符；两张表合在一起按 ASCII 不分大小写地不重复（和 WinUI 设置页 `GameProcessList.h` 的检查一致），合计不超过 `MAX_PROCESSES` 条。DLL 不分大小写地比较进程名，所以只差大小写的两项会让同一个进程同时落在两张表里。
    fn validate(&self) -> bool {
        let mut seen = BTreeSet::new();
        self.overlay_processes.len() + self.excluded_processes.len() <= Self::MAX_PROCESSES
            && self
                .overlay_processes
                .iter()
                .chain(&self.excluded_processes)
                .all(|name| valid_process_name(name) && seen.insert(name.to_ascii_lowercase()))
    }
}

fn valid_process_name(name: &str) -> bool {
    name.chars().count() <= GameCompatibilityPreferences::MAX_PROCESS_NAME_CHARS
        && !name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
        && name.len() >= 4
        && name.as_bytes()[name.len() - 4..].eq_ignore_ascii_case(b".exe")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloatingToolbarPreferences {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default = "enabled_by_default")]
    pub english_mode: bool,
    /// 切换输入方案的按钮：点开列出全拼、双拼、五笔、粤拼、注音等方案。默认开启——在 macOS 27 上粤、注这类菜单栏入口只能由用户自己去系统设置里添加，这个按钮让不加入口也能切换。目前只有 macOS 的工具栏画它（见 `HostCapabilities::floating_toolbar_input_scheme`）。
    #[serde(default = "enabled_by_default")]
    pub input_scheme: bool,
    #[serde(default = "default_toolbar_scale")]
    pub scale_percent: u16,
    #[serde(default = "default_toolbar_font_size")]
    pub font_size: u16,
    #[serde(default = "enabled_by_default")]
    pub fullwidth: bool,
    #[serde(default = "enabled_by_default")]
    pub punctuation: bool,
    #[serde(default = "enabled_by_default")]
    pub character_set: bool,
    /// Off by default, with `handwriting` and `voice`, so the toolbar a new profile gets is the
    /// compact one. This is a deliberate reversal: these three defaulted on because they had been on
    /// the toolbar since it shipped, and the switches appearing was not allowed to remove them. A
    /// profile that never touched the switches therefore loses these buttons and turns back on the
    /// ones it wants, which is the cost that was chosen over carrying the wider toolbar forever.
    /// Windows is unaffected: its installer template sets every component explicitly, mirroring the
    /// reference's own default config.
    #[serde(default)]
    pub emoji: bool,
    /// The handwriting panel button. The reference's toolbar has no such button; this client's
    /// macOS toolbar carries one, and it is opt-in for the reason above.
    #[serde(default)]
    pub handwriting: bool,
    #[serde(default)]
    pub screen_keyboard: bool,
    /// The voice input button, for the same reason as `handwriting`.
    #[serde(default)]
    pub voice: bool,
    #[serde(default = "enabled_by_default")]
    pub settings: bool,
}

fn default_toolbar_scale() -> u16 {
    100
}
fn default_toolbar_font_size() -> u16 {
    24
}

/// Which optional buttons the touch keyboard's toolbar carries. The first three are the buttons the bar always had; the rest are tools that otherwise sit one tap deeper, in the keyboard's 更多 panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TouchToolbarPreferences {
    pub layout: bool,
    pub emoji: bool,
    pub skin: bool,
    pub clipboard: bool,
    pub ai: bool,
    pub character_set: bool,
    pub fullwidth: bool,
    pub punctuation: bool,
}

impl Default for TouchToolbarPreferences {
    fn default() -> Self {
        Self {
            layout: true,
            emoji: true,
            skin: true,
            clipboard: false,
            ai: false,
            character_set: false,
            fullwidth: false,
            punctuation: false,
        }
    }
}

impl Default for FloatingToolbarPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            english_mode: true,
            input_scheme: true,
            scale_percent: 100,
            font_size: 24,
            fullwidth: true,
            punctuation: true,
            character_set: true,
            emoji: false,
            handwriting: false,
            screen_keyboard: false,
            voice: false,
            settings: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Dark,
    Light,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SettingsTheme {
    #[default]
    Follow,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidateLayout {
    Horizontal,
    #[default]
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CandidatePreeditStyle {
    #[default]
    Pinyin,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PreeditStyle {
    #[default]
    Raw,
    Pinyin,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalModePreferences {
    pub unicode: bool,
    pub date_time: bool,
    pub quick_phrase: bool,
    pub emoji: bool,
    pub kaomoji: bool,
    pub super_jianpin: bool,
    pub temporary_english: bool,
    pub temporary_japanese: bool,
    /// `V` on an empty composition: calculator, Chinese numerals and dates. Off by default, because Shift+V otherwise types a capital V.
    #[serde(default)]
    pub expression: bool,
    /// `/` on an empty composition: built-in and installed commands. Off by default, because `/` otherwise types a mark.
    #[serde(default)]
    pub command: bool,
    /// `@` on an empty composition: the local mention list. Off by default, because `@` otherwise types itself.
    #[serde(default)]
    pub mention: bool,
    /// The `@` mode also offers China's provinces, cities and counties from the Engine's built-in table, after the user's own names. Off by default, and only meaningful while `mention` is on.
    #[serde(default)]
    pub mention_places: bool,
}

impl Default for LocalModePreferences {
    fn default() -> Self {
        Self {
            unicode: true,
            date_time: true,
            quick_phrase: true,
            emoji: true,
            kaomoji: true,
            super_jianpin: true,
            temporary_english: true,
            temporary_japanese: true,
            expression: false,
            command: false,
            mention: false,
            mention_places: false,
        }
    }
}

/// What the plugin packs do: which pack sounds and how loud, and which command tables the `/` mode reads. Everything is off in a fresh profile. Pack ids name a built-in pack or one installed under the plugins directory; a host that cannot find the named pack stays silent rather than falling back to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PluginPreferences {
    pub key_sound: KeySoundPreferences,
    pub commit_sound: CommitSoundPreferences,
    pub melody: MelodyPreferences,
    pub music: MusicPreferences,
    pub achievements: AchievementPreferences,
    /// Installed command-table packs the `/` mode reads, in priority order: the first pack that defines a trigger wins.
    pub command_tables: Vec<String>,
    /// The typing effect a host draws on keys and commits; `Off` draws nothing.
    pub effect_style: crate::plugins::EffectStyle,
    /// 0-100: how large and how long the effect is drawn. Only the host reads it.
    pub effect_intensity: u8,
    /// An installed effect pack whose style and parameters replace `effect_style` and `effect_intensity`; empty for none, which leaves those two in force. A selected pack that cannot be loaded draws no effect rather than falling back (`plugins::effect_pack::TypingEffect::resolve`). Left out of the document while empty, so a build from before effect packs still reads a document that never selected one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub effect_pack: String,
    /// Count consecutive keys and show the count; a pause of `plugins::COMBO_IDLE_RESET_MILLIS` or a backspace starts it again.
    pub combo_counter: bool,
    /// Play the key sound pack's commit sample, pitched up, when the count reaches one of `plugins::COMBO_MILESTONES`.
    pub combo_tier_sound: bool,
    /// K 模式读取的已安装短语表包，按优先级排列，最多 `MAX_PHRASE_TABLES` 个。为空时不写进文档，没有这个键的旧版本照样能读。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phrase_tables: Vec<String>,
    /// 全拼方案选用的已安装辅助码表包；为空表示沿用 `quanpin_helpcode.schema`；包载入失败时也回退到那个方案。为空时不写进文档。
    #[serde(skip_serializing_if = "String::is_empty")]
    pub helpcode_pack_quanpin: String,
    /// 双拼方案选用的已安装辅助码表包，规则同 `helpcode_pack_quanpin`。
    #[serde(skip_serializing_if = "String::is_empty")]
    pub helpcode_pack_shuangpin: String,
}

impl Default for PluginPreferences {
    fn default() -> Self {
        Self {
            key_sound: KeySoundPreferences::default(),
            commit_sound: CommitSoundPreferences::default(),
            melody: MelodyPreferences::default(),
            music: MusicPreferences::default(),
            achievements: AchievementPreferences::default(),
            command_tables: Vec::new(),
            effect_style: crate::plugins::EffectStyle::Off,
            effect_intensity: 50,
            effect_pack: String::new(),
            combo_counter: false,
            combo_tier_sound: false,
            phrase_tables: Vec::new(),
            helpcode_pack_quanpin: String::new(),
            helpcode_pack_shuangpin: String::new(),
        }
    }
}

impl PluginPreferences {
    /// Most command tables enabled at once.
    pub const MAX_COMMAND_TABLES: usize = 16;
    /// 同时启用的短语表包上限。
    pub const MAX_PHRASE_TABLES: usize = 16;

    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    fn validate(&self) -> bool {
        let pack = |id: &str| id.is_empty() || crate::skin::catalog::safe_id(id);
        pack(&self.key_sound.pack)
            && pack(&self.melody.pack)
            && pack(&self.music.pack)
            && pack(&self.effect_pack)
            && self.key_sound.volume <= 100
            && self.music.volume <= 100
            && self.effect_intensity <= 100
            && self.command_tables.len() <= Self::MAX_COMMAND_TABLES
            && self.command_tables.iter().enumerate().all(|(index, id)| {
                crate::skin::catalog::safe_id(id) && !self.command_tables[..index].contains(id)
            })
            && pack(&self.helpcode_pack_quanpin)
            && pack(&self.helpcode_pack_shuangpin)
            && self.phrase_tables.len() <= Self::MAX_PHRASE_TABLES
            && self.phrase_tables.iter().enumerate().all(|(index, id)| {
                crate::skin::catalog::safe_id(id) && !self.phrase_tables[..index].contains(id)
            })
    }
}

/// What a key sounds like.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySoundMode {
    /// The sound pack's sample for the key's class.
    #[default]
    Keys,
    /// The next note of the melody pack.
    Melody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeySoundPreferences {
    pub enabled: bool,
    pub mode: KeySoundMode,
    /// The sound pack keys, commits and achievements are played from.
    pub pack: String,
    /// 0-100, for every effect sound: keys, the melody, commits and achievements.
    pub volume: u8,
}

impl Default for KeySoundPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: KeySoundMode::Keys,
            pack: crate::plugins::DEFAULT_SOUND_PACK.to_owned(),
            volume: 50,
        }
    }
}

/// A sound when text is committed, from the key sound pack's `commit` sample. Independent of the key sound switch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CommitSoundPreferences {
    pub enabled: bool,
}

/// The sequence pack a key plays a note of when `key_sound.mode` is `melody`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MelodyPreferences {
    pub pack: String,
}

impl Default for MelodyPreferences {
    fn default() -> Self {
        Self {
            pack: crate::plugins::DEFAULT_MELODY_PACK.to_owned(),
        }
    }
}

/// Background music, streamed from an installed music pack while the input method is active. Off, and with no pack chosen, until the user picks one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MusicPreferences {
    pub enabled: bool,
    pub pack: String,
    /// 0-100.
    pub volume: u8,
}

impl Default for MusicPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            pack: String::new(),
            volume: 30,
        }
    }
}

/// A short jingle, the key sound pack's `achievement` sample, when the commit count passes one of `plugins::ACHIEVEMENT_MILESTONES`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AchievementPreferences {
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MixedInputPreferences {
    pub english: bool,
    pub minimum_prefix: u8,
    pub emoji: bool,
    pub kaomoji: bool,
}

impl Default for MixedInputPreferences {
    fn default() -> Self {
        Self {
            english: true,
            minimum_prefix: 5,
            emoji: source_mixed_emoji_default(),
            kaomoji: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FrequencyMode {
    Disabled,
    Pin,
    Halve,
    Linear,
    #[default]
    Promote,
}

impl FrequencyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Pin => "pin",
            Self::Halve => "halve",
            Self::Linear => "linear",
            Self::Promote => "promote",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrequencyPreferences {
    pub mode: FrequencyMode,
    pub trigger_count: u8,
    pub linear_step: u8,
}

impl Default for FrequencyPreferences {
    fn default() -> Self {
        Self {
            mode: FrequencyMode::Promote,
            trigger_count: 1,
            linear_step: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WordCharacterKeys {
    #[default]
    Brackets,
    MinusEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordCharacterPreferences {
    pub enabled: bool,
    pub keys: WordCharacterKeys,
}

impl Default for WordCharacterPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            keys: WordCharacterKeys::Brackets,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavigationPreferences {
    pub minus_equal: bool,
    pub comma_period: bool,
    pub brackets: bool,
    pub tab: bool,
    pub page_up_down: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub mouse_wheel: bool,
    #[serde(alias = "candidate_arrow_navigation")]
    pub arrows: bool,
}

impl Default for NavigationPreferences {
    fn default() -> Self {
        Self {
            minus_equal: true,
            comma_period: true,
            brackets: false,
            tab: true,
            page_up_down: true,
            mouse_wheel: false,
            arrows: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeybindingPreferences {
    #[serde(default = "enabled_by_default")]
    pub switch_language_shift: bool,
    #[serde(default)]
    pub switch_language_ctrl: bool,
    /// Linux 已默认处理这个组合键；关闭时交还应用，不改输入法框架的全局绑定。
    #[serde(default = "enabled_by_default")]
    pub switch_language_ctrl_space: bool,
    #[serde(default = "enabled_by_default")]
    pub switch_language_ctrl_alt_space: bool,
    #[serde(default = "enabled_by_default")]
    pub toggle_character_set_ctrl_shift_f: bool,
    /// The macOS Option+Shift+H chord. The host has reserved it unconditionally since it shipped,
    /// so this defaults on: the preference gives the chord back to the application, it does not
    /// turn on something that was off. Other hosts have no such chord and ignore it.
    #[serde(default = "enabled_by_default")]
    pub toggle_fullwidth_option_shift_h: bool,
}

impl Default for KeybindingPreferences {
    fn default() -> Self {
        Self {
            switch_language_shift: true,
            switch_language_ctrl: false,
            switch_language_ctrl_space: true,
            switch_language_ctrl_alt_space: true,
            toggle_character_set_ctrl_shift_f: true,
            toggle_fullwidth_option_shift_h: true,
        }
    }
}

fn enabled_by_default() -> bool {
    true
}

/// Smart punctuation is off on a fresh Windows or macOS profile.
///
/// It rewrites a character the user already saw land, so the source ships the whole family disabled and asks for it to be turned on deliberately - `platforms/windows/installer/config.default.toml` has every one of the five switches `false`. That file is only the installed template; the running host reads this document, so without this the effective first-run default would be the opposite of the baseline the source ships. macOS is the port of that desktop product and follows it.
///
/// Linux, Android, iOS and HarmonyOS keep what they have shipped, since a preference that changes under existing users is worse than one that differs by platform. A stored value is never reinterpreted either way; this answers only for a document that does not have the key yet.
fn smart_punctuation_default() -> bool {
    !cfg!(any(windows, target_os = "macos"))
}

/// Three voice switches the source ships on and the shared document had off: muting other audio while recording, Doubao's semantic smoothing (DDC), and polishing the recognized text.
///
/// `platforms/windows/installer/config.default.toml` has all three `true`, matching the source's factory configuration, but like smart punctuation that file is only the installed template - the running host reads this document - so the effective first-run value on Windows was `false`. macOS is the port of that desktop product and follows it. None of the three depends on the platform: muting uses CoreAudio on macOS, DDC is a Doubao request flag, and polishing still needs a polish token before anything is sent.
///
/// The other hosts keep what they have shipped. A stored value is untouched either way; this answers only for a document that does not have the key yet.
fn source_voice_default() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

/// The source's factory template turns the AI assistant on and points it at DeepSeek (`deepseek-v4-flash`); `platforms/windows/installer/config.default.toml` ships the same, but the running host reads this document, so the effective first-run value on Windows was off with no endpoint or model. macOS follows the desktop product it ports. Being on without a token sends nothing: `chat_completion_http_request` refuses to build a request until a usable key is set.
///
/// The other hosts keep the assistant off with an empty endpoint and model. A stored value is untouched either way; this answers only for a document that does not have the key yet.
fn source_ai_default() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

const SOURCE_DEEPSEEK_ENDPOINT: &str = "https://api.deepseek.com/chat/completions";
const SOURCE_DEEPSEEK_MODEL: &str = "deepseek-v4-flash";

/// A polish provider with the endpoint and model that belong to it, kept together so a default never pairs one provider's URL with another's model.
struct PolishService {
    provider: &'static str,
    endpoint: &'static str,
    model: &'static str,
}

/// First-run polish service. The source template and the Windows installer template both ship DeepSeek (`deepseek-v4-flash`), and macOS follows the desktop product it ports; the other hosts keep SiliconFlow with `Qwen/Qwen3-8B`, which is what they have shipped. Stored values are never reinterpreted.
fn default_polish_service() -> PolishService {
    if source_voice_default() {
        PolishService {
            provider: "deepseek",
            endpoint: SOURCE_DEEPSEEK_ENDPOINT,
            model: SOURCE_DEEPSEEK_MODEL,
        }
    } else {
        PolishService {
            provider: "siliconflow",
            endpoint: "https://api.siliconflow.cn/v1/chat/completions",
            model: "Qwen/Qwen3-8B",
        }
    }
}

/// The source's `config.default.toml` ships `emoji_mixed_input = true`; Windows follows it, macOS follows the desktop product it ports, the other hosts keep `false`, and a stored value is untouched either way.
fn source_mixed_emoji_default() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

fn default_candidate_font_size() -> u8 {
    18
}

fn default_candidate_preedit_font_size() -> u8 {
    15
}

fn default_candidate_scale_percent() -> u16 {
    100
}

fn default_candidate_opacity_percent() -> u8 {
    100
}

fn default_touch_key_spacing_tenths() -> u8 {
    60
}

fn default_touch_row_spacing_tenths() -> u8 {
    70
}

fn default_candidate_font_family() -> String {
    "Noto Sans SC".to_owned()
}
fn default_candidate_fallback_fonts() -> Vec<String> {
    vec!["Noto Sans SC".to_owned(), "Microsoft YaHei".to_owned()]
}

fn default_commit_mode() -> String {
    "tsf".to_owned()
}

fn default_doubao_auth_mode() -> String {
    "api_key".to_owned()
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            default_ime_mode: DefaultImeMode::default(),
            ime_mode_scope: ImeModeScope::default(),
            ai_assistant: AiAssistantPreferences::default(),
            sentence_association: SentenceAssociationPreferences::default(),
            custom_translation: CustomTranslationPreferences::default(),
            tencent_tmt: TencentTmtPreferences::default(),
            niutrans: NiuTransPreferences::default(),
            voice_input: VoiceInputPreferences::default(),
            floating_toolbar: FloatingToolbarPreferences::default(),
            character_width: CharacterWidthPreference::default(),
            theme: ThemeMode::default(),
            settings_theme: SettingsTheme::default(),
            candidate_theme: SettingsTheme::default(),
            toolbar_theme: SettingsTheme::default(),
            screen_keyboard_theme: SettingsTheme::default(),
            handwriting_theme: SettingsTheme::default(),
            voice_theme: SettingsTheme::default(),
            emoji_theme: SettingsTheme::default(),
            menu_theme: SettingsTheme::default(),
            // 新装在所有平台都跟随系统。触屏键盘的跟随系统皮肤按应用主题当前季节取色，键盘与应用界面是同一套颜色；原先触屏默认的薄荷晨光是固定配色，秋冬季节和应用主题对不上。
            global_theme: crate::skin::theme::GlobalTheme::default(),
            custom_theme: CustomTheme::default(),
            candidate_layout: CandidateLayout::default(),
            candidate_preedit_style: CandidatePreeditStyle::default(),
            show_candidate_page_number: true,
            tsf_preedit_style: PreeditStyle::default(),
            diagnostic_log: DiagnosticLogPreferences::default(),
            candidate_follow_cursor: true,
            input_mode_hud: true,
            show_app_logo: false,
            scheme: InputScheme::default(),
            wubi_code_hint: true,
            wubi_auto_commit_unique: true,
            wubi_mixed_pinyin: false,
            wubi_profile: WubiProfile::default(),
            touch_keyboard_layout: TouchKeyboardLayout::default(),
            touch_keyboard_schemes: TouchKeyboardSchemePreferences::default(),
            touch_key_spacing_tenths: default_touch_key_spacing_tenths(),
            touch_row_spacing_tenths: default_touch_row_spacing_tenths(),
            touch_keyboard_height_adjustment: 0,
            touch_voice_shortcut: false,
            touch_number_keypad_order: NumberKeypadOrder::default(),
            touch_toolbar: TouchToolbarPreferences::default(),
            last_chinese_scheme: None,
            shuangpin_profile: ShuangpinProfile::default(),
            shuangpin_preedit_uses_raw: true,
            vietnamese: VietnamesePreferences::default(),
            single_character_only: false,
            candidate_page_size: 6,
            number_row_selection: true,
            candidate_font_size: default_candidate_font_size(),
            candidate_preedit_font_size: default_candidate_preedit_font_size(),
            candidate_scale_percent: default_candidate_scale_percent(),
            candidate_opacity_percent: default_candidate_opacity_percent(),
            candidate_corner_radius: None,
            candidate_font_family: default_candidate_font_family(),
            candidate_english_font: None,
            candidate_fallback_fonts: default_candidate_fallback_fonts(),
            learning: true,
            quanpin: QuanpinPreferences::default(),
            fuzzy_pinyin: FuzzyPinyinPreferences::default(),
            quanpin_helpcode: default_quanpin_helpcode(),
            shuangpin_helpcode: default_shuangpin_helpcode(),
            traditional_chinese_output: false,
            chinese_punctuation: true,
            smart_punctuation: smart_punctuation_default(),
            smart_punctuation_repeat: smart_punctuation_default(),
            smart_punctuation_space_convert: false,
            smart_punctuation_direct_digit: smart_punctuation_default(),
            smart_punctuation_direct_letter: smart_punctuation_default(),
            paired_punctuation: true,
            punctuation_lock: PunctuationLock::Follow,
            navigation: NavigationPreferences::default(),
            keybindings: KeybindingPreferences::default(),
            word_character: WordCharacterPreferences::default(),
            frequency: FrequencyPreferences::default(),
            mixed_input: MixedInputPreferences::default(),
            local_modes: LocalModePreferences::default(),
            plugins: PluginPreferences::default(),
            game_compatibility: GameCompatibilityPreferences::default(),
            clipboard_history: false,
            // 会把输入内容发出设备的两条路径新装都关闭，由 Windows、macOS、Linux 的首次询问或各平台的设置开关打开。
            cloud_candidates: false,
            candidate_translations: true,
            candidate_english_gloss: false,
            candidate_pronunciation: false,
            english_suggestions: true,
            translation_target_language: TranslationTargetLanguage::default(),
            translation_secondary_language: None,
            translation_account: false,
            usage_reporting: true,
        }
    }
}

/// Stable fuzzy-pinyin rule identifiers and bit assignments shared with the Engine and Apple host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FuzzyPinyinRule {
    #[serde(rename = "z-zh")]
    ZZh,
    #[serde(rename = "c-ch")]
    CCh,
    #[serde(rename = "s-sh")]
    SSh,
    #[serde(rename = "n-l")]
    NL,
    #[serde(rename = "f-h")]
    FH,
    #[serde(rename = "r-l")]
    RL,
    #[serde(rename = "an-ang")]
    AnAng,
    #[serde(rename = "en-eng")]
    EnEng,
    #[serde(rename = "in-ing")]
    InIng,
    #[serde(rename = "ian-iang")]
    IanIang,
    #[serde(rename = "uan-uang")]
    UanUang,
}

impl FuzzyPinyinRule {
    fn mask(self) -> u32 {
        match self {
            Self::ZZh => 1 << 0,
            Self::CCh => 1 << 1,
            Self::SSh => 1 << 2,
            Self::NL => 1 << 3,
            Self::FH => 1 << 4,
            Self::RL => 1 << 5,
            Self::AnAng => 1 << 6,
            Self::EnEng => 1 << 7,
            Self::InIng => 1 << 8,
            Self::IanIang => 1 << 9,
            Self::UanUang => 1 << 10,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FuzzyPinyinPreferences {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub rules: BTreeSet<FuzzyPinyinRule>,
    /// Internal marker used to distinguish first enable from an intentionally
    /// empty rule selection. It is persisted but never rendered by the UI.
    #[serde(default, skip_serializing_if = "is_false")]
    pub seeded: bool,
}

impl FuzzyPinyinPreferences {
    /// Disabled fuzzy pinyin preserves the selected rules while presenting exact matching to Engine.
    pub fn active_rules(&self) -> u32 {
        if !self.enabled {
            return 0;
        }
        self.rules.iter().fold(0, |mask, rule| mask | rule.mask())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuanpinPreferences {
    #[serde(default = "enabled_by_default")]
    pub autocorrect_transposition: bool,
    #[serde(default = "enabled_by_default")]
    pub autocorrect_neighbor: bool,
}

impl Default for QuanpinPreferences {
    fn default() -> Self {
        Self {
            autocorrect_transposition: true,
            autocorrect_neighbor: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShuangpinProfile {
    #[default]
    Xiaohe,
    Ziranma,
    Shoudao,
    Microsoft,
}

/// 五笔码表版本。Engine 的编码是声明顺序（`wubi_profile`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WubiProfile {
    #[default]
    #[serde(rename = "wubi86")]
    Wubi86,
    #[serde(rename = "wubi98")]
    Wubi98,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HelpcodeSchema {
    Lantian,
    #[default]
    Ziranma,
    Shouyou2,
    Shouyouplus,
    Xiaohe,
    Jiajia,
    /// A user table under the resource set's `helpcodes/custom` directory.
    Custom(String),
}

impl HelpcodeSchema {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Lantian => "lantian",
            Self::Ziranma => "ziranma",
            Self::Shouyou2 => "shouyou2_0",
            Self::Shouyouplus => "shouyouplus",
            Self::Xiaohe => "xiaohe",
            Self::Jiajia => "jiajia",
            Self::Custom(value) => value,
        }
    }
}

impl Serialize for HelpcodeSchema {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for HelpcodeSchema {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "lantian" => Ok(Self::Lantian),
            "ziranma" => Ok(Self::Ziranma),
            "shouyou2_0" => Ok(Self::Shouyou2),
            "shouyouplus" => Ok(Self::Shouyouplus),
            "xiaohe" => Ok(Self::Xiaohe),
            "jiajia" => Ok(Self::Jiajia),
            value if crate::helpcode::is_custom_schema(value) => Ok(Self::Custom(value.into())),
            _ => Err(serde::de::Error::custom("unknown helpcode schema")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelpcodePreferences {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    pub schema: HelpcodeSchema,
    #[serde(default = "enabled_by_default")]
    pub show_in_candidate_window: bool,
}

impl Default for HelpcodePreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            schema: HelpcodeSchema::default(),
            show_in_candidate_window: true,
        }
    }
}

fn default_quanpin_helpcode() -> HelpcodePreferences {
    HelpcodePreferences {
        enabled: true,
        schema: HelpcodeSchema::Ziranma,
        show_in_candidate_window: false,
    }
}

/// Whether `path` is absolute on any OS a preferences document may be read on: a Unix path, a Windows drive path (`C:\...` or `C:/...`), a verbatim or device path (`\\?\...`, `\\.\...`) or a UNC share (`\\server\share`). Checked textually rather than with `Path::is_absolute`, which answers only for the OS doing the checking, so a Windows path saved by the Windows host would be refused when the same document is validated elsewhere.
pub fn is_absolute_model_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if bytes.first() == Some(&b'/') {
        return true;
    }
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
    {
        return true;
    }
    // `\\server\share`, `\\?\C:\...` and `\\.\device`: two leading separators and something after them.
    bytes.len() > 2 && bytes[0] == b'\\' && bytes[1] == b'\\' && bytes[2] != b'\\'
}

/// 判断本地模型镜像：空字符串，或不带凭据、查询和片段的 HTTPS 前缀。
pub fn valid_model_mirror(mirror: &str) -> bool {
    if mirror.is_empty() {
        return true;
    }
    if mirror.len() > 2048
        || mirror
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
        || !mirror
            .strip_prefix("https://")
            .is_some_and(|rest| rest.as_bytes().first().is_some_and(|byte| *byte != b'/'))
    {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(mirror) else {
        return false;
    };
    url.scheme() == "https"
        && url.host_str().is_some_and(|host| !host.is_empty())
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn default_shuangpin_helpcode() -> HelpcodePreferences {
    HelpcodePreferences {
        enabled: true,
        schema: HelpcodeSchema::Lantian,
        show_in_candidate_window: true,
    }
}

/// Persisted recognition provider identifiers. Hosts expose only the providers they implement: `system` is the platform speech adapter, not a cloud profile, and `local` is an installed on-device sherpa-onnx model directory named by `asr_model_path`, which needs a host built with the recognizer behind it.
pub const ASR_PROVIDERS: [&str; 8] = [
    "doubao",
    "siliconflow",
    "openai",
    "groq",
    "everyapi",
    "mistral",
    "system",
    "local",
];
/// OpenAI-compatible AI services exposed by the Apple settings surface and
/// shared by every host. Providers that need special request fields are still
/// handled in `ai.rs`; the rest use the common Chat Completions shape.
pub const AI_PROVIDERS: [&str; 12] = [
    "everyapi",
    "openai",
    "anthropic",
    "gemini",
    "deepseek",
    "qwen",
    "kimi",
    "zhipu",
    "siliconflow",
    "groq",
    "openrouter",
    "custom",
];
/// Polishing additionally supports DeepSeek, which offers no recognition.
pub const POLISH_PROVIDERS: [&str; 5] = ["siliconflow", "openai", "deepseek", "groq", "doubao"];

impl Preferences {
    pub fn active_helpcode(&self) -> HelpcodePreferences {
        match self.scheme {
            InputScheme::Shuangpin => self.shuangpin_helpcode.clone(),
            InputScheme::Quanpin => self.quanpin_helpcode.clone(),
            _ => HelpcodePreferences {
                enabled: false,
                ..HelpcodePreferences::default()
            },
        }
    }

    /// `edition` 的默认偏好：在 `Default` 之上换成本版本的默认方案，叠加版本表的 `preference_defaults`，并把触屏键盘的方案收窄到本版本提供的那些。full 得到的就是 `Default`。
    ///
    /// 默认方案不是全拼时，`last_chinese_scheme` 也指向它：从日文等方案切回中文、或偏好里的方案不可用而回退时，回到的是本版本的方案。
    pub fn for_edition(edition: &crate::edition::Edition) -> Self {
        let mut preferences = Self::default();
        if edition.default_scheme != preferences.scheme {
            preferences.scheme = edition.default_scheme;
            preferences.last_chinese_scheme = ChineseScheme::of(edition.default_scheme);
        }
        if let Some(mixed) = edition.preference_defaults.wubi_mixed_pinyin {
            preferences.wubi_mixed_pinyin = mixed;
        }
        preferences.touch_keyboard_schemes = TouchKeyboardSchemePreferences::for_edition(edition);
        preferences
    }

    /// Every setting back to its default, except what the user cannot simply retype.
    ///
    /// The source window's 恢复默认设置 clears a fixed list of preference keys, and that list does
    /// not name the translation or voice services at all -- over there their credentials live
    /// outside this document, so a reset there never costs a secret. Here they live in it, so the
    /// same promise has to be kept from the other direction: start at `Default` and carry the
    /// service configuration across.
    ///
    /// The endpoint, provider and model travel with the token rather than resetting beside it. A key left pointing at a default endpoint is worse than either keeping the pair or clearing it, because nothing on the page says the two no longer belong together. `asr_model_path` travels for the same reason: it is a model the user went and downloaded.
    ///
    /// `fuzzy_pinyin.seeded` is not a setting at all -- it records that the one-time seeding has
    /// happened -- so clearing it would silently re-seed rules the user had turned off.
    pub fn restored_to_defaults(&self) -> Self {
        self.restored_to_defaults_for(crate::edition::Edition::full())
    }

    /// [`Preferences::restored_to_defaults`]，只是回到的是 `edition` 的默认偏好（[`Preferences::for_edition`]）。
    pub fn restored_to_defaults_for(&self, edition: &crate::edition::Edition) -> Self {
        let mut next = Self::for_edition(edition);

        next.voice_input.asr_provider = self.voice_input.asr_provider.clone();
        next.voice_input.asr_app_key = self.voice_input.asr_app_key.clone();
        next.voice_input.asr_token = self.voice_input.asr_token.clone();
        next.voice_input.asr_tokens = self.voice_input.asr_tokens.clone();
        next.voice_input.asr_endpoint = self.voice_input.asr_endpoint.clone();
        next.voice_input.asr_model = self.voice_input.asr_model.clone();
        next.voice_input.asr_model_path = self.voice_input.asr_model_path.clone();
        next.voice_input.asr_model_mirror = self.voice_input.asr_model_mirror.clone();
        next.voice_input.asr_resource_id = self.voice_input.asr_resource_id.clone();
        next.voice_input.doubao_auth_mode = self.voice_input.doubao_auth_mode.clone();
        next.voice_input.polish_provider = self.voice_input.polish_provider.clone();
        next.voice_input.polish_token = self.voice_input.polish_token.clone();
        next.voice_input.polish_tokens = self.voice_input.polish_tokens.clone();
        next.voice_input.polish_endpoint = self.voice_input.polish_endpoint.clone();
        next.voice_input.polish_model = self.voice_input.polish_model.clone();

        next.ai_assistant.provider = self.ai_assistant.provider.clone();
        next.ai_assistant.model = self.ai_assistant.model.clone();
        next.ai_assistant.token = self.ai_assistant.token.clone();
        next.ai_assistant.tokens = self.ai_assistant.tokens.clone();
        next.ai_assistant.endpoint = self.ai_assistant.endpoint.clone();

        next.custom_translation.endpoint = self.custom_translation.endpoint.clone();
        next.custom_translation.api_key = self.custom_translation.api_key.clone();

        next.tencent_tmt.secret_id = self.tencent_tmt.secret_id.clone();
        next.tencent_tmt.secret_key = self.tencent_tmt.secret_key.clone();
        next.tencent_tmt.region = self.tencent_tmt.region.clone();

        next.niutrans.app_id = self.niutrans.app_id.clone();
        next.niutrans.apikey = self.niutrans.apikey.clone();

        next.fuzzy_pinyin.seeded = self.fuzzy_pinyin.seeded;

        next
    }

    /// 偏好里的全部服务凭据（token、密钥、应用 id），按文档里的路径列出。恢复默认设置时原样保留的、导出诊断包时换成 [`REDACTED`] 的，都是这一份清单；`preferences/tests.rs` 用字段名兜底，新加的凭据字段不进清单测试就失败。
    pub fn credential_slots(&mut self) -> [(&'static str, CredentialSlot<'_>); 12] {
        [
            (
                "voice_input.asr_app_key",
                CredentialSlot::Text(&mut self.voice_input.asr_app_key),
            ),
            (
                "voice_input.asr_token",
                CredentialSlot::Text(&mut self.voice_input.asr_token),
            ),
            (
                "voice_input.asr_tokens",
                CredentialSlot::Map(&mut self.voice_input.asr_tokens),
            ),
            (
                "voice_input.polish_token",
                CredentialSlot::Text(&mut self.voice_input.polish_token),
            ),
            (
                "voice_input.polish_tokens",
                CredentialSlot::Map(&mut self.voice_input.polish_tokens),
            ),
            (
                "ai_assistant.token",
                CredentialSlot::Text(&mut self.ai_assistant.token),
            ),
            (
                "ai_assistant.tokens",
                CredentialSlot::Map(&mut self.ai_assistant.tokens),
            ),
            (
                "custom_translation.api_key",
                CredentialSlot::Text(&mut self.custom_translation.api_key),
            ),
            (
                "tencent_tmt.secret_id",
                CredentialSlot::Text(&mut self.tencent_tmt.secret_id),
            ),
            (
                "tencent_tmt.secret_key",
                CredentialSlot::Text(&mut self.tencent_tmt.secret_key),
            ),
            (
                "niutrans.app_id",
                CredentialSlot::Text(&mut self.niutrans.app_id),
            ),
            (
                "niutrans.apikey",
                CredentialSlot::Text(&mut self.niutrans.apikey),
            ),
        ]
    }

    /// 诊断包里的配置快照：整份偏好文档，凭据清单里的每个字段都换成 [`REDACTED`]；此外任何键名符合服务端脱敏规则（含 `token`、`secret`、`password`、`api_key` 或以 `key` 结尾，不分大小写）的值也一律换掉，这样快照上传时一定通过服务端的校验。
    pub fn redacted_for_diagnostics(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or(serde_json::Value::Null);
        let mut copy = self.clone();
        for (path, _) in copy.credential_slots() {
            let mut cursor = &mut value;
            let mut segments = path.split('.').peekable();
            while let Some(segment) = segments.next() {
                let Some(object) = cursor.as_object_mut() else {
                    break;
                };
                if segments.peek().is_none() {
                    if object.contains_key(segment) {
                        object.insert(segment.to_owned(), REDACTED.into());
                    }
                    break;
                }
                let Some(next) = object.get_mut(segment) else {
                    break;
                };
                cursor = next;
            }
        }
        redact_sensitive_keys(&mut value);
        for pointer in DIAGNOSTIC_ENDPOINTS {
            if let Some(slot) = value.pointer_mut(pointer) {
                if let Some(endpoint) = slot.as_str() {
                    *slot = diagnostic_endpoint(endpoint).into();
                }
            }
        }
        value
    }

    pub fn validate(&self) -> Result<(), PreferencesError> {
        let tencent = &self.tencent_tmt;
        if tencent.secret_id.len() > 4096
            || !crate::text::is_bounded_text(&tencent.secret_key, 4096)
            || !crate::is_ascii_identifier(&tencent.secret_id)
            || tencent.region.len() > 64
            || !crate::is_ascii_alphanumeric_dash(&tencent.region)
        {
            return Err(PreferencesError::InvalidTencentTmt);
        }
        let niutrans = &self.niutrans;
        if !crate::text::is_bounded_text(&niutrans.app_id, 4096)
            || !crate::text::is_bounded_text(&niutrans.apikey, 4096)
            || (!niutrans.app_id.is_empty()
                && !crate::translation::usable_credential(&niutrans.app_id))
            || (!niutrans.apikey.is_empty()
                && !crate::translation::usable_credential(&niutrans.apikey))
        {
            return Err(PreferencesError::InvalidNiuTrans);
        }
        let translation = &self.custom_translation;
        if !crate::text::is_bounded_text(&translation.api_key, 4096)
            || (!translation.endpoint.is_empty()
                && !crate::translation::is_supported_endpoint(&translation.endpoint))
        {
            return Err(PreferencesError::InvalidCustomTranslation);
        }
        if !(1..=10).contains(&self.ai_assistant.candidate_limit)
            || !AI_PROVIDERS.contains(&self.ai_assistant.provider.as_str())
        {
            return Err(PreferencesError::InvalidAiAssistant);
        }
        let model_path = &self.voice_input.asr_model_path;
        if !ASR_PROVIDERS.contains(&self.voice_input.asr_provider.as_str())
            || !POLISH_PROVIDERS.contains(&self.voice_input.polish_provider.as_str())
            || !crate::text::is_bounded_text(model_path, 4096)
            || (!model_path.is_empty() && !is_absolute_model_path(model_path))
            || !valid_model_mirror(&self.voice_input.asr_model_mirror)
        {
            return Err(PreferencesError::InvalidVoiceInput);
        }
        if !(50..=200).contains(&self.floating_toolbar.scale_percent)
            || !(12..=48).contains(&self.floating_toolbar.font_size)
        {
            return Err(PreferencesError::InvalidFloatingToolbar);
        }
        if !(1..=8).contains(&self.mixed_input.minimum_prefix) {
            return Err(PreferencesError::InvalidMixedInput);
        }
        if !self.plugins.validate() {
            return Err(PreferencesError::InvalidPlugins);
        }
        if !self.game_compatibility.validate() {
            return Err(PreferencesError::InvalidGameCompatibility);
        }
        if !(1..=10).contains(&self.frequency.trigger_count)
            || !(1..=10).contains(&self.frequency.linear_step)
        {
            return Err(PreferencesError::InvalidFrequency);
        }
        if !(1..=9).contains(&self.candidate_page_size) {
            return Err(PreferencesError::InvalidPageSize);
        }
        if !(30..=60).contains(&self.touch_key_spacing_tenths)
            || !(40..=100).contains(&self.touch_row_spacing_tenths)
            || !(-12..=48).contains(&self.touch_keyboard_height_adjustment)
        {
            return Err(PreferencesError::InvalidTouchKeyboardSpacing);
        }
        self.custom_theme.validate()?;
        if self.touch_keyboard_schemes.enabled.is_empty()
            || self
                .touch_keyboard_schemes
                .selected
                .is_some_and(|selected| !self.touch_keyboard_schemes.enabled.contains(&selected))
        {
            return Err(PreferencesError::InvalidTouchKeyboardSchemes);
        }
        if !(12..=32).contains(&self.candidate_font_size) {
            return Err(PreferencesError::InvalidCandidateFontSize);
        }
        if !(12..=32).contains(&self.candidate_preedit_font_size) {
            return Err(PreferencesError::InvalidCandidateFontSize);
        }
        if !(50..=200).contains(&self.candidate_scale_percent)
            || !(50..=100).contains(&self.candidate_opacity_percent)
            || self
                .candidate_corner_radius
                .is_some_and(|radius| radius > 32)
        {
            return Err(PreferencesError::InvalidCandidateWindowStyle);
        }
        // Font family names are Unicode display names, not paths or identifiers.
        // Keep the existing UTF-8 byte budget while allowing localized families.
        if !valid_font_family(&self.candidate_font_family) {
            return Err(PreferencesError::InvalidCandidateFontFamily);
        }
        if self
            .candidate_english_font
            .as_deref()
            .is_some_and(|font| !valid_font_family(font))
        {
            return Err(PreferencesError::InvalidCandidateFontFamily);
        }
        // Match the 32 ordered supplementary families in Windows appearance.ts.
        if self.candidate_fallback_fonts.len() > 32
            || self
                .candidate_fallback_fonts
                .iter()
                .any(|font| !valid_font_family(font))
        {
            return Err(PreferencesError::InvalidCandidateFontFamily);
        }
        let paging = match self.word_character.keys {
            WordCharacterKeys::Brackets => self.navigation.brackets,
            WordCharacterKeys::MinusEqual => self.navigation.minus_equal,
        };
        if self.word_character.enabled && paging {
            return Err(PreferencesError::ConflictingKeyBindings);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencesSnapshot {
    pub format_version: u32,
    pub revision: u64,
    pub preferences: Preferences,
}

impl Default for PreferencesSnapshot {
    fn default() -> Self {
        Self {
            format_version: 1,
            revision: 0,
            preferences: Preferences::default(),
        }
    }
}

impl PreferencesSnapshot {
    /// 还没有偏好文件时 `edition` 读到的快照：修订号 0，内容是 [`Preferences::for_edition`]。
    pub fn for_edition(edition: &crate::edition::Edition) -> Self {
        Self {
            preferences: Preferences::for_edition(edition),
            ..Self::default()
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PreferencesError {
    #[error("floating toolbar settings are invalid")]
    InvalidFloatingToolbar,
    #[error("AI assistant provider or candidate limit is invalid")]
    InvalidAiAssistant,
    #[error("voice recognition or polishing provider is not supported")]
    InvalidVoiceInput,
    #[error("custom translation endpoint or API key is invalid")]
    InvalidCustomTranslation,
    #[error("Tencent translation credentials or region are invalid")]
    InvalidTencentTmt,
    #[error("NiuTrans translation credentials are invalid")]
    InvalidNiuTrans,
    #[error("candidate page size must be between 1 and 9")]
    InvalidPageSize,
    #[error("touch keyboard key spacing must be 3.0-6.0 and row spacing must be 4.0-10.0")]
    InvalidTouchKeyboardSpacing,
    #[error(
        "at least one touch keyboard scheme must be enabled and the selection must be visible"
    )]
    InvalidTouchKeyboardSchemes,
    #[error("custom theme keyboard design is invalid")]
    InvalidTouchKeyboardSkinDesign,
    #[error("custom theme base must be system or a built-in theme")]
    InvalidCustomThemeBase,
    #[error("candidate font size must be between 12 and 32")]
    InvalidCandidateFontSize,
    #[error("candidate window scale must be 50-200%, opacity 50-100% and corner radius 0-32")]
    InvalidCandidateWindowStyle,
    #[error("candidate text color must be #RRGGBB or omitted")]
    InvalidCandidateTextColor,
    #[error("candidate number color must be #RRGGBB or omitted")]
    InvalidCandidateNumberColor,
    #[error("candidate accent color must be #RRGGBB or omitted")]
    InvalidCandidateAccentColor,
    #[error("candidate selected color must be #RRGGBB or omitted")]
    InvalidCandidateSelectedColor,
    #[error("candidate hover color must be #RRGGBB or omitted")]
    InvalidCandidateHoverColor,
    #[error("candidate surface color must be #RRGGBB or omitted")]
    InvalidCandidateSurfaceColor,
    #[error("candidate border color must be #RRGGBB or omitted")]
    InvalidCandidateBorderColor,
    #[error("candidate font family must be non-empty, contain no control characters, and be at most 128 bytes")]
    InvalidCandidateFontFamily,
    #[error("custom theme candidate skin identifier is invalid")]
    InvalidCandidateSkin,
    #[error("word-to-character and paging cannot use the same keys")]
    ConflictingKeyBindings,
    #[error("frequency trigger count and linear step must be between 1 and 10")]
    InvalidFrequency,
    #[error("mixed English minimum prefix must be between 1 and 8")]
    InvalidMixedInput,
    #[error("plugin settings are invalid")]
    InvalidPlugins,
    #[error("game compatibility process names must be distinct .exe base names of at most 64 characters, at most 32 in total")]
    InvalidGameCompatibility,
    #[error("preferences changed; reload before saving")]
    Conflict,
    #[error("unsupported preferences format")]
    UnsupportedFormat,
    #[error("preferences revision exhausted")]
    RevisionExhausted,
    #[error("preferences document is too large")]
    DocumentTooLarge,
    #[error("preferences storage failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid preferences document: {0}")]
    Json(#[from] serde_json::Error),
}

/// What `PreferencesStore::recover` did.
#[derive(Debug, Clone, PartialEq)]
pub enum RecoveryOutcome {
    /// The document already loads (or does not exist yet); nothing was written or backed up.
    NotNeeded(PreferencesSnapshot),
    /// The damaged document was copied verbatim to `backup_path` and replaced by `snapshot`. `salvaged` is true when at least one setting from the damaged document survived; false means the replacement is the defaults.
    Recovered {
        snapshot: PreferencesSnapshot,
        backup_path: PathBuf,
        salvaged: bool,
    },
}

/// Which damaged documents `recover` may rewrite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoveryScope {
    /// Anything the normal read rejects, other than a storage failure.
    Unreadable,
    /// Only bytes that are not well-formed JSON at all. A well-formed document the schema rejects may come from a newer build and is left alone.
    Malformed,
}

pub struct PreferencesStore {
    directory: PathBuf,
    /// 构造时指定的版本；`None` 时以状态目录里的版本记录为准（[`crate::edition::Edition::recorded_in`]），没有记录就是 full。只影响没有偏好文件时读到的默认值和修复时垫底的默认值。
    edition: Option<&'static crate::edition::Edition>,
}

impl PreferencesStore {
    /// `directory` 的偏好存储，版本取自目录里的版本记录（准备宿主时写下，见 [`crate::edition::Edition::record_in`]）：各平台读写偏好的 C ABI 和设置应用只拿到这个目录，不必各自知道版本，偏好文件不见了或被修复时也回到本版本的默认偏好。full 的状态目录没有记录，行为与引入版本之前相同。
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            edition: None,
        }
    }

    /// `edition` 的偏好存储，不看目录里的版本记录。版本之间完全隔离，每个版本有自己的状态目录；这里只决定还没有偏好文件时读到的是哪个版本的默认值（[`PreferencesSnapshot::for_edition`]），以及修复损坏文件时以哪份默认值垫底。
    pub fn for_edition(
        directory: impl Into<PathBuf>,
        edition: &'static crate::edition::Edition,
    ) -> Self {
        Self {
            directory: directory.into(),
            edition: Some(edition),
        }
    }

    /// 这个存储所属的版本：构造时指定的，否则是目录里记录的，都没有时是 full。
    pub fn edition(&self) -> &'static crate::edition::Edition {
        self.edition
            .or_else(|| crate::edition::Edition::recorded_in(&self.directory))
            .unwrap_or_else(crate::edition::Edition::full)
    }

    /// 还没有偏好文件时读到的快照。
    ///
    /// 状态目录已经被准备过（[`Self::prepared_before`]）却没有偏好文件，说明是以前的版本装的、用户从没存过偏好：触屏键盘的方案按以前的默认列表（[`TouchKeyboardSchemePreferences::legacy_for_edition`]），升级后键盘不变。只有新装才得到现在的默认列表。准备宿主时会把读到的结果写成第一份偏好文件（[`Self::write_first_document`]），之后不再靠这个判断。
    fn missing_document(&self) -> PreferencesSnapshot {
        let edition = self.edition();
        let mut snapshot = if edition.is_full() {
            PreferencesSnapshot::default()
        } else {
            PreferencesSnapshot::for_edition(edition)
        };
        if self.prepared_before() {
            snapshot.preferences.touch_keyboard_schemes =
                TouchKeyboardSchemePreferences::legacy_for_edition(edition);
        }
        snapshot
    }

    /// 准备宿主时在状态目录里建的用户词库代次目录 `user/dictionaries` 已经存在：这个目录被某个版本准备过。新装在写下第一份偏好文件之前还没有它。
    fn prepared_before(&self) -> bool {
        std::fs::symlink_metadata(self.directory.join("user").join("dictionaries"))
            .is_ok_and(|metadata| metadata.is_dir())
    }

    /// 还没有偏好文件时，把此刻读到的偏好（[`Self::missing_document`]）写成第一份文件；已经有文件时什么也不写，返回读到的快照。
    ///
    /// 准备宿主时、在建 `user/dictionaries` 之前调用：新装和从以前的版本升级只在第一次准备之前分得清（见 [`Self::missing_document`]），写下文件后这个结论就固定在文件里，之后的准备和读取不会把一次新装误当成升级。设置应用恰好在这期间写下了第一份偏好时，以它为准。
    pub fn write_first_document(&self) -> Result<PreferencesSnapshot, PreferencesError> {
        let _lock = self.lock()?;
        let current = self.read_locked()?;
        if current.revision > 0 {
            return Ok(current);
        }
        let preferences = current.preferences.clone();
        self.save_locked(current, 0, preferences)
    }

    /// 修复损坏文件时垫底的默认偏好。
    fn default_preferences(&self) -> Preferences {
        match self.edition() {
            edition if edition.is_full() => Preferences::default(),
            edition => Preferences::for_edition(edition),
        }
    }

    /// The directory holding `preferences.json`, its lock and any `preferences.json.corrupt-*` backups `recover` wrote.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    fn open_lock(&self) -> Result<File, PreferencesError> {
        if !crate::storage::create_directory_and_check(&self.directory)? {
            return Err(PreferencesError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "preferences directory is not a real directory",
            )));
        }
        let lock = crate::file_lock::open_lock_file(self.directory.join("preferences.lock"))?;
        Ok(lock)
    }

    fn lock(&self) -> Result<File, PreferencesError> {
        let lock = self.open_lock()?;
        crate::file_lock::exclusive(&lock)?;
        Ok(lock)
    }

    fn path(&self) -> PathBuf {
        self.directory.join("preferences.json")
    }

    fn read_locked(&self) -> Result<PreferencesSnapshot, PreferencesError> {
        let path = self.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(self.missing_document())
            }
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_file() {
            return Err(PreferencesError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "preferences document is not a regular file",
            )));
        }
        let bytes = crate::bounded_io::read_bounded_file(
            crate::storage::open_private_file_in(&path)?,
            MAX_DOCUMENT_BYTES,
            || PreferencesError::DocumentTooLarge,
        )?;
        let snapshot: PreferencesSnapshot = serde_json::from_slice(&bytes)?;
        if snapshot.format_version != 1 {
            return Err(PreferencesError::UnsupportedFormat);
        }
        snapshot.preferences.validate()?;
        Ok(snapshot)
    }

    pub fn load(&self) -> Result<PreferencesSnapshot, PreferencesError> {
        let _lock = self.lock()?;
        self.read_locked()
    }

    /// None means the writer lock is busy; retry later without using defaults.
    /// File operations may still block on storage. Validation matches load().
    pub fn try_load(&self) -> Result<Option<PreferencesSnapshot>, PreferencesError> {
        let lock = self.open_lock()?;
        if !crate::file_lock::try_exclusive(&lock)? {
            return Ok(None);
        }
        self.read_locked().map(Some)
    }

    /// Capture under the preferences lock so disabling cannot race a later write.
    /// Lock order is preferences, then clipboard history; never reverse it.
    pub fn capture_clipboard_text(&self, text: String) -> Result<bool, PreferencesError> {
        let _lock = self.lock()?;
        if !self.read_locked()?.preferences.clipboard_history {
            return Ok(false);
        }
        let mut history = crate::clipboard::ClipboardHistoryStore::open(
            self.directory.join("clipboard_history.json"),
        );
        Ok(history.push(text)?)
    }

    /// Clear only while history is still disabled, using the same lock order
    /// as capture so another settings writer cannot re-enable between checks.
    pub fn clear_disabled_clipboard_history(&self) -> Result<(), PreferencesError> {
        let _lock = self.lock()?;
        if !self.read_locked()?.preferences.clipboard_history {
            let mut history = crate::clipboard::ClipboardHistoryStore::open(
                self.directory.join("clipboard_history.json"),
            );
            history.clear()?;
        }
        Ok(())
    }

    /// Compare-and-swap prevents stale settings windows or IME hosts losing updates.
    /// Corrupt or future-format files are never silently replaced with defaults.
    pub fn save(
        &self,
        expected_revision: u64,
        preferences: Preferences,
    ) -> Result<PreferencesSnapshot, PreferencesError> {
        let _lock = self.lock()?;
        let current = self.read_locked()?;
        self.save_locked(current, expected_revision, preferences)
    }

    /// 「恢复默认设置」：在同一把锁里读出当前文档，换成本存储所属版本的默认偏好（[`Preferences::restored_to_defaults_for`]，服务凭据和 `fuzzy_pinyin.seeded` 保留），再按 `expected_revision` 做比较并交换写回。修订号不符时返回 `Conflict`，什么也不写。
    pub fn restore_defaults(
        &self,
        expected_revision: u64,
    ) -> Result<PreferencesSnapshot, PreferencesError> {
        let _lock = self.lock()?;
        let current = self.read_locked()?;
        let restored = current.preferences.restored_to_defaults_for(self.edition());
        self.save_locked(current, expected_revision, restored)
    }

    /// `save` 的主体，调用方已持有锁并读出了 `current`。
    fn save_locked(
        &self,
        current: PreferencesSnapshot,
        expected_revision: u64,
        mut preferences: Preferences,
    ) -> Result<PreferencesSnapshot, PreferencesError> {
        if current.revision != expected_revision {
            return Err(PreferencesError::Conflict);
        }
        // Match the Windows baseline: the first transition from disabled to
        // enabled opts every fuzzy rule in once. The marker is separate from
        // the rule set so intentionally clearing every rule does not reseed
        // on a later disable/enable cycle.
        if preferences.fuzzy_pinyin.enabled
            && !current.preferences.fuzzy_pinyin.enabled
            && !current.preferences.fuzzy_pinyin.seeded
        {
            preferences.fuzzy_pinyin.rules = [
                FuzzyPinyinRule::ZZh,
                FuzzyPinyinRule::CCh,
                FuzzyPinyinRule::SSh,
                FuzzyPinyinRule::NL,
                FuzzyPinyinRule::FH,
                FuzzyPinyinRule::RL,
                FuzzyPinyinRule::AnAng,
                FuzzyPinyinRule::EnEng,
                FuzzyPinyinRule::InIng,
                FuzzyPinyinRule::IanIang,
                FuzzyPinyinRule::UanUang,
            ]
            .into_iter()
            .collect();
            preferences.fuzzy_pinyin.seeded = true;
        } else if current.preferences.fuzzy_pinyin.seeded {
            // Keep the internal marker monotonic even if a client sends a snapshot without the field.
            preferences.fuzzy_pinyin.seeded = true;
        }
        preferences.validate()?;
        // Hosts save whenever a setting might have changed, several processes and controllers at a time, and every reader reloads and reapplies the whole document when the revision moves. Writing the same settings again would cost an fsync under the exclusive lock and a reload everywhere for nothing, so an unchanged document keeps its revision. An existing file only: the first save still creates it.
        if current.revision > 0 && preferences == current.preferences {
            return Ok(current);
        }
        let snapshot = PreferencesSnapshot {
            format_version: 1,
            revision: current
                .revision
                .checked_add(1)
                .ok_or(PreferencesError::RevisionExhausted)?,
            preferences,
        };
        atomic_write(
            &self.directory,
            &self.path(),
            &serde_json::to_vec_pretty(&snapshot)?,
        )?;
        Ok(snapshot)
    }

    /// Replace a document that `load` rejects, keeping what can be kept.
    ///
    /// This is the counterpart of the source's `SyncConfigWithInstalledTemplate` repair of a config.toml that does not parse. The damaged bytes are first copied verbatim to `preferences.json.corrupt-YYYYMMDD-HHMMSS` (UTC) beside the document; if that copy cannot be written nothing else happens, so the original is never lost. Then every top-level setting the current schema accepts is carried over one at a time onto the defaults, and a section that fails as a whole (a wrong-typed sibling next to a service key, say) is retried field by field, so credentials survive the way `ReapplyRealCredentials` keeps real API tokens. Whatever still does not fit takes its default.
    ///
    /// A missing or already loadable document is `NotNeeded` and nothing is written, so calling this twice, or racing another writer that already repaired the file, is harmless. Storage failures are returned unchanged and never lead to a rewrite. There is no compare-and-swap: the caller has no valid revision to offer, and the lock plus the re-check that the document is still unreadable cover the race.
    pub fn recover(&self) -> Result<RecoveryOutcome, PreferencesError> {
        self.recover_within(RecoveryScope::Unreadable)
    }

    /// `recover`, restricted to a document that is not well-formed JSON (truncated, empty, overwritten with other bytes). A well-formed document the schema rejects - unknown fields or a newer `format_version` - returns the load error unchanged, because it is most likely a newer build's file and rewriting it behind the user's back would lose that build's settings. Input method hosts call this automatically; the explicit settings-page repair uses `recover`.
    pub fn recover_malformed(&self) -> Result<RecoveryOutcome, PreferencesError> {
        self.recover_within(RecoveryScope::Malformed)
    }

    fn recover_within(&self, scope: RecoveryScope) -> Result<RecoveryOutcome, PreferencesError> {
        let _lock = self.lock()?;
        let failure = match self.read_locked() {
            Ok(snapshot) => return Ok(RecoveryOutcome::NotNeeded(snapshot)),
            Err(PreferencesError::Io(error)) => return Err(PreferencesError::Io(error)),
            Err(failure) => failure,
        };
        let bytes = crate::bounded_io::read_bounded_file(
            crate::storage::open_private_file_in(&self.path())?,
            MAX_DOCUMENT_BYTES,
            || PreferencesError::DocumentTooLarge,
        )?;
        let document = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
        if scope == RecoveryScope::Malformed && document.is_some() {
            return Err(failure);
        }
        let backup_path = self.write_backup(&bytes)?;
        let (preferences, salvaged) = match &document {
            Some(document) => salvage_preferences(document, self.default_preferences())?,
            None => (self.default_preferences(), false),
        };
        let revision = match document
            .as_ref()
            .and_then(|document| document.get("revision"))
            .and_then(serde_json::Value::as_u64)
        {
            Some(revision) => revision
                .checked_add(1)
                .ok_or(PreferencesError::RevisionExhausted)?,
            // Hosts skip a document whose revision equals the one they last applied, so restarting at 1 could leave a running host on the pre-damage values. Seconds since the epoch are far above any revision a host counted up to and still leave the counter room to grow.
            None => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or(0)
                .max(1),
        };
        let snapshot = PreferencesSnapshot {
            format_version: 1,
            revision,
            preferences,
        };
        atomic_write(
            &self.directory,
            &self.path(),
            &serde_json::to_vec_pretty(&snapshot)?,
        )?;
        Ok(RecoveryOutcome::Recovered {
            snapshot,
            backup_path,
            salvaged,
        })
    }

    /// Copy the damaged bytes to a new file and make sure they reached the disk before the original is replaced. `create_new` means an existing backup is never overwritten; a name already taken gets a `-N` suffix.
    fn write_backup(&self, bytes: &[u8]) -> Result<PathBuf, PreferencesError> {
        let now = time::OffsetDateTime::now_utc();
        let stem = format!(
            "preferences.json.corrupt-{:04}{:02}{:02}-{:02}{:02}{:02}",
            now.year(),
            u8::from(now.month()),
            now.day(),
            now.hour(),
            now.minute(),
            now.second()
        );
        let mut attempt = 0u32;
        #[cfg(unix)]
        let directory = crate::storage::open_private_directory(&self.directory)?;
        loop {
            let name = if attempt == 0 {
                stem.clone()
            } else {
                format!("{stem}-{attempt}")
            };
            #[cfg(unix)]
            let (path, mut file) = {
                let descriptor = match rustix::fs::openat(
                    &directory,
                    std::ffi::OsStr::new(&name),
                    rustix::fs::OFlags::WRONLY
                        | rustix::fs::OFlags::CREATE
                        | rustix::fs::OFlags::EXCL
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC
                        | rustix::fs::OFlags::NONBLOCK,
                    rustix::fs::Mode::from_raw_mode(0o600),
                ) {
                    Ok(descriptor) => descriptor,
                    Err(error) if error == rustix::io::Errno::EXIST => {
                        attempt += 1;
                        continue;
                    }
                    Err(error) => return Err(std::io::Error::from(error).into()),
                };
                (self.directory.join(&name), File::from(descriptor))
            };
            #[cfg(not(unix))]
            let (path, mut file) = {
                let path = self.directory.join(&name);
                let file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        attempt += 1;
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                (path, file)
            };
            if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                drop(file);
                #[cfg(unix)]
                let _ = rustix::fs::unlinkat(
                    &directory,
                    std::ffi::OsStr::new(&name),
                    rustix::fs::AtFlags::empty(),
                );
                #[cfg(not(unix))]
                let _ = fs::remove_file(&path);
                return Err(error.into());
            }
            return Ok(path);
        }
    }
}

/// Whether `preferences` is a document `load` would accept.
fn acceptable_preferences(candidate: &serde_json::Map<String, serde_json::Value>) -> bool {
    serde_json::from_value::<Preferences>(serde_json::Value::Object(candidate.clone()))
        .is_ok_and(|preferences| preferences.validate().is_ok())
}

/// Carry every setting of a damaged document that the current schema accepts onto the defaults, one top-level key at a time, retrying a rejected section one field at a time. Returns the result and whether anything was kept.
///
/// `default` 是垫底的默认偏好，即存储所属版本的默认值。
fn salvage_preferences(
    document: &serde_json::Value,
    default: Preferences,
) -> Result<(Preferences, bool), PreferencesError> {
    let serde_json::Value::Object(mut salvaged) = serde_json::to_value(&default)? else {
        return Ok((default, false));
    };
    // A snapshot keeps its settings under `preferences`; a bare settings object at the root is accepted too.
    let source = match document.get("preferences") {
        Some(serde_json::Value::Object(source)) => source.clone(),
        _ => match document {
            serde_json::Value::Object(source) => source.clone(),
            _ => return Ok((default, false)),
        },
    };
    // 损坏的文档里没有触屏方案列表时，它出自以前的版本（现在的版本总是写出列表），垫底值里也去掉这个键，读回来按以前的默认列表，与这份文档还能正常读取时相同。
    if !source.contains_key("touch_keyboard_schemes") {
        salvaged.remove("touch_keyboard_schemes");
    }
    let mut kept = false;
    for (key, value) in &source {
        let mut candidate = salvaged.clone();
        candidate.insert(key.clone(), value.clone());
        if acceptable_preferences(&candidate) {
            salvaged = candidate;
            kept = true;
            continue;
        }
        let serde_json::Value::Object(fields) = value else {
            continue;
        };
        let mut section = match salvaged.get(key) {
            Some(serde_json::Value::Object(section)) => section.clone(),
            _ => serde_json::Map::new(),
        };
        let mut section_kept = false;
        for (field, field_value) in fields {
            let mut trial = section.clone();
            trial.insert(field.clone(), field_value.clone());
            let mut candidate = salvaged.clone();
            candidate.insert(key.clone(), serde_json::Value::Object(trial.clone()));
            if acceptable_preferences(&candidate) {
                section = trial;
                section_kept = true;
            }
        }
        if section_kept {
            salvaged.insert(key.clone(), serde_json::Value::Object(section));
            kept = true;
        }
    }
    // Every step above was accepted by the same check, so this cannot fail on the salvaged map.
    let preferences: Preferences = serde_json::from_value(serde_json::Value::Object(salvaged))?;
    Ok((preferences, kept))
}

fn atomic_write(directory: &Path, path: &Path, contents: &[u8]) -> Result<(), PreferencesError> {
    sweep_stale_temporaries(directory);
    #[cfg(unix)]
    {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let opened = crate::storage::open_private_directory(parent)?;
        let name = path.file_name().ok_or_else(|| {
            PreferencesError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "preferences path has no file name",
            ))
        })?;
        crate::storage::write_private_file_at(&opened, name, contents)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        temporary.write_all(contents)?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// How long a staged write has to sit before it is considered abandoned. A staged write takes
/// milliseconds; a day is far past anything a slow disk explains.
const STALE_TEMPORARY_AGE: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Remove staged writes nobody is going to finish.
///
/// `NamedTempFile` removes itself when it is dropped, but a process killed between creating the
/// file and renaming it drops nothing - and the input method is stopped exactly that way every time
/// it is reinstalled. The staged file then sits in the user's data directory forever, one per
/// interrupted write, and nothing else ever looks at it. Two were found there on a machine running
/// this client, holding a copy of the preferences and of the typing statistics.
///
/// Only files a day old are touched, and the age is what makes this safe rather than the lock: the
/// statistics document stages its writes into this same directory under a lock of its own, so a
/// sweep that went by name alone could delete a write that was in flight.
#[cfg(unix)]
fn sweep_stale_temporaries(directory: &Path) {
    let Ok(directory) = crate::storage::open_private_directory(directory) else {
        return;
    };
    let Ok(mut entries) = rustix::fs::Dir::read_from(&directory) else {
        return;
    };
    let now = std::time::SystemTime::now();
    while let Some(Ok(entry)) = entries.next() {
        let name = entry.file_name();
        if !name.to_bytes().starts_with(b".tmp") {
            continue;
        }
        let Ok(fd) = rustix::fs::openat(
            &directory,
            name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        ) else {
            continue;
        };
        let file: File = fd.into();
        let Ok(metadata) = file.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let abandoned = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= STALE_TEMPORARY_AGE);
        if abandoned {
            let _ = rustix::fs::unlinkat(&directory, name, rustix::fs::AtFlags::empty());
        }
    }
}

#[cfg(not(unix))]
fn sweep_stale_temporaries(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let now = std::time::SystemTime::now();
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(".tmp"))
        {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let abandoned = metadata
            .modified()
            .ok()
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age >= STALE_TEMPORARY_AGE);
        if abandoned {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// 诊断快照里替换凭据的值，与服务端校验要求的写法相同。
pub const REDACTED: &str = "<redacted>";

/// [`Preferences::credential_slots`] 里的一个凭据字段。
pub enum CredentialSlot<'a> {
    Text(&'a mut String),
    Map(&'a mut BTreeMap<String, String>),
}

/// 服务端（`POST /v1/users/me/diagnostics`）要求必须是 [`REDACTED`] 的键名：`(?i)token|secret|password|api_key|key$`。
pub fn is_sensitive_key(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.contains("token")
        || name.contains("secret")
        || name.contains("password")
        || name.contains("api_key")
        || name.ends_with("key")
}

/// 用户可以自填的服务地址。地址本身不是凭据，但有的服务把密钥放在查询串或 `user:password@` 里，诊断快照只留协议、主机、端口和路径。
const DIAGNOSTIC_ENDPOINTS: [&str; 4] = [
    "/voice_input/asr_endpoint",
    "/voice_input/polish_endpoint",
    "/ai_assistant/endpoint",
    "/custom_translation/endpoint",
];

/// 去掉地址里的用户信息、查询串和片段；解析不了的地址整个换成 [`REDACTED`]，因为看不出密钥藏在哪。
fn diagnostic_endpoint(endpoint: &str) -> String {
    if endpoint.is_empty() {
        return String::new();
    }
    let Ok(mut url) = reqwest::Url::parse(endpoint) else {
        return REDACTED.to_owned();
    };
    // Both setters only fail for URLs that cannot carry userinfo (no host), which then has none to strip.
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

fn redact_sensitive_keys(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object.iter_mut() {
                if is_sensitive_key(key) {
                    *child = REDACTED.into();
                } else {
                    redact_sensitive_keys(child);
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(redact_sensitive_keys),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
