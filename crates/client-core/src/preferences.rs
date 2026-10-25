//! Versioned local preferences. Hosts supply a private application data directory.
//! All writers coordinate through the stable lock file, not the replaced data file.

pub use crate::chinese_conversion::TraditionalVariant;
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
use unicode_segmentation::UnicodeSegmentation;

/// The largest preference document accepted by the shared host boundary. This
/// covers a validated custom skin photo while preventing a damaged local file
/// from forcing an unbounded allocation during startup or recovery.
const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;

fn valid_font_family(value: &str) -> bool {
    !value.is_empty() && crate::text::is_bounded_text(value, 128)
}

/// 账号偏好接口接受的值，有意只留这几种标量：凭据、任意 JSON 对象和输入内容都不经过这个边界，各平台宿主把自己可以安全同步的本地设置映射成这些标量。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AccountPreferenceValue {
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
}

impl AccountPreferenceValue {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Boolean(_) => "boolean",
            Self::Integer(_) => "integer",
            Self::Number(_) => "number",
            Self::String(_) => "string",
        }
    }
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
    /// 全拼 14 键：QWERTY 上相邻两个字母一个键（L、M 单独一键），按组消歧，与九键共用九宫格会话。只配全拼。
    FourteenKey,
    /// 九键双拼：数字键上每个音节固定两位（声母一位、韵母一位），表由 `nine_key_shuangpin_profile` 选，与九键共用九宫格会话（#6572）。引擎方案是全拼，不配别的方案。
    NineKeyShuangpin,
    /// 全拼十八键；Android 和 iOS 原生宿主创建会话时按它自动开启十八键全拼网格（host-api 的 `layout_key_grid_for`），鸿蒙宿主自己按它调用 `msime_client_set_key_grid`，其他宿主保留但忽略这个偏好。
    EighteenKey,
}

/// 触屏双拼的字母键布局；与全拼的布局独立保存，切换方案后仍保留选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TouchShuangpinLayout {
    #[default]
    TwentySixKey,
    FourteenKey,
    EighteenKey,
}

impl TouchShuangpinLayout {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
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

/// 文本编辑面板可排序的侧边动作；方向、选择、文档边界与换行固定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEditButton {
    Copy,
    Paste,
    SelectAll,
    Cut,
    Space,
    NumberPad,
    Delete,
}

/// 从左列上到下，再到右列前三格；换行固定在右下角。
pub fn default_text_edit_button_order() -> Vec<TextEditButton> {
    use TextEditButton::*;
    vec![Copy, Paste, SelectAll, Cut, Space, NumberPad, Delete]
}

/// 触屏 26 键按「123」切到的数字层。桌面宿主原样保留、不使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TwentySixKeyNumberLayout {
    /// 1 到 0 排成一行，下面是符号行。
    #[default]
    Row,
    /// 九宫格数字层：3×3 数字键，排列跟 [`NumberKeypadOrder`] 走。
    NineKey,
}

/// 触屏键盘把正在拼写的内容（全拼、双拼、五笔、粤拼）放在哪里。桌面宿主原样保留、不使用：桌面候选窗画不画拼音行归 `candidate_preedit_style`，它的默认值是画，拿它当触屏的位置开关会让触屏默认把拼写挪出输入框。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TouchSpellingPosition {
    /// 作为组字写进输入框，候选栏不再重复画它。
    #[default]
    Editor,
    /// 只画在候选栏上，不写进输入框。
    CandidateBar,
}

/// Android 文本编辑触控板的本机偏好；默认值、步长范围和恢复设置归共享层，不参与账号同步。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextTrackpadPreferences {
    pub enabled: bool,
    pub hold_to_open: bool,
    pub horizontal_dp: u16,
    pub vertical_dp: u16,
    pub tap_selection: bool,
    pub haptics: bool,
    pub trail: bool,
    pub magnifier: bool,
}

impl Default for TextTrackpadPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            hold_to_open: false,
            horizontal_dp: 12,
            vertical_dp: 28,
            tap_selection: true,
            haptics: true,
            trail: true,
            magnifier: true,
        }
    }
}

impl TextTrackpadPreferences {
    pub const HORIZONTAL_MIN_DP: u16 = 4;
    pub const HORIZONTAL_MAX_DP: u16 = 48;
    pub const VERTICAL_MIN_DP: u16 = 8;
    pub const VERTICAL_MAX_DP: u16 = 96;

    pub fn validate(&self) -> bool {
        (Self::HORIZONTAL_MIN_DP..=Self::HORIZONTAL_MAX_DP).contains(&self.horizontal_dp)
            && (Self::VERTICAL_MIN_DP..=Self::VERTICAL_MAX_DP).contains(&self.vertical_dp)
    }
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
#[serde(default, rename_all = "camelCase")]
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

    #[doc(hidden)]
    pub fn validate(&self) -> bool {
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
#[serde(default)]
pub struct CustomTheme {
    /// 自定义主题的底：`system` 或五个内置主题之一，不能是 `custom`，也不能是只在部分宿主上提供的 `native`（见 `GlobalTheme::is_base`）。皮肤包和取色器没设的候选颜色由它补上；没有键盘设计时，键盘也画它的。应用了皮肤包时，改用包清单自己的 `base`。
    #[serde(skip_serializing_if = "is_system_theme")]
    pub base: crate::theme::GlobalTheme,
    /// 浅色模式用的候选窗皮肤包（宿主皮肤目录里的文件夹名），也是没设 `candidate_skin_dark` 时深色模式用的那款。不会是全局主题 ID。皮肤包只在它 `base` 的明暗下绘制，见 `CustomTheme::candidate_skin_for`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_skin: Option<String>,
    /// 深色模式用的候选窗皮肤包。`None` 时深色模式也取 `candidate_skin`，只设过一款皮肤的旧文档因此照旧。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_skin_dark: Option<String>,
    /// The candidate colour pickers, drawn over the package's colours.
    #[serde(skip_serializing_if = "CustomCandidateColors::is_empty")]
    pub candidate_colors: CustomCandidateColors,
    /// The keyboard editor design; community, saved and AI-generated keyboard skins are applied by writing it here. `None` until the user designs or applies one: the custom theme then draws the base theme's keyboard.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<TouchKeyboardSkinDesign>,
}

fn is_system_theme(theme: &crate::theme::GlobalTheme) -> bool {
    *theme == crate::theme::GlobalTheme::System
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
        if [&self.candidate_skin, &self.candidate_skin_dark]
            .into_iter()
            .flatten()
            .any(|skin| !crate::theme::is_selectable_id(skin))
        {
            return Err(PreferencesError::InvalidCandidateSkin);
        }
        Ok(())
    }

    /// 宿主在 `dark` 模式下要加载的皮肤包：深色模式优先取 `candidate_skin_dark`，没设时与浅色模式一样取 `candidate_skin`。选中的包画不画还要看它的 `base` 是否属于这个模式，由 `theme::resolve` 判断。
    pub fn candidate_skin_for(&self, dark: bool) -> Option<&str> {
        if dark {
            self.candidate_skin_dark
                .as_deref()
                .or(self.candidate_skin.as_deref())
        } else {
            self.candidate_skin.as_deref()
        }
    }
}

/// The seven candidate colour pickers, each `#RRGGBB` or unset.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
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
    /// 全拼 14 键（`InputScheme::Quanpin` 配 `TouchKeyboardLayout::FourteenKey`）。默认不启用，由用户在「添加语言」或全拼的布局选择里打开。
    FourteenKey,
    /// 九键双拼（`InputScheme::Quanpin` 配 `TouchKeyboardLayout::NineKeyShuangpin`，表由 `nine_key_shuangpin_profile` 选）。默认不启用。
    NineKeyShuangpin,
    /// 全拼十八键（`InputScheme::Quanpin` 配 `TouchKeyboardLayout::EighteenKey`），Android、iOS 和鸿蒙的触屏键盘都有；默认不启用，由用户选择布局。
    EighteenKey,
}

impl TouchKeyboardScheme {
    /// Every touch scheme in picker order. Schemes are appended, never reordered.
    pub const ALL: [Self; 20] = [
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
        Self::FourteenKey,
        Self::NineKeyShuangpin,
        Self::EighteenKey,
    ];

    /// 新装时键盘显示的方案：只有中文方案（拼音、双拼、五笔和手写）。日文（9 键和 26 键）、韩文、粤拼、注音（大千和 9 键）、越南文、藏文和笔画都由用户自己在「添加语言」里打开：日文词典和语言词库在 Android 上按需下载，默认不启用它们，新装用户就不会看到一个还没有词典的键盘。全拼 14 键也不默认启用：它是全拼的另一种键面，由用户在全拼的布局选择或「添加语言」里打开。
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

/// 「应用例外」里一个应用固定的起始模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppInputModeRule {
    Chinese,
    English,
}

/// 「应用例外」的条目上限与应用标识的字节上限。规则表进 Windows Server 的偏好快照和 iOS 桥接的偏好文档；32 条、每条 64 字节以内最多约 2.6KB，bundle id 和程序文件名很少超过 64 字节。
pub const MAX_APP_INPUT_MODE_RULES: usize = 32;
pub const MAX_APP_INPUT_MODE_RULE_ID_BYTES: usize = 64;

/// 应用标识：macOS 是 bundle id，Windows 是进程的可执行文件基名（如 `code.exe`，设置页写入前转成小写），Linux 是输入法框架报告的程序名（如 `firefox`）。非空、不超过 `MAX_APP_INPUT_MODE_RULE_ID_BYTES` 字节，不含控制字符和路径分隔符，两侧没有空白。
fn valid_app_list_identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_APP_INPUT_MODE_RULE_ID_BYTES
        && id.trim() == id
        && !id
            .chars()
            .any(|character| character.is_control() || matches!(character, '\\' | '/'))
}

/// 整张表按 ASCII 不分大小写地不重复，因为 Windows 按这个规则比较进程名，只差大小写的两条会让同一个进程拿到两种模式。
fn valid_app_input_mode_rules(rules: &BTreeMap<String, AppInputModeRule>) -> bool {
    let mut seen = BTreeSet::new();
    rules.len() <= MAX_APP_INPUT_MODE_RULES
        && rules
            .keys()
            .all(|id| valid_app_list_identifier(id) && seen.insert(id.to_ascii_lowercase()))
}

/// 应用名单与应用例外使用同一口径、同一档上限。标识认法相同，只差大小写的两条会命中同一个应用。
fn valid_app_identifier_list(apps: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    apps.len() <= MAX_APP_INPUT_MODE_RULES
        && apps
            .iter()
            .all(|id| valid_app_list_identifier(id) && seen.insert(id.to_ascii_lowercase()))
}

/// 跨次上屏空格：只补新输入与前文的接缝。默认关，应用排除采用共享应用标识口径。
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CrossCommitSpacing {
    pub enabled: bool,
    pub excluded_apps: Vec<String>,
}

impl CrossCommitSpacing {
    pub fn validate(&self) -> bool {
        valid_app_identifier_list(&self.excluded_apps)
    }

    pub fn enabled_for_application(&self, application: &str) -> bool {
        self.enabled
            && valid_app_list_identifier(application)
            && !self
                .excluded_apps
                .iter()
                .any(|entry| entry.eq_ignore_ascii_case(application))
    }
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

/// 韩语实体键盘布局；缺少字段时保留两套式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum KoreanLayout {
    #[default]
    Dubeolsik,
    Romaja,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KoreanPreferences {
    pub layout: KoreanLayout,
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
#[serde(default)]
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

/// 中文标点下 `'` 和 `"` 键出哪种引号。`Corner` 按键对应：`'` 出「」，`"`（Shift+'）出『』。`CornerWhenTraditional` 在 `traditional_chinese_output` 打开时等于 `Corner`，否则等于 `Curly`；解析见 `crate::punctuation::punctuation_marks`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuoteStyle {
    #[default]
    Curly,
    Corner,
    CornerWhenTraditional,
}

impl QuoteStyle {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// 中文标点下 `[` `]` 键出实心【】还是空心〖〗。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BracketStyle {
    #[default]
    Solid,
    Hollow,
}

impl BracketStyle {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
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

/// octagram `.gram` 语法模型选择：`None` 不用语法模型（默认），其余三项
/// 各对应一个按需下载的资源包（`grammar-model-<变体>`）。白霜（GPL-3.0 且
/// 构建链许可未声明）本轮不提供。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrammarModel {
    #[default]
    None,
    /// 万象 LTS（CC-BY-4.0），`wanxiang-lts-zh-hans.gram`。
    Wanxiang,
    /// 八股文·词级（LGPL-3.0），`zh-hans-t-essay-bgw.gram`。
    BaguwenWord,
    /// 八股文·词级紧凑（LGPL-3.0），`zh-hans-t-essay-bgw-compact.gram`。
    BaguwenCompact,
    /// 白霜/墨奇（实验，GPL-3.0 且构建链许可未声明），`zh-moqi.gram`。
    /// 收录与旧版 Windows 正式版一致，界面上标「实验」。
    Baishuang,
}

/// Candidate sentence-association sources. The dictionary lattice keeps its historical default; neural rerankers are opt-in because they add model work while typing or settling.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
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
    /// octagram `.gram` 语法模型：对应资源包装好后引擎才拿到文件路径，
    /// 选中但还没下载时等于 `None`（逐字节退回无模型基线）。
    #[serde(default)]
    pub grammar_model: GrammarModel,
    /// 搭配项标定系数（log10 域）：0 = 关闭加成。与 Windows `[association]`
    /// `sentence_collocation_weight` 同名同出厂值；不进设置页，只在
    /// preferences.json 里手调，等于出厂值时不写进文档。键缺失、类型不符或
    /// 非有限值时回出厂值（见 `deserialize_sentence_collocation_weight`）。
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_weight",
        skip_serializing_if = "is_default_sentence_collocation_weight"
    )]
    pub sentence_collocation_weight: f64,
    /// 搭配上下文与词的码点长度上限/下限，对应 Windows `sentence_collocation_max_length`
    /// / `sentence_collocation_min_length`；取值口径与写入约定同 `sentence_collocation_weight`。
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_max_length",
        skip_serializing_if = "is_default_sentence_collocation_max_length"
    )]
    pub sentence_collocation_max_length: i32,
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_min_length",
        skip_serializing_if = "is_default_sentence_collocation_min_length"
    )]
    pub sentence_collocation_min_length: i32,
    /// 命中搭配的惩罚，对应 Windows `sentence_collocation_penalty`。
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_penalty",
        skip_serializing_if = "is_default_sentence_collocation_penalty"
    )]
    pub sentence_collocation_penalty: f64,
    /// 全部窗口未命中时的常数分，对应 Windows `sentence_collocation_non_penalty`。
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_non_penalty",
        skip_serializing_if = "is_default_sentence_collocation_non_penalty"
    )]
    pub sentence_collocation_non_penalty: f64,
    /// 搭配长度低于下限时的弱命中惩罚，对应 Windows `sentence_collocation_weak_penalty`。
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_weak_penalty",
        skip_serializing_if = "is_default_sentence_collocation_weak_penalty"
    )]
    pub sentence_collocation_weak_penalty: f64,
    /// 句尾词的「词+$」项惩罚，对应 Windows `sentence_collocation_rear_penalty`。
    #[serde(
        deserialize_with = "deserialize_sentence_collocation_rear_penalty",
        skip_serializing_if = "is_default_sentence_collocation_rear_penalty"
    )]
    pub sentence_collocation_rear_penalty: f64,
}

impl Default for SentenceAssociationPreferences {
    fn default() -> Self {
        Self {
            word_lattice: true,
            neural_desktop: false,
            neural_keyboard: false,
            show_next_on_duplicate: false,
            grammar_model: GrammarModel::None,
            sentence_collocation_weight: SENTENCE_COLLOCATION_WEIGHT_DEFAULT,
            sentence_collocation_max_length: SENTENCE_COLLOCATION_MAX_LENGTH_DEFAULT,
            sentence_collocation_min_length: SENTENCE_COLLOCATION_MIN_LENGTH_DEFAULT,
            sentence_collocation_penalty: SENTENCE_COLLOCATION_PENALTY_DEFAULT,
            sentence_collocation_non_penalty: SENTENCE_COLLOCATION_NON_PENALTY_DEFAULT,
            sentence_collocation_weak_penalty: SENTENCE_COLLOCATION_WEAK_PENALTY_DEFAULT,
            sentence_collocation_rear_penalty: SENTENCE_COLLOCATION_REAR_PENALTY_DEFAULT,
        }
    }
}

/// octagram 搭配调参系数的出厂值：与 Windows `installer/default_config/config.default.toml`
/// 的 `[association]` 键和 `engine` 的 `GramConfig::default()` 逐项一致（万象现行
/// 6/2/-14/-6/-100/-18，weight 1.0）。
const SENTENCE_COLLOCATION_WEIGHT_DEFAULT: f64 = 1.0;
const SENTENCE_COLLOCATION_MAX_LENGTH_DEFAULT: i32 = 6;
const SENTENCE_COLLOCATION_MIN_LENGTH_DEFAULT: i32 = 2;
const SENTENCE_COLLOCATION_PENALTY_DEFAULT: f64 = -14.0;
const SENTENCE_COLLOCATION_NON_PENALTY_DEFAULT: f64 = -6.0;
const SENTENCE_COLLOCATION_WEAK_PENALTY_DEFAULT: f64 = -100.0;
const SENTENCE_COLLOCATION_REAR_PENALTY_DEFAULT: f64 = -18.0;

/// 搭配调参系数的容错读取，与 Windows `value_or(出厂值)` 同口径：键缺失走
/// `#[serde(default)]`（即 `Default` 里的出厂值），类型不符、超出目标范围或
/// 非有限值回出厂值，文档其余字段照常生效——单个手误不该让整份偏好不可读。
/// JSON 里 `1e400` 能解析出 `inf`，不挡会把非有限分带进词格打分。
fn tuned_collocation_f64<'de, D>(deserializer: D, default: f64) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(serde_json::Value::deserialize(deserializer)?
        .as_f64()
        .filter(|value| value.is_finite())
        .unwrap_or(default))
}

fn tuned_collocation_i32<'de, D>(deserializer: D, default: i32) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(serde_json::Value::deserialize(deserializer)?
        .as_i64()
        .and_then(|value| i32::try_from(value).ok())
        .unwrap_or(default))
}

fn deserialize_sentence_collocation_weight<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_f64(d, SENTENCE_COLLOCATION_WEIGHT_DEFAULT)
}

fn deserialize_sentence_collocation_max_length<'de, D>(d: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_i32(d, SENTENCE_COLLOCATION_MAX_LENGTH_DEFAULT)
}

fn deserialize_sentence_collocation_min_length<'de, D>(d: D) -> Result<i32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_i32(d, SENTENCE_COLLOCATION_MIN_LENGTH_DEFAULT)
}

fn deserialize_sentence_collocation_penalty<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_f64(d, SENTENCE_COLLOCATION_PENALTY_DEFAULT)
}

fn deserialize_sentence_collocation_non_penalty<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_f64(d, SENTENCE_COLLOCATION_NON_PENALTY_DEFAULT)
}

fn deserialize_sentence_collocation_weak_penalty<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_f64(d, SENTENCE_COLLOCATION_WEAK_PENALTY_DEFAULT)
}

fn deserialize_sentence_collocation_rear_penalty<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    tuned_collocation_f64(d, SENTENCE_COLLOCATION_REAR_PENALTY_DEFAULT)
}

fn is_default_sentence_collocation_weight(value: &f64) -> bool {
    *value == SENTENCE_COLLOCATION_WEIGHT_DEFAULT
}

fn is_default_sentence_collocation_max_length(value: &i32) -> bool {
    *value == SENTENCE_COLLOCATION_MAX_LENGTH_DEFAULT
}

fn is_default_sentence_collocation_min_length(value: &i32) -> bool {
    *value == SENTENCE_COLLOCATION_MIN_LENGTH_DEFAULT
}

fn is_default_sentence_collocation_penalty(value: &f64) -> bool {
    *value == SENTENCE_COLLOCATION_PENALTY_DEFAULT
}

fn is_default_sentence_collocation_non_penalty(value: &f64) -> bool {
    *value == SENTENCE_COLLOCATION_NON_PENALTY_DEFAULT
}

fn is_default_sentence_collocation_weak_penalty(value: &f64) -> bool {
    *value == SENTENCE_COLLOCATION_WEAK_PENALTY_DEFAULT
}

fn is_default_sentence_collocation_rear_penalty(value: &f64) -> bool {
    *value == SENTENCE_COLLOCATION_REAR_PENALTY_DEFAULT
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub default_ime_mode: DefaultImeMode,
    #[serde(default)]
    pub ime_mode_scope: ImeModeScope,
    /// 应用例外：切到这些应用时从固定的中文或英文开始，优先于按应用记忆、全局状态和 `default_ime_mode`，在全局作用域下同样生效；用户在应用里手动切换后，本次停留期间让位，离开再回来重新生效。macOS 按 bundle id、Windows 按进程基名、Linux 按输入法框架报告的程序名查找。没有规则时不写进文档，没有这个键的旧版本照样能读。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub app_input_mode_rules: BTreeMap<String, AppInputModeRule>,
    /// Vim 模式：名单命中的前台应用里，交给应用的无修饰 Esc 同时把水杉切到英文，Vim 用户可以立刻输入 `hjkl`；组字开着时 Esc 仍只取消组字。标识口径与 `app_input_mode_rules` 相同（macOS 按 bundle id，Windows 按进程基名）。没有名单时不写进文档，没有这个键的旧版本照样能读。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vim_mode_apps: Vec<String>,
    #[serde(default)]
    pub voice_input: VoiceInputPreferences,
    #[serde(default)]
    pub ai_assistant: AiAssistantPreferences,
    /// Controls local whole-sentence candidate sources. Neural reranking remains opt-in until
    /// its model is installed.
    #[serde(default)]
    pub sentence_association: SentenceAssociationPreferences,
    /// 上屏后推荐下一词；Android 原生键盘提供入口，独立于整句候选，默认关闭。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub next_word_prediction: bool,
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
    pub global_theme: crate::theme::GlobalTheme,
    #[serde(default)]
    pub custom_theme: CustomTheme,
    #[serde(default)]
    pub candidate_layout: CandidateLayout,
    #[serde(default)]
    pub candidate_preedit_style: CandidatePreeditStyle,
    /// Show the current/total page text in Linux candidate panels, independently of preedit. On macOS it also governs the ‹ › page arrows beside that text, so with the logo and the reading hidden too the card loses its top row. Paging by key and wheel is unaffected on both.
    #[serde(default = "enabled_by_default")]
    pub show_candidate_page_number: bool,
    #[serde(default)]
    pub tsf_preedit_style: PreeditStyle,
    #[serde(default)]
    pub diagnostic_log: DiagnosticLogPreferences,
    #[serde(default = "enabled_by_default")]
    pub candidate_follow_cursor: bool,
    /// 切换中英文后在光标旁短暂显示一个不抢焦点的「中」/「英」徽标。macOS 和 Windows 自己画，HarmonyOS 和 Linux 交给各自的面板（见 `HostCapabilities::input_mode_hud`），其他宿主保留这个偏好但不画。
    #[serde(default = "enabled_by_default")]
    pub input_mode_hud: bool,
    /// Linux 聚焦输入框时显示当前中英文状态，与切换提示独立。新装与缺字段的旧文档默认关闭；其他宿主保留但不使用。
    #[serde(default)]
    pub input_mode_focus_hud: bool,
    /// Linux 允许屏幕角落的附加模式提示；关闭后保留光标旁提示和其他位置的产品标志。缺省开启以兼容旧配置。
    #[serde(default = "enabled_by_default")]
    pub input_mode_corner_hud: bool,
    /// 在 macOS 菜单栏放一个状态项，常驻显示当前输入模式（中/双/五/日……，英文时为「英」），不需要把「英」加成系统输入源。默认关闭；只有 macOS 宿主画它（见 `HostCapabilities::input_mode_status_item`），其他宿主保留这个偏好但不画。
    #[serde(default)]
    pub input_mode_status_item: bool,
    /// 候选窗和悬浮工具栏左端的水杉 logo。新装默认隐藏（见 `Default`）；已存文档缺这个字段时读成显示，升级沿用原来的样子。
    #[serde(default = "enabled_by_default")]
    pub show_app_logo: bool,
    /// 「展开候选」：横排候选窗组字时按 Tab、↓ 展开成多行候选网格（一行一页，上下滚动），候选窗上也多一个展开按钮；竖排候选窗不受影响。默认关闭，缺字段的旧文档读成关闭。只有声明 `HostCapabilities::expand_candidates` 的宿主（目前只有 macOS）读它，其他宿主保留这个偏好但不用。
    #[serde(default)]
    pub expand_candidates: bool,
    /// macOS 展开候选的鼠标按钮，独立于页码开关；默认显示，关闭后仍可用下键展开。
    #[serde(default = "enabled_by_default")]
    pub show_candidate_expand_button: bool,
    pub scheme: InputScheme,
    /// Show the Wubi code suffix that remains after the typed prefix.
    #[serde(default = "enabled_by_default")]
    pub wubi_code_hint: bool,
    /// 完整四码且整个列表只有一个候选时，在第四键提交。
    ///
    /// *哪些*码是这样由 Engine 回答；词是否就在那里离开输入法由用户决定，关掉表示它留在候选列表里，等空格或数字键选走。
    #[serde(default = "enabled_by_default")]
    pub wubi_auto_commit_unique: bool,
    /// 五笔 `z` 通配：`z` 匹配码表里该位置的任意一个字母（含首位）。
    ///
    /// 关掉时 `z` 不是码位，组字开头按下它照旧按混输开关打开临时拼音反查。两条路都要占用这个键，
    /// 所以通配打开时反查关掉。默认关，与来源出厂 `wubi_z_mode = "off"` 一致。
    #[serde(default)]
    pub wubi_z_wildcard: bool,
    /// 五笔顶字：完整四码之后再敲一个字母时上屏首选，并把这个字母留作下一组开头。
    ///
    /// 关掉只在拼音混输开启时生效：那个字母并进混输串；混输关闭时它无处容纳，仍按顶字上屏，不会丢字。
    /// 默认开，与来源出厂一致，也是接入这个开关之前唯一可能的行为。
    #[serde(default = "enabled_by_default")]
    pub wubi_fifth_code_top_commit: bool,
    /// 五笔拼音混输：五笔候选之后接着列出同一串字母的全拼候选，五笔编码打不出时直接出拼音候选。
    #[serde(default)]
    pub wubi_mixed_pinyin: bool,
    /// 五笔用 86 还是 98 码表；个人词条和学习记录也按它分开存。
    #[serde(default)]
    pub wubi_profile: WubiProfile,
    #[serde(default)]
    pub touch_keyboard_layout: TouchKeyboardLayout,
    /// Android 大屏竖屏也使用分离键盘；还需宿主的分离主开关开启，默认关闭。其他宿主原样保留。
    #[serde(default)]
    pub touch_keyboard_split_portrait: bool,
    /// Android 和 iOS 双拼键盘的 26 / 14 键选择；默认 26 键，其他宿主保留该字段。
    #[serde(default, skip_serializing_if = "TouchShuangpinLayout::is_default")]
    pub touch_shuangpin_layout: TouchShuangpinLayout,
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
    /// Android 文本编辑面板七个侧边动作的完整排列；其他宿主保留但不使用。
    #[serde(default = "default_text_edit_button_order")]
    pub touch_text_edit_button_order: Vec<TextEditButton>,
    /// 26 键按「123」时出一行 1 到 0 的数字符号页，还是九宫格那样的 3×3 数字层。
    #[serde(default)]
    pub touch_twenty_six_key_number_layout: TwentySixKeyNumberLayout,
    /// 触屏 26 键双拼时在字母键底部画当前方案的声母/韵母提示，读屏也读它。默认关；全拼、独立英文和本地模式本来就不画，不受它影响。桌面宿主原样保留、不使用。
    #[serde(default = "disabled_by_default")]
    pub touch_shuangpin_key_hints: bool,
    /// Android 日语九键连点同一键时循环假名；关闭后每次轻点都追加键面假名，四向滑动不变。默认开，保留旧文档的输入习惯；其他宿主原样保留。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub touch_japanese_toggle_input: bool,
    /// 触屏键盘的拼写写进输入框还是只画在候选栏；缺这个键的旧文档读作输入框。
    #[serde(default)]
    pub touch_spelling_position: TouchSpellingPosition,
    /// 仅 Android 使用，其他宿主原样保留。
    #[serde(default)]
    pub android_text_trackpad: TextTrackpadPreferences,
    /// 中文触屏键盘的快捷符号，单个字素、去重，最多十二项；宿主只负责显示与提交。
    #[serde(default = "default_touch_quick_symbols")]
    pub touch_quick_punctuation_symbols: Vec<String>,
    /// The optional buttons on the touch keyboard's toolbar, the counterpart of the floating toolbar's component switches. The voice entry stays under `touch_voice_shortcut`.
    #[serde(default)]
    pub touch_toolbar: TouchToolbarPreferences,
    /// HarmonyOS 手机键盘空闲工具栏上的五个工具和显示方式。默认全开、不隐藏，就是没有这组设置时鸿蒙键盘的样子；等于默认值时不写进文档，没有这个键的旧版本照样能读。`touch_toolbar` 的缺省（剪贴板关、没有常用语和输入方式）与鸿蒙工具栏对不上，所以另起一组。其他宿主不读它，设置页只在宿主报告 `touch_toolbar_tools` 时给出这组开关。
    #[serde(default, skip_serializing_if = "HarmonyToolbarPreferences::is_default")]
    pub harmony_toolbar: HarmonyToolbarPreferences,
    /// 当前方案是日文、韩文、越南文或藏文时保留，记住要回到的中文方案。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_chinese_scheme: Option<ChineseScheme>,
    #[serde(default)]
    pub shuangpin_profile: ShuangpinProfile,
    /// `shuangpin_profile` 为 `custom` 时用的键位表。这里只限制大小，合不合法由 Engine 判定（`msime_engine::host::validate_shuangpin_custom_profile`）；表缺失或不合法时宿主按小鹤运行。选别的方案时保留，切回 `custom` 还能用。
    #[serde(default, skip_serializing_if = "ShuangpinCustomProfile::is_empty")]
    pub shuangpin_custom_profile: ShuangpinCustomProfile,
    /// 九键双拼（`touch_keyboard_layout` 为 `nine_key_shuangpin`）用哪张表。等于默认值时不写进文件，没用过九键双拼的文档与以前的版本相同。
    #[serde(default, skip_serializing_if = "NineKeyShuangpinProfile::is_default")]
    pub nine_key_shuangpin_profile: NineKeyShuangpinProfile,
    /// `nine_key_shuangpin_profile` 为 `custom` 时用的表，形状与 `shuangpin_custom_profile` 相同，键是数字：`initials` 放 23 个声母，`finals` 放 33 个韵母，各一个数字；`zero_initials` 放 12 个零声母音节，各两位数字。这里只限制大小，合不合法由 Engine 判定（`msime_engine::host::validate_nine_key_shuangpin_table`）；表缺失或不合法时宿主按默认的内置方案运行。选别的方案时保留，空表不写进文件。
    #[serde(default, skip_serializing_if = "ShuangpinCustomProfile::is_empty")]
    pub nine_key_shuangpin_custom_profile: ShuangpinCustomProfile,
    #[serde(default = "enabled_by_default")]
    pub shuangpin_preedit_uses_raw: bool,
    /// 「输入时显示双拼键位提示」：双拼组字时在候选窗旁显示当前方案的键位图，高亮刚按下的键，上屏后隐藏。macOS 和 Windows 画它。
    ///
    /// 没写过时为 `None`，不写进文档：这个开关以前只存在 macOS 本机的 defaults 里（`MSIMEClientShuangpinKeymap`），macOS 在文档从没带过这一项时仍读那里的旧选择，再由下一次保存写进文档；文档带着这一项载入过一次之后 macOS 删掉那个旧键，此后缺这一项在所有宿主上都是关。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shuangpin_keymap_hint: Option<bool>,
    /// The Vietnamese input method and tone placement.
    #[serde(default)]
    pub vietnamese: VietnamesePreferences,
    /// 可选韩语布局；宿主未声明能力时仍运行两套式。
    #[serde(default)]
    pub korean: KoreanPreferences,
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
    /// 候选译文字号，12–32 点；缺省使用宿主的 12pt 默认值，目前由 macOS 消费。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_translation_font_size: Option<u8>,
    /// Overall size of the floating candidate window, 50-200 percent. The host multiplies `candidate_font_size` and every piece of window geometry (paddings, row heights, header, arrows, insets, shadow) by it.
    #[serde(default = "default_candidate_scale_percent")]
    pub candidate_scale_percent: u16,
    /// Opacity of the candidate card, 50-100 percent. It multiplies only the alpha of the card fill, its border and the skin background image; text, numbers and the selection highlight stay opaque.
    #[serde(default = "default_candidate_opacity_percent")]
    pub candidate_opacity_percent: u8,
    /// Corner radius of the candidate card in points (DIP on Windows), 0-32. It wins over the skin package's `corner_radius_dip`, which wins over the host's own constant; absent means the host or skin decides. Row and selection radii become the smaller of the host's row radius and this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_corner_radius: Option<u8>,
    /// 候选卡片边缘到候选的距离，单位为点（Windows 上为 DIP），0–16，在 `candidate_scale_percent` 缩放之前生效。它优先于皮肤包的内边距和宿主自己的常量（macOS 为 4pt）；缺省时由宿主或皮肤决定。宿主可以保留一个很小的下限，让选中高亮不压住卡片边框（macOS 至少留出边框的宽度）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_padding: Option<u8>,
    /// 窗口水平内边距，0–16pt；缺省时沿用 `candidate_padding`，再跟随皮肤与宿主。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_padding_horizontal: Option<u8>,
    /// 窗口垂直内边距，0–16pt；缺省时沿用 `candidate_padding`，再跟随皮肤与宿主。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_padding_vertical: Option<u8>,
    /// 候选项左右各自的内边距，0–16pt；缺省时保留皮肤与宿主的留白。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_item_padding_horizontal: Option<u8>,
    /// 候选项上下各自的内边距，0–16pt；缺省时保留皮肤与宿主的行高规则。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_item_padding_vertical: Option<u8>,
    /// 选中与悬停背景的独立圆角，0–32pt；缺省时保留原有圆角推导。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_item_corner_radius: Option<u8>,
    /// 相邻候选项之间的间距，0–16pt；横排为列距，竖排为行距。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_item_spacing: Option<u8>,
    /// 候选窗口严格按文字所需的宽度收紧，不垫出厂最小宽度（Windows `appearance.candidate_window_fit_content`，出厂关）。与皮肤包 `skin.toml` 的 `fit_content` 是或关系：任一为真即收紧。默认关，关着时不写进文档。
    #[serde(default, skip_serializing_if = "is_false")]
    pub candidate_window_fit_content: bool,
    #[serde(default = "default_candidate_font_family")]
    pub candidate_font_family: String,
    /// Optional leading face for the Windows candidate glyph fallback chain.
    /// The host supplies its default; absent values preserve other hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_english_font: Option<String>,
    #[serde(default = "default_candidate_fallback_fonts")]
    pub candidate_fallback_fonts: Vec<String>,
    pub learning: bool,
    /// 回车上屏英文时把该英文单词写入英文词库，之后可作为候选出现（Windows `input.enter_learns_english_word`）。Windows 正式版 `installer/default_config/config.default.toml` 出厂为 `true`，所以等于出厂值时文档里不写这个键，没有它的旧版本照样按开启读；关掉后回车照常上屏，只是不学词。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub enter_learns_english_word: bool,
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
    /// 繁体输出用哪套 OpenCC 配置（`s2t`、`s2tw`、`s2twp`），只在 `traditional_chinese_output` 打开时起作用；宿主转换时把它交给 `msime_client_convert_to_traditional`。默认 `s2t`，默认时不写进文档，没有这个键的旧版本照样能读。
    #[serde(default, skip_serializing_if = "is_default_traditional_variant")]
    pub traditional_chinese_variant: TraditionalVariant,
    /// 中英文之间加空格：一次上屏的文本里，汉字与 ASCII 字母、数字相邻处插入一个半角空格（host-api 在全角转换之后做，见 `crates/host-api/src/chinese_english_spacing.rs`）。两次上屏之间不加，候选窗显示不变。默认关；关着时不写进文档，没有这个键的旧版本照样能读。
    #[serde(default, skip_serializing_if = "is_false")]
    pub chinese_english_spacing: bool,
    /// 跨次上屏的中英文边界补空格，独立于单次上屏内的空格开关，默认关。宿主必须提供光标前的真实字符；目前仅 macOS 接入。
    #[serde(default, skip_serializing_if = "is_false")]
    pub cross_commit_spacing: bool,
    /// 跨次空格的应用排除名单，macOS 使用 Bundle ID；写代码、网址或邮箱的应用可在这里保留原样。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cross_commit_spacing_excluded_apps: Vec<String>,
    pub chinese_punctuation: bool,
    #[serde(default = "smart_punctuation_default")]
    pub smart_punctuation: bool,
    #[serde(default = "smart_punctuation_default")]
    pub smart_punctuation_repeat: bool,
    /// 刚上屏的中文标点后按空格，把它改回 ASCII 并吞掉空格。所有宿主默认关：它改写的是用户已经看着落下去的字符，而且没有像重复转中文那样的反悔手势（再按一次空格只是空格），所以不跟着智能标点默认开，见 `smart_punctuation_default`。
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
    /// 内部标记，不是设置：这份文档已经按「智能标点默认开」定过一次。macOS 和 Windows 以前默认关，旧文档把那个 `false` 明明白白存了下来，分不出「从没动过」和「自己关掉」，所以读到没有这个标记的文档时，在这两个平台上把智能标点、重复标点转中文和两种直出一次性打开，再标上它（`Preferences::settle_smart_punctuation_default`）；之后用户关掉就一直关着。新文档从一开始就带着它（`Default` 为真），缺键时按假读，也就是「这是旧文档」。保存时只会从假变真（`PreferencesStore::save`），恢复默认设置和导入设置文件都不会把它清掉。不在任何设置页上显示。
    #[serde(default, skip_serializing_if = "is_false")]
    pub smart_punctuation_migrated: bool,
    #[serde(default = "enabled_by_default")]
    pub paired_punctuation: bool,
    #[serde(default)]
    pub punctuation_lock: PunctuationLock,
    /// 大写锁定打开时，中文模式下没有组字时的标点按英文标点输出，和切到英文模式时一样（`punctuation_lock` 为 `chinese` 时仍是中文标点）。只有向会话报告大写锁定状态的宿主（`HostCapabilities::caps_lock_punctuation`）会用到它。默认关，打开前的行为不变。
    #[serde(default)]
    pub caps_lock_ascii_punctuation: bool,
    /// 中文标点下引号键的样式（W#474、W#54）。等于默认值（弯引号）时不写进文档，没有这个键的旧版本照样能读。
    #[serde(default, skip_serializing_if = "QuoteStyle::is_default")]
    pub quote_style: QuoteStyle,
    /// 中文标点下方头括号键的样式（W#24）。等于默认值（实心）时不写进文档。
    #[serde(default, skip_serializing_if = "BracketStyle::is_default")]
    pub bracket_style: BracketStyle,
    /// 顿号自定义：中文标点下 `\` 键输出顿号「、」（Windows v0.11.1 `input.enumeration_comma_backslash`）。默认开，等于默认值时不写进文档。两个顿号键都关掉时按「`\` 开」读，归一在 `crate::punctuation::punctuation_marks`。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub enumeration_comma_backslash: bool,
    /// 顿号自定义：中文标点下 `/` 键输出顿号「、」。默认关：新架构的 `/` 在词首是命令模式触发键，现有用户的 `/` 从来不出顿号，和 Windows 升级用户（旧配置没有这个键时并成 `false`）一致；打开后词首仍打开命令模式，其余位置出顿号。等于默认值时不写进文档。
    #[serde(default, skip_serializing_if = "is_false")]
    pub enumeration_comma_slash: bool,
    #[serde(default)]
    pub navigation: NavigationPreferences,
    #[serde(default)]
    pub keybindings: KeybindingPreferences,
    #[serde(default)]
    pub word_character: WordCharacterPreferences,
    /// 用两个专用键直接选当前页的第二、第三个候选。等于默认值（关闭）时不写进文档，没有这个键的旧版本照样能读。
    #[serde(
        default,
        skip_serializing_if = "SecondThirdCandidatePreferences::is_default"
    )]
    pub second_third_candidate: SecondThirdCandidatePreferences,
    /// 五笔普通组字的二三候选字符，候选不足时保留原标点行为。
    #[serde(
        default,
        skip_serializing_if = "WubiCandidateSelectionPreferences::is_default"
    )]
    pub wubi_candidate_selection: WubiCandidateSelectionPreferences,
    /// 普通退格在剩余拼写即将删空时，是否撤回最近一次选词；默认保持原行为。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub backspace_reverts_selection: bool,
    /// 造词时按 Esc 只丢掉还没选的拼写、保留已经选好的词，再按一次才整体取消（Windows `input.escape_keeps_selected_word` 的语义，即小狼毫的 `ClearNonConfirmedComposition`）。Windows 正式版 `installer/default_config/config.default.toml` 出厂为 `false`，关着时 Esc 连已选词一起丢掉，与旧行为一致；关着时不写进文档，没有这个键的旧版本照样能读。
    #[serde(default, skip_serializing_if = "is_false")]
    pub escape_keeps_selected_word: bool,
    #[serde(default)]
    pub frequency: FrequencyPreferences,
    #[serde(default)]
    pub mixed_input: MixedInputPreferences,
    /// 连续多语言输入的语言集合；默认关闭，只有至少两种不同语言时才交给 Engine。
    #[serde(
        default,
        skip_serializing_if = "MultilingualInputPreferences::is_default"
    )]
    pub multilingual_input: MultilingualInputPreferences,
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
    /// 本机剪贴板历史最多保留的条数；这是本机隐私和存储选择，不随设置文件导出。
    #[serde(default = "default_clipboard_history_limit")]
    pub clipboard_history_limit: usize,
    /// 云剪贴板改走用户自建的中继（协议见 `docs/cloud-clipboard-relay.md`）：地址为空表示不用中继，走水杉账号。配置了中继时云剪贴板只经中继，不再碰账号。令牌和语音、翻译服务的凭据一样明文存在这份文档里，列在 [`Preferences::credential_slots`]，不进设置文件导出（`settings_document::LOCAL_SECTIONS`）和账号设置同步。没配置时不写进文档，没有这个键的旧版本照样能读。
    #[serde(
        default,
        skip_serializing_if = "CloudClipboardRelayPreferences::is_default"
    )]
    pub cloud_clipboard_relay: CloudClipboardRelayPreferences,
    /// 向所选云候选服务发送当前组字。新装默认关闭；旧文档缺开关时沿用开启行为。
    #[serde(default = "enabled_by_default")]
    pub cloud_candidates: bool,
    /// 本机云候选来源与 HTTP 配置，不参与设置导出和账号同步。
    #[serde(default)]
    pub cloud_candidate_provider: crate::cloud::candidates::CloudCandidatePreferences,
    /// 用户内容请求（云候选、翻译、AI、语音、账号、更新检查、词库与模型下载）走哪个代理。默认值与 Windows 正式版的 `network.proxy_mode` / `network.proxy_server` 相同：跟随系统，不填服务器。`custom` 但服务器留空时按 `system` 处理。等于默认值时不写进文档，没有这个键的旧版本照样能读。
    #[serde(
        default,
        skip_serializing_if = "crate::network::NetworkPreferences::is_default"
    )]
    pub network: crate::network::NetworkPreferences,
    /// 新装默认关闭候选翻译；旧文档缺字段时仍沿用原有开启行为，显式保存的选择不变。
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
    /// 候选译文中显示 n.、v. 等词性标记；关闭后只显示释义。出厂默认对齐 Windows v0.11.1 的
    /// `candidate_translation_show_pos = true`。
    #[serde(default = "enabled_by_default")]
    pub candidate_translation_show_pos: bool,
    /// 候选译文只显示第一条释义，关闭时显示全部释义。出厂默认对齐 Windows v0.11.1 的
    /// `candidate_translation_single_sense = false`。
    #[serde(default)]
    pub candidate_translation_single_sense: bool,
    /// 固定排位的候选词是否显示角标。出厂默认对齐 Windows v0.11.1 的 `candidate_fixed_badge = true`。
    #[serde(default = "enabled_by_default")]
    pub candidate_fixed_badge: bool,
    /// 固定排位候选角的样式，可选 "paperclip"（📎）、"pushpin"（📌）、"dot"（·）；未知值渲染时不加角标。
    /// 出厂默认对齐 Windows v0.11.1 的 `candidate_fixed_badge_style = "paperclip"`。
    #[serde(default = "default_candidate_fixed_badge_style")]
    pub candidate_fixed_badge_style: String,
    /// AI 联想候选的来源角标样式，可选 "robot"（🤖）、"whale"（🐳）；未知值回退到 robot。
    /// 出厂默认对齐 Windows v0.11.1 的 `candidate_ai_badge_style = "robot"`。
    #[serde(default = "default_candidate_ai_badge_style")]
    pub candidate_ai_badge_style: String,
    /// Show read-only English word completions in direct English touch input.
    /// Hosts without a direct English suggestion surface preserve this value.
    #[serde(default = "enabled_by_default")]
    pub english_suggestions: bool,
    /// 额外授权：允许英文组字和前文发送给已启用的 AI 或自定义云候选服务。
    #[serde(default)]
    pub english_online_candidates: bool,
    #[serde(default)]
    pub translation_target_language: TranslationTargetLanguage,
    /// Optional second language for mobile candidate glosses. `None` shows a single language and is omitted from serialized snapshots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation_secondary_language: Option<TranslationTargetLanguage>,
    /// 候选翻译服务选的是水杉账号时为真，候选词会发到 `https://api.msime.app/v1/translate`。新装和缺字段时都是关闭，必须由用户显式选择。
    #[serde(default)]
    pub translation_account: bool,
    /// 候选翻译服务选的是 AI 辅助时为真，复用 `ai_assistant` 的服务商、接口地址、Token 与模型做候选词翻译；不要求 `ai_assistant.enabled`，只想用翻译、不开 AI 联想也能生效。与 `translation_account` 一样必须由用户显式选择。
    #[serde(default)]
    pub translation_ai: bool,
    /// 向 `https://api.msime.app/v1/telemetry/events` 发送匿名使用报告（每日活跃、会话结束、崩溃摘要；见 `telemetry` 和 PRIVACY.md）。默认开启；关闭后停止全部上报并清空本地队列。读取方把缺失的键当作开启。
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
    /// 识别语言：系统识别器用 BCP 47 区域形式（`zh-CN`、`en-US`），其他识别服务用短形式
    /// （`zh-cn`、`en`、`ja`、`auto`）。出厂默认取 `zh-CN`，与共享设置页（React 的
    /// `defaultVoiceInput`）和 macOS 原生回退窗口一致；缺字段的旧文档仍由各宿主自己的回退处理，
    /// 读盘和界面比较都不区分大小写。
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
    /// DeepSeek 润色的思考强度：disabled | low | high | max，仅接口地址属于 DeepSeek 时生效；请求体按档写 `thinking.type` 与 `reasoning_effort`（shared/voice 的 polish_cloud_text）。
    #[serde(default = "default_thinking_level")]
    pub polish_thinking: String,
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

/// EveryAPI 的识别默认模型以前写成 `openai/whisper-large-v3-turbo`，那是照搬 Apple 预设的名字，EveryAPI 从没有它的渠道（请求返回 503 `model_not_found`）。共享设置页切到 EveryAPI 时会把默认模型写进 `asr_model`，所以选过 EveryAPI 的文档里存的就是这个值。
const RETIRED_EVERYAPI_ASR_MODEL: &str = "openai/whisper-large-v3-turbo";

impl VoiceInputPreferences {
    /// 读出文档时把 EveryAPI 下存着的旧默认模型清空，由 `voice::provider` 补上现在的默认（`volc.seedasr.sauc.duration`）。只认这一个值：用户自己填的其他模型原样保留；文件本身等下一次保存才改写。
    fn retire_unavailable_asr_model(&mut self) {
        if self.asr_provider == "everyapi" && self.asr_model.trim() == RETIRED_EVERYAPI_ASR_MODEL {
            self.asr_model.clear();
        }
    }
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
            language: "zh-CN".into(),
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
            polish_thinking: "disabled".into(),
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
    /// 每个提供方记住的接口地址与模型，键为提供方 id；槽位只存自定义值，空槽或缺键表示用目录默认值。设置页切提供方时按槽位回填 endpoint/model，自定义过的值不会随切换丢失（Windows `endpoint_*`/`model_*` 槽的对应物）。
    #[serde(default)]
    pub endpoints: BTreeMap<String, String>,
    #[serde(default)]
    pub models: BTreeMap<String, String>,
    /// DeepSeek 的思考强度：disabled | low | high | max，仅接口地址属于 DeepSeek 时生效；请求体按档写 `thinking.type` 与 `reasoning_effort`（client-providers 的 `chat_completion_body`）。
    #[serde(default = "default_thinking_level")]
    pub thinking: String,
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

/// `ai_assistant.thinking` 与 `voice_input.polish_thinking` 共同的出厂档位。
fn default_thinking_level() -> String {
    "disabled".into()
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
            endpoints: BTreeMap::new(),
            models: BTreeMap::new(),
            thinking: "disabled".into(),
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
#[serde(default)]
pub struct DiagnosticLogPreferences {
    /// Server-side timing and window state: slow request stages, candidate
    /// window, floating toolbar, menus, focus sessions and transport status.
    pub server: bool,
    /// In-process TSF preedit and input latency, buffered and batched out.
    pub tsf: bool,
    /// 允许 `msime-mcp candidates` 在终端里按用户的设置和词库打出一串按键的候选（#9663）。候选来自用户自己的词库，等于把打过的词交给任何能运行这个命令的程序，所以缺省关闭，只在桌面「维护与诊断」里由用户打开。和诊断日志一样只属于本机：不随云同步、不进设置文件（键名在 `diagnostic_log` 下，两处的本机清单已经按这个名字排除）。MCP 的偏好工具不提供这一项，助手不能自己把它打开。
    pub command_line_candidates: bool,
}

/// Windows 游戏兼容：游戏声明自己画候选却不画时，由水杉的候选窗显示。DLL 在每次激活时读取，对已打开的游戏要重新激活输入法才生效。各项都等于默认值时不写进文档，没有这个键的旧版本照样能读。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
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
    /// 切换输入方案的按钮：点开列出全拼、双拼、五笔、粤拼、注音等方案。默认开启——在 macOS 27 上粤、注这类菜单栏入口只能由用户自己去系统设置里添加，这个按钮让不加入口也能切换。macOS、Windows 和 Linux 的悬浮工具栏画它，Linux 的「工具栏」子菜单也列出它（见 `HostCapabilities::floating_toolbar_input_scheme`）。
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
    /// 显示一段时间无交互后把悬浮工具栏淡出隐藏。Windows 旧正式版 v0.11.1 的
    /// `installer/default_config/config.default.toml` 里 `floating_toolbar_auto_hide = false`，这里对齐它。
    /// macOS 此前把这个行为写死成常开 10 秒（`FloatingToolbarPanel.mm` 的 `_idleTimer`），新字段接管后
    /// 默认不再自动隐藏；想保留旧行为的用户在共享设置页把这一项打开。
    #[serde(default)]
    pub auto_hide: bool,
    /// 自动隐藏延时（秒）。Windows v0.11.1 的 `floating_toolbar_auto_hide_delay` 出厂为 5、可选 1–60，
    /// 越界按 5 处理。macOS 面板用 `FloatingToolbarBehavior.h` 解析它来决定空闲计时器的间隔。
    #[serde(default = "default_toolbar_auto_hide_delay")]
    pub auto_hide_delay: u16,
    /// 悬浮工具栏的柔和阴影，与候选窗一致。Windows 旧正式版 v0.11.1 的
    /// `installer/default_config/config.default.toml` 里 `floating_toolbar_shadow = true`，这里对齐它；
    /// macOS 此前写死 `self.hasShadow = YES`（`FloatingToolbarPanel.mm`）。
    #[serde(default = "enabled_by_default")]
    pub shadow: bool,
}

fn default_toolbar_scale() -> u16 {
    100
}
fn default_toolbar_font_size() -> u16 {
    24
}
fn default_toolbar_auto_hide_delay() -> u16 {
    5
}

/// 触屏工具栏的可选按钮；撤销和重做目前由 Android 宿主提供，其余宿主只保留设置。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TouchToolbarPreferences {
    /// 由当前编辑器处理撤销、重做；默认隐藏，输入法不保存文本历史。
    pub undo: bool,
    pub redo: bool,
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
            undo: false,
            redo: false,
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

/// HarmonyOS 手机键盘空闲工具栏的工具开关和显示方式，缺哪个字段就取默认值。标志键和收起键固定在两端，不在这里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HarmonyToolbarPreferences {
    pub emoji: bool,
    pub phrase: bool,
    pub clipboard: bool,
    pub skin: bool,
    pub scheme: bool,
    /// 显示方式选了「隐藏」：空闲时整行不显示，组字时候选栏照常显示。
    pub hidden: bool,
}

impl Default for HarmonyToolbarPreferences {
    fn default() -> Self {
        Self {
            emoji: true,
            phrase: true,
            clipboard: true,
            skin: true,
            scheme: true,
            hidden: false,
        }
    }
}

impl HarmonyToolbarPreferences {
    fn is_default(&self) -> bool {
        *self == Self::default()
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
            auto_hide: false,
            auto_hide_delay: default_toolbar_auto_hide_delay(),
            shadow: true,
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
pub struct LocalModePreferences {
    pub unicode: bool,
    /// Shift+T 日期时间快捷输入模式（Windows `utility.date_time_mode`，出厂开）。行内日期时间混入普通候选由 `date_time_inline` 单独开关。
    pub date_time: bool,
    /// 日期时间混进普通候选（对应 Windows `utility.date_time_candidates`）：全拼输入 `riqi`、`sj`、`xq`、`nongli` 时把当前日期、时间、星期或农历接在对应的词后面。与 `date_time`（T 模式）是两个独立开关；缺这个键的旧文档读作开——此前行内混入由 `date_time` 一并控制且默认开，保持同样的有效行为，开着时不写进文档。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub date_time_inline: bool,
    /// 日期时间混进普通候选时，第三位放一个「📅日期」入口，选中后候选窗换成这一组的全部格式（Windows `utility.date_time_menu`，出厂开）。依附 `date_time_inline`；缺这个键的旧文档读作开。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub date_time_menu: bool,
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
    /// 拆字反查（`C`）：没有组字时 Shift+C，再输入字的各个部件的拼音找字。`None` 表示用户没动过开关，由宿主形态决定：触屏宿主（iOS、Android、HarmonyOS）读作开，屏幕键盘从菜单进入；桌面宿主读作关，因为打开后句首和中英混打的大写 C（C盘、C语言）会进入模式。Android 和 HarmonyOS 接实体键盘（含鸿蒙电脑）时 Shift+C 同样会进入模式，桌面的这个问题在那里仍然存在，而且这两个宿主不提供「Shift+字母」总开关，用户只能整项关掉拆字。只有用户动过开关时文档里才有这个键，缺键的文档读作 `None`，旧版本也只在这种情况下因 `deny_unknown_fields` 读不懂文档。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chaizi: Option<bool>,
    /// 符号模式（`S`）：没有组字时 Shift+S，再输入分组码（`xl` 希腊字母、`sx` 数学、`szq` 带圈数字、`jt` 箭头）或符号名（`alpha`、`infinity`）。默认开，与 Windows 正式版的出厂配置一致；缺这个键的旧文档读作开。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub symbol: bool,
    /// 「Shift+字母进入快捷模式」总开关：关掉后没有组字时 Shift+U/T/K/E/M/J/Y/R/V/C/S 不进入快捷模式，照常打出大写字母，上面各模式自己的开关保留原值。默认开，和以前的行为一样；缺这个键的旧文档读作开。
    #[serde(default = "enabled_by_default")]
    pub shift_letter_entry: bool,
}

impl Default for LocalModePreferences {
    fn default() -> Self {
        Self {
            unicode: true,
            date_time: true,
            date_time_inline: true,
            date_time_menu: true,
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
            chaizi: None,
            symbol: true,
            shift_letter_entry: true,
        }
    }
}

/// 宿主在按键和上屏时绘制的打字特效。这是封闭集合：每种样式都内置在宿主里，特效包（`PluginKind::Effect`）只能从中选一种并在 `effect_pack` 的范围内调整参数，任何包都不能自带新样式。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectStyle {
    #[default]
    Off,
    /// 在宿主自己的候选区上短暂闪一下。
    Flash,
    /// 宿主能画火花的地方画火花，画不了的地方改为闪一下。
    Sparks,
    /// 火花、震动，外加随连击数上升而增强特效的连击。
    PowerMode,
}

impl EffectStyle {
    /// 该样式在 `msime_client_typing_effect` 应答中的编号。
    pub fn code(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Flash => 1,
            Self::Sparks => 2,
            Self::PowerMode => 3,
        }
    }
}

/// What the plugin packs do: which pack sounds and how loud, and which command tables the `/` mode reads. Everything is off in a fresh profile. Pack ids name a built-in pack or one installed under the plugins directory; a host that cannot find the named pack stays silent rather than falling back to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginPreferences {
    pub key_sound: KeySoundPreferences,
    pub commit_sound: CommitSoundPreferences,
    pub melody: MelodyPreferences,
    pub music: MusicPreferences,
    pub achievements: AchievementPreferences,
    /// Installed command-table packs the `/` mode reads, in priority order: the first pack that defines a trigger wins.
    pub command_tables: Vec<String>,
    /// The typing effect a host draws on keys and commits; `Off` draws nothing.
    pub effect_style: EffectStyle,
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
            effect_style: EffectStyle::Off,
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
        let pack = |id: &str| id.is_empty() || crate::text::safe_id(id);
        pack(&self.key_sound.pack)
            && pack(&self.melody.pack)
            && pack(&self.music.pack)
            && pack(&self.effect_pack)
            && self.key_sound.volume <= 100
            && self.music.volume <= 100
            && self.effect_intensity <= 100
            && self.command_tables.len() <= Self::MAX_COMMAND_TABLES
            && self.command_tables.iter().enumerate().all(|(index, id)| {
                crate::text::safe_id(id) && !self.command_tables[..index].contains(id)
            })
            && pack(&self.helpcode_pack_quanpin)
            && pack(&self.helpcode_pack_shuangpin)
            && self.phrase_tables.len() <= Self::MAX_PHRASE_TABLES
            && self.phrase_tables.iter().enumerate().all(|(index, id)| {
                crate::text::safe_id(id) && !self.phrase_tables[..index].contains(id)
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

/// 全新配置默认选中的音效包。
pub const DEFAULT_SOUND_PACK: &str = "default";
/// 全新配置默认选中的旋律包。
pub const DEFAULT_MELODY_PACK: &str = "twinkle";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
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
            pack: DEFAULT_SOUND_PACK.to_owned(),
            volume: 50,
        }
    }
}

/// A sound when text is committed, from the key sound pack's `commit` sample. Independent of the key sound switch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CommitSoundPreferences {
    pub enabled: bool,
}

/// The sequence pack a key plays a note of when `key_sound.mode` is `melody`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MelodyPreferences {
    pub pack: String,
}

impl Default for MelodyPreferences {
    fn default() -> Self {
        Self {
            pack: DEFAULT_MELODY_PACK.to_owned(),
        }
    }
}

/// Background music, streamed from an installed music pack while the input method is active. Off, and with no pack chosen, until the user picks one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
pub struct AchievementPreferences {
    pub enabled: bool,
}

/// 中英混输默认的补全与英文领先门槛；26 键中更短的完整词仍可选。调低会让常见简拼被英文词抢走首选（见 `.agents/notes/implemented/feature/2026-10-12-mixed-english-leads-unsegmentable-input.md`）；不读偏好的宿主固定用它（`HostPlatform::reads_mixed_input_minimum_prefix`）。
pub const DEFAULT_MIXED_INPUT_MINIMUM_PREFIX: u8 = 5;

/// 连续多语言解码支持的语言；不依赖 Engine 或宿主的输入方案枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultilingualLanguage {
    Chinese,
    English,
    Japanese,
}

/// 连续多语言输入的共享偏好。默认关闭，避免未完成接线的宿主改变行为。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MultilingualInputPreferences {
    pub enabled: bool,
    pub languages: Vec<MultilingualLanguage>,
}

impl Default for MultilingualInputPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            languages: vec![
                MultilingualLanguage::Chinese,
                MultilingualLanguage::English,
                MultilingualLanguage::Japanese,
            ],
        }
    }
}

impl MultilingualInputPreferences {
    fn is_default(&self) -> bool {
        self == &Self::default()
    }

    /// 只返回已打开且至少包含两种不同语言的集合，并按用户配置顺序去重。
    pub fn active_languages(&self) -> Vec<MultilingualLanguage> {
        if !self.enabled {
            return Vec::new();
        }
        let mut active = Vec::with_capacity(3);
        for language in &self.languages {
            if !active.contains(language) {
                active.push(*language);
            }
        }
        if active.len() >= 2 {
            active
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixedInputPreferences {
    /// 候选混输总开关（Windows `utility.mixed_candidates`）：关掉后英文、emoji、颜文字、符号、日文、快捷短语和行内日期时间都不再混进普通候选，各分开关的原值保留。后加的成员，开着时不写进文档，旧版本和没有这个键的文档都按开处理。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub enabled: bool,
    pub english: bool,
    pub minimum_prefix: u8,
    /// 中文状态下首页最多展示几个英文候选（Windows `general.cn_en_mixed_input_first_page_count`，1–3）：第一行英文进优先位置之后，其余英文紧跟它继续占首页的空位，放不下的照旧接在列表末尾。后加的成员，为 1 时不写进文档，旧版本和没有这个键的文档都按 1 处理。
    #[serde(
        default = "default_first_page_count",
        skip_serializing_if = "is_default_first_page_count"
    )]
    pub first_page_count: u8,
    pub emoji: bool,
    pub kaomoji: bool,
    /// 实验选项：全拼、双拼里给整段罗马字补一个日文转换候选（ohayou → おはよう）。后加的成员，缺省为关；关着时不写进文档，旧版本和只写前四项的宿主都还能读。
    #[serde(default, skip_serializing_if = "is_false")]
    pub japanese: bool,
    /// 打拼音出符号：全拼、双拼整段打出符号的名称时（`zhengfu`、`pingfangmi`），在候选里接着那个词给出符号（± ㎡）。后加的成员，所有宿主默认开；缺这个键的旧文档读作开，只有用户关掉时才写进文档（`"symbols": false`），开着时文档逐字节不变，旧版本和只写前四项的宿主都还能读。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub symbols: bool,
    /// 快捷短语混进普通候选（Windows `utility.quick_phrase_candidates`，出厂关）：普通组字的编码精确等于某条快捷短语的编码时，短语追加在普通候选之后。只对声明 `HostCapabilities::quick_phrase_candidates` 的宿主（macOS、Linux）生效，它们此前无条件混入，所以缺这个键的旧文档读作开；新装默认关，与 Windows 出厂一致，开着时不写进文档。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub quick_phrase: bool,
    /// 快捷短语参与调频（Windows `utility.quick_phrase_frequency`，出厂开）：常越过整组快捷短语选别的候选时整组后退，最多退到首页末位；选中组内短语时整组前移。默认开，与 Windows 出厂一致；只在混输打开时起作用，缺这个键的旧文档读作开。
    #[serde(default = "enabled_by_default", skip_serializing_if = "is_true")]
    pub quick_phrase_frequency: bool,
}

impl Default for MixedInputPreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            english: true,
            minimum_prefix: DEFAULT_MIXED_INPUT_MINIMUM_PREFIX,
            first_page_count: DEFAULT_MIXED_INPUT_FIRST_PAGE_COUNT,
            emoji: source_mixed_emoji_default(),
            kaomoji: false,
            japanese: false,
            symbols: true,
            quick_phrase: false,
            quick_phrase_frequency: true,
        }
    }
}

/// 中文状态下首页默认展示的英文候选数（Windows `cn_en_mixed_input_first_page_count` 的出厂值）。
pub const DEFAULT_MIXED_INPUT_FIRST_PAGE_COUNT: u8 = 1;

fn default_first_page_count() -> u8 {
    DEFAULT_MIXED_INPUT_FIRST_PAGE_COUNT
}

fn is_default_first_page_count(value: &u8) -> bool {
    *value == DEFAULT_MIXED_INPUT_FIRST_PAGE_COUNT
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

/// 选第二、第三个候选的那一对键。目前只有分号和单引号一种；留成枚举，是为了以后加左右 Shift 之类的键位时不必改文档格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecondThirdCandidateKeys {
    /// `;` 选第二个候选，`'` 选第三个候选。
    #[default]
    SemicolonQuote,
}

/// 缺哪个字段就取默认值，宿主的设置页只写 `enabled` 也能读。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SecondThirdCandidatePreferences {
    pub enabled: bool,
    pub keys: SecondThirdCandidateKeys,
}

impl SecondThirdCandidatePreferences {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// 这个键选当前页从零计数的第几个候选；关闭或不在键位表里时为 `None`。
    /// 与 Windows `second_third_candidate_slot` 同一张表，键位枚举目前只有 `;`/`'` 一组。
    pub fn slot(&self, value: u8) -> Option<usize> {
        if !self.enabled {
            return None;
        }
        match self.keys {
            SecondThirdCandidateKeys::SemicolonQuote => match value {
                b';' => Some(1),
                b'\'' => Some(2),
                _ => None,
            },
        }
    }
}

/// 五笔二三候选只占用两个不同的 ASCII 标点，不改变字母组码和数字选词。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WubiCandidateSelectionPreferences {
    pub enabled: bool,
    pub second_key: String,
    pub third_key: String,
}

impl Default for WubiCandidateSelectionPreferences {
    fn default() -> Self {
        Self {
            enabled: false,
            second_key: ";".into(),
            third_key: "'".into(),
        }
    }
}

impl WubiCandidateSelectionPreferences {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> bool {
        let valid = |key: &str| key.len() == 1 && key.as_bytes()[0].is_ascii_punctuation();
        valid(&self.second_key) && valid(&self.third_key) && self.second_key != self.third_key
    }

    /// 当前页中从零计数的位置；无效文档不抢占任何字符。
    pub fn slot(&self, value: u8) -> Option<usize> {
        if !self.enabled || !self.validate() {
            return None;
        }
        if self.second_key.as_bytes() == [value] {
            Some(1)
        } else if self.third_key.as_bytes() == [value] {
            Some(2)
        } else {
            None
        }
    }
}

const MAX_URL_BYTES: usize = 2048;
const MIN_TOKEN_CHARS: usize = 16;
const MAX_TOKEN_CHARS: usize = 256;

/// 中继地址是否可用：`https`（`http` 只放行 `localhost`、`127.0.0.1` 和 `[::1]`），有主机名，可以带路径前缀，不带查询串、片段和用户信息，不超过 2048 字节。
pub fn valid_relay_url(url: &str) -> bool {
    parse_relay_url(url).is_some()
}

/// 中继令牌是否可用：16 到 256 个可打印 ASCII 字符，不含空格。
pub fn valid_relay_token(token: &str) -> bool {
    (MIN_TOKEN_CHARS..=MAX_TOKEN_CHARS).contains(&token.len()) && crate::is_ascii_graphic(token)
}

#[doc(hidden)]
pub fn parse_relay_url(url: &str) -> Option<url::Url> {
    if url.is_empty() || !crate::text::is_bounded_text(url, MAX_URL_BYTES) {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    let shape = match parsed.scheme() {
        "https" => crate::endpoint::valid_endpoint_shape(url, &parsed, "https"),
        "http" => {
            crate::endpoint::valid_endpoint_shape(url, &parsed, "http")
                && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
        }
        _ => false,
    };
    (shape && !parsed.cannot_be_a_base() && parsed.query().is_none()).then_some(parsed)
}

/// 自建的云剪贴板中继：基地址和中继管理员发的令牌，两项要么都空（不用中继），要么都合规。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CloudClipboardRelayPreferences {
    /// 中继的基地址，例如 `https://relay.example.com/msime`；规则见 [`valid_relay_url`]。
    pub url: String,
    /// 中继令牌，每个令牌对应中继上一个独立的剪贴板空间；规则见 [`valid_relay_token`]。
    pub token: String,
}

impl CloudClipboardRelayPreferences {
    fn is_default(&self) -> bool {
        self.url.is_empty() && self.token.is_empty()
    }

    /// 是否配置了中继。配置了就只走中继。
    pub fn is_configured(&self) -> bool {
        !self.url.is_empty()
    }

    fn validate(&self) -> bool {
        self.is_default() || (valid_relay_url(&self.url) && valid_relay_token(&self.token))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// 不带修饰键的左右方向键在横排候选窗里移动候选高亮（左是上一个、右是下一个）。高亮在首选时 ← 照旧编辑输入串，→ 在光标没到输入串末尾时照旧往末尾移，到了末尾才选下一个；高亮不在首选时左右键只移动候选。桌面候选窗竖排（`candidate_layout`）时左右键与候选方向垂直，只编辑输入串；触屏的候选栏总是横排。与 `arrows` 各自独立，默认开启（Windows `general.candidate_horizontal_arrow_navigation` 出厂为关，默认值差异已记录在案，是否对齐留待产品决策）。
    ///
    /// 判断放在 `input-runtime` 里而不是每个宿主各写一遍：宿主只把左右键当普通光标命令发下去，端点判断和高亮移动跟着 ↑/↓ 的那段代码一起走。
    #[serde(default = "enabled_by_default")]
    pub left_right: bool,
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
            left_right: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeybindingPreferences {
    #[serde(default = "enabled_by_default")]
    pub switch_language_shift: bool,
    /// macOS 单按 Shift 只在可确认的可编辑焦点生效；依赖辅助功能权限，默认关闭。
    #[serde(default)]
    pub switch_language_shift_requires_editable: bool,
    #[serde(default)]
    pub switch_language_ctrl: bool,
    /// Linux 已默认处理这个组合键；关闭时交还应用，不改输入法框架的全局绑定。
    #[serde(default = "enabled_by_default")]
    pub switch_language_ctrl_space: bool,
    #[serde(default = "enabled_by_default")]
    pub switch_language_ctrl_alt_space: bool,
    #[serde(default = "enabled_by_default")]
    pub toggle_character_set_ctrl_shift_f: bool,
    /// macOS 的 Option+Shift+H、Windows 和 Android 的 Alt+Shift+H 切换全半角。macOS 从上线起就无条件占用这个组合键，所以默认开：这个偏好是把组合键还给应用，而不是打开一个原本关着的功能。没有这个组合键的宿主忽略它。
    #[serde(default = "enabled_by_default")]
    pub toggle_fullwidth_option_shift_h: bool,
    /// 单按 Caps Lock 切换中英文：按下后在 300 毫秒内松开、期间没有别的键，算一次轻点，切换输入法自己的中英文模式，并把大写锁定撤回关闭；按住更久照常打开大写锁定。大写锁定已经开着时按它照常关掉，不切换。默认关；只有声明 `HostCapabilities::caps_lock_language_switch` 的宿主读它。
    #[serde(default)]
    pub switch_language_caps_lock: bool,
    /// Ctrl+Shift+J 在中文和日语之间切换：中文状态下，当前是中文方案时切到日语方案，并把这个中文方案记进 `last_chinese_scheme`；当前是日语或其他非中文方案时回到 `last_chinese_scheme`。英文模式下组合键交给应用。默认关，因为浏览器把它当作打开开发者工具或下载页；只有声明 `HostCapabilities::japanese_toggle_shortcut` 的宿主读它。
    #[serde(default)]
    pub toggle_japanese_ctrl_shift_j: bool,
    /// 中日英循环切换：打开后，「切换中英文」那一组快捷键按 中 → 日 → 英 → 中 依次切换，有组字时先把正在组的原始字母上屏；英文模式下再按切换键回到中文。默认关，对齐旧版 Windows 正式版的出厂值（`installer/default_config/config.default.toml`）。没打开时这一组快捷键仍是二态切换，行为与加这个开关之前完全一样。只有声明 `HostCapabilities::trilingual_cycle` 的宿主读它；状态机在 `crate::language_cycle`。
    #[serde(default)]
    pub trilingual_cycle: bool,
    /// 中英切换（英文候选模式）热键，写法见 [`crate::key_chord::KeyChord`]；空串表示不绑定。键名沿用旧版 Windows 的 `keybindings.toggle_english_candidates`，出厂值 `Ctrl+Shift+KeyE` 不写进文档，没有这个键的旧文档照出厂值读。
    #[serde(
        default = "default_toggle_english_candidates",
        skip_serializing_if = "is_default_toggle_english_candidates"
    )]
    pub toggle_english_candidates: String,
    /// 全角 / 半角切换热键（旧版 `keybindings.toggle_full_width`），出厂值 `Ctrl+Shift+Space`；空串表示不绑定。
    #[serde(
        default = "default_toggle_full_width",
        skip_serializing_if = "is_default_toggle_full_width"
    )]
    pub toggle_full_width: String,
    /// 中英文标点切换热键（旧版 `keybindings.toggle_punctuation`），出厂值 `Ctrl+Period`；空串表示不绑定。
    #[serde(
        default = "default_toggle_punctuation",
        skip_serializing_if = "is_default_toggle_punctuation"
    )]
    pub toggle_punctuation: String,
    /// 额外的语音开始／结束热键；空串表示不绑定，不改变既有固定语音快捷键。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub toggle_voice_input: String,
}

/// 三个切换热键的出厂值，沿用旧版 Windows v0.11.1 `engine/contracts/input_state_shortcuts.h` 的 `defaultBinding`。
pub const TOGGLE_ENGLISH_CANDIDATES_DEFAULT: &str = "Ctrl+Shift+KeyE";
pub const TOGGLE_FULL_WIDTH_DEFAULT: &str = "Ctrl+Shift+Space";
pub const TOGGLE_PUNCTUATION_DEFAULT: &str = "Ctrl+Period";

impl KeybindingPreferences {
    /// `toggle_english_candidates` 解析出的 chord；空串或非法写法为 `None`，宿主按「不绑定」处理。`Preferences::validate` 保证能进文档的串都解析得出来。
    pub fn english_candidates_chord(&self) -> Option<crate::key_chord::KeyChord> {
        toggle_chord(&self.toggle_english_candidates)
    }

    /// `toggle_full_width` 解析出的 chord；语义同上。
    pub fn full_width_chord(&self) -> Option<crate::key_chord::KeyChord> {
        toggle_chord(&self.toggle_full_width)
    }

    /// `toggle_punctuation` 解析出的 chord；语义同上。
    pub fn punctuation_chord(&self) -> Option<crate::key_chord::KeyChord> {
        toggle_chord(&self.toggle_punctuation)
    }

    /// 自定义语音切换键；空串或非法写法为 `None`。
    pub fn voice_input_chord(&self) -> Option<crate::key_chord::KeyChord> {
        toggle_chord(&self.toggle_voice_input)
    }
}

/// 「不绑定」在文档里存成空串而不是缺键：缺键读回出厂值，空串才留在「不绑定」。
fn toggle_chord(text: &str) -> Option<crate::key_chord::KeyChord> {
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        text.parse().ok()
    }
}

/// 可重绑切换热键之一。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToggleHotkeySlot {
    /// `keybindings.toggle_english_candidates`。
    #[serde(rename = "toggle_english_candidates")]
    EnglishCandidates,
    /// `keybindings.toggle_full_width`。
    #[serde(rename = "toggle_full_width")]
    FullWidth,
    /// `keybindings.toggle_punctuation`。
    #[serde(rename = "toggle_punctuation")]
    Punctuation,
    /// `keybindings.toggle_voice_input`。
    #[serde(rename = "toggle_voice_input")]
    VoiceInput,
}

/// 一个组合键已被谁占用：另一个切换热键，或某个开关随附的快捷键 / macOS 维护键。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToggleChordOwner {
    EnglishCandidates,
    FullWidth,
    Punctuation,
    VoiceInput,
    /// `keybindings.switch_language_shift` 打开时 macOS 同时启用 `Shift+Space`。
    SwitchLanguageShift,
    /// `keybindings.switch_language_ctrl_space` 打开时的 `Ctrl+Space`。
    SwitchLanguageCtrlSpace,
    /// `keybindings.switch_language_ctrl_alt_space` 打开时的 `Ctrl+Alt+Space`。
    SwitchLanguageCtrlAltSpace,
    /// `keybindings.toggle_character_set_ctrl_shift_f` 打开时的 `Ctrl+Shift+KeyF`。
    ToggleCharacterSetCtrlShiftF,
    /// `keybindings.toggle_japanese_ctrl_shift_j` 打开时的 `Ctrl+Shift+KeyJ`。
    ToggleJapaneseCtrlShiftJ,
    /// macOS 独立的 `Alt+Shift+KeyH` 全半角入口。
    ToggleFullwidthOptionShiftH,
    /// macOS 无开关的 `Ctrl+Shift+Meta+KeyK` 屏幕键盘入口。
    MacosScreenKeyboard,
    /// `voice_input.hotkey_ctrl_f9` 打开时的 `Ctrl+F9` 与 `Ctrl+Shift+F9`（Windows 的语音开关只看 Ctrl 按住、不看 Shift）。
    VoiceHotkey,
    /// macOS 无开关的维护键 `Ctrl+Alt+Shift+KeyC/R/T`。
    MacosMaintenance,
}

impl ToggleChordOwner {
    /// 跨进程回传给设置页的稳定标识：切换热键用文档字段名，开关占用用开关字段名，其余用 `voice_hotkey` / `macos_maintenance`。
    pub fn id(self) -> &'static str {
        match self {
            Self::EnglishCandidates => "toggle_english_candidates",
            Self::FullWidth => "toggle_full_width",
            Self::Punctuation => "toggle_punctuation",
            Self::VoiceInput => "toggle_voice_input",
            Self::SwitchLanguageShift => "switch_language_shift",
            Self::SwitchLanguageCtrlSpace => "switch_language_ctrl_space",
            Self::SwitchLanguageCtrlAltSpace => "switch_language_ctrl_alt_space",
            Self::ToggleCharacterSetCtrlShiftF => "toggle_character_set_ctrl_shift_f",
            Self::ToggleJapaneseCtrlShiftJ => "toggle_japanese_ctrl_shift_j",
            Self::ToggleFullwidthOptionShiftH => "toggle_fullwidth_option_shift_h",
            Self::MacosScreenKeyboard => "macos_screen_keyboard",
            Self::VoiceHotkey => "voice_hotkey",
            Self::MacosMaintenance => "macos_maintenance",
        }
    }
}

/// 试写一个切换热键不通过的结论。
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToggleHotkeyRejection {
    /// 写法不是合法 chord。
    #[error("{0}")]
    Invalid(crate::key_chord::KeyChordError),
    /// 与 `owner` 占用的组合键相同（`chord` 是归一后的写法）。
    #[error("conflicts with {owner:?}")]
    Conflict {
        owner: ToggleChordOwner,
        chord: crate::key_chord::KeyChord,
    },
}

impl KeybindingPreferences {
    /// 切换热键的槽位与当前 chord：空串或非法写法为 `None`（不绑定）。
    fn toggle_chords(&self) -> [(ToggleHotkeySlot, Option<crate::key_chord::KeyChord>); 4] {
        [
            (
                ToggleHotkeySlot::EnglishCandidates,
                self.english_candidates_chord(),
            ),
            (ToggleHotkeySlot::FullWidth, self.full_width_chord()),
            (ToggleHotkeySlot::Punctuation, self.punctuation_chord()),
            (ToggleHotkeySlot::VoiceInput, self.voice_input_chord()),
        ]
    }

    /// 切换热键不能绑的占用键：随开关开合的快捷键加 macOS 无开关的维护键（`InputControllerPhysicalKeys.h`
    /// 的 Ctrl+Shift+Option+C/R/T，对应 Alt 写作 Option 的一面），绑了它在 macOS 上会被维护动作吃掉。
    /// `voice_hotkey_ctrl_f9` 是 `voice_input.hotkey_ctrl_f9`：Windows 的语音开关只看 Ctrl 按住、不看 Shift，
    /// `Ctrl+Shift+F9` 同样触发，所以两个写法都占。
    fn occupied_toggle_chords(
        &self,
        voice_hotkey_ctrl_f9: bool,
    ) -> Vec<(crate::key_chord::KeyChord, ToggleChordOwner)> {
        let mut occupied = Vec::new();
        for (enabled, text, owner) in [
            (
                self.switch_language_shift,
                "Shift+Space",
                ToggleChordOwner::SwitchLanguageShift,
            ),
            (
                self.toggle_fullwidth_option_shift_h,
                "Alt+Shift+KeyH",
                ToggleChordOwner::ToggleFullwidthOptionShiftH,
            ),
            (
                true,
                "Ctrl+Shift+Meta+KeyK",
                ToggleChordOwner::MacosScreenKeyboard,
            ),
            (
                self.switch_language_ctrl_space,
                "Ctrl+Space",
                ToggleChordOwner::SwitchLanguageCtrlSpace,
            ),
            (
                self.switch_language_ctrl_alt_space,
                "Ctrl+Alt+Space",
                ToggleChordOwner::SwitchLanguageCtrlAltSpace,
            ),
            (
                self.toggle_character_set_ctrl_shift_f,
                "Ctrl+Shift+KeyF",
                ToggleChordOwner::ToggleCharacterSetCtrlShiftF,
            ),
            (
                self.toggle_japanese_ctrl_shift_j,
                "Ctrl+Shift+KeyJ",
                ToggleChordOwner::ToggleJapaneseCtrlShiftJ,
            ),
            (
                voice_hotkey_ctrl_f9,
                "Ctrl+F9",
                ToggleChordOwner::VoiceHotkey,
            ),
            (
                voice_hotkey_ctrl_f9,
                "Ctrl+Shift+F9",
                ToggleChordOwner::VoiceHotkey,
            ),
            (
                true,
                "Ctrl+Alt+Shift+KeyC",
                ToggleChordOwner::MacosMaintenance,
            ),
            (
                true,
                "Ctrl+Alt+Shift+KeyR",
                ToggleChordOwner::MacosMaintenance,
            ),
            (
                true,
                "Ctrl+Alt+Shift+KeyT",
                ToggleChordOwner::MacosMaintenance,
            ),
        ] {
            if enabled {
                occupied.push((text.parse().expect("保留表里的写法必须是合法 chord"), owner));
            }
        }
        occupied
    }

    /// 把 `text` 作为 `slot` 的绑定试写：通过时回规范写法（空串回空，表示不绑定）；
    /// 不通过时给原因，冲突经 `owner` 指出占用它的是哪一项。
    pub fn check_toggle_hotkey(
        &self,
        slot: ToggleHotkeySlot,
        text: &str,
        voice_hotkey_ctrl_f9: bool,
    ) -> Result<String, ToggleHotkeyRejection> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(String::new());
        }
        let chord: crate::key_chord::KeyChord =
            text.parse().map_err(ToggleHotkeyRejection::Invalid)?;
        for (other, bound) in self.toggle_chords() {
            if other == slot {
                continue;
            }
            if bound.is_some_and(|bound| chord.conflicts_with(&bound)) {
                return Err(ToggleHotkeyRejection::Conflict {
                    owner: match other {
                        ToggleHotkeySlot::EnglishCandidates => ToggleChordOwner::EnglishCandidates,
                        ToggleHotkeySlot::FullWidth => ToggleChordOwner::FullWidth,
                        ToggleHotkeySlot::Punctuation => ToggleChordOwner::Punctuation,
                        ToggleHotkeySlot::VoiceInput => ToggleChordOwner::VoiceInput,
                    },
                    chord,
                });
            }
        }
        for (occupied, owner) in self.occupied_toggle_chords(voice_hotkey_ctrl_f9) {
            if chord.conflicts_with(&occupied) {
                return Err(ToggleHotkeyRejection::Conflict { owner, chord });
            }
        }
        Ok(chord.to_string())
    }

    /// 切换热键的写法归一成规范形；不是 chord 的原样留下，交给 `Preferences::validate` 拒绝。
    pub fn normalize_toggle_hotkeys(&mut self) {
        for text in [
            &mut self.toggle_english_candidates,
            &mut self.toggle_full_width,
            &mut self.toggle_punctuation,
            &mut self.toggle_voice_input,
        ] {
            if let Ok(chord) = text.trim().parse::<crate::key_chord::KeyChord>() {
                *text = chord.to_string();
            }
        }
    }
}

impl Default for KeybindingPreferences {
    fn default() -> Self {
        Self {
            switch_language_shift: true,
            switch_language_shift_requires_editable: false,
            switch_language_ctrl: false,
            switch_language_ctrl_space: true,
            switch_language_ctrl_alt_space: true,
            toggle_character_set_ctrl_shift_f: true,
            toggle_fullwidth_option_shift_h: true,
            switch_language_caps_lock: false,
            toggle_japanese_ctrl_shift_j: false,
            trilingual_cycle: false,
            toggle_english_candidates: default_toggle_english_candidates(),
            toggle_full_width: default_toggle_full_width(),
            toggle_punctuation: default_toggle_punctuation(),
            toggle_voice_input: String::new(),
        }
    }
}

fn default_toggle_english_candidates() -> String {
    TOGGLE_ENGLISH_CANDIDATES_DEFAULT.into()
}

fn default_toggle_full_width() -> String {
    TOGGLE_FULL_WIDTH_DEFAULT.into()
}

fn default_toggle_punctuation() -> String {
    TOGGLE_PUNCTUATION_DEFAULT.into()
}

fn is_default_toggle_english_candidates(value: &str) -> bool {
    value == TOGGLE_ENGLISH_CANDIDATES_DEFAULT
}

fn is_default_toggle_full_width(value: &str) -> bool {
    value == TOGGLE_FULL_WIDTH_DEFAULT
}

fn is_default_toggle_punctuation(value: &str) -> bool {
    value == TOGGLE_PUNCTUATION_DEFAULT
}

fn enabled_by_default() -> bool {
    true
}

fn default_candidate_fixed_badge_style() -> String {
    "paperclip".to_string()
}

fn default_candidate_ai_badge_style() -> String {
    "robot".to_string()
}

fn disabled_by_default() -> bool {
    false
}

fn default_clipboard_history_limit() -> usize {
    crate::clipboard::DEFAULT_CLIPBOARD_HISTORY_LIMIT
}

/// 智能标点在所有平台上默认开：中文模式下打 `0.10.1` 得到的是 `0.10.1`，不是 `0。10。1`。同一个默认也给重复标点转中文、数字后直出和字母后直出；空格转换不跟着开（见 `smart_punctuation_space_convert`）。
///
/// 以前 Windows 和 macOS 默认关，照的是来源产品的安装模板（`platforms/windows/installer/config.default.toml` 五个开关全是 `false`），理由是它会改写一个用户已经看见落下去的字符，句末是数字时（「共计 100。」）句号也会变成 `.`。现在默认开，靠的是重复标点转中文这条退路：输出 ASCII 之后两秒内再按同一个键，就换回中文标点，「100。」只多按一下；而数字后的 `.` 被改成「。」时，默认关的一侧没有对应的退路。用户能找到的开关不该是这种基本行为的前提。决定和取舍见 `.agents/notes/implemented/feature/2026-10-16-smart-punctuation-default-on.md`。
///
/// 这个函数只回答「文档里还没有这个键」。旧的 macOS、Windows 文档里存着以前默认的 `false`，由 [`Preferences::settle_smart_punctuation_default`] 在读入时一次性改过来。
fn smart_punctuation_default() -> bool {
    true
}

/// macOS 和 Windows 以前把智能标点这一族默认关，读入没有 `smart_punctuation_migrated` 标记的旧文档时要一次性打开；其余平台一直默认开，旧文档里的 `false` 就是用户自己关的，不动。
const SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT: bool = cfg!(any(windows, target_os = "macos"));

impl Preferences {
    /// 读入偏好文档时调用一次：文档还没有 `smart_punctuation_migrated` 标记，就是这次默认改变之前写下的。`was_off_by_default` 为真（macOS、Windows，见 [`SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT`]）时把智能标点、重复标点转中文、数字后直出和字母后直出打开；不论平台都标上标记，之后再读不会重来，用户之后关掉的值原样保留。空格转换不动。只改内存里读到的这份，文件等下一次真正的保存才带上新值和标记，在那之前每次读都得到同样的结果。
    fn settle_smart_punctuation_default(&mut self, was_off_by_default: bool) {
        if self.smart_punctuation_migrated {
            return;
        }
        if was_off_by_default {
            self.smart_punctuation = true;
            self.smart_punctuation_repeat = true;
            self.smart_punctuation_direct_digit = true;
            self.smart_punctuation_direct_letter = true;
        }
        self.smart_punctuation_migrated = true;
    }
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

/// 快捷符号的统一默认值，空配置和无有效字符的输入均恢复这份列表。
pub fn default_touch_quick_symbols() -> Vec<String> {
    ["，", "。", "？", "！", "、", "；", "："]
        .map(str::to_owned)
        .to_vec()
}

/// 规范化原生设置提交的单字素列表；保留组合 emoji，过滤控制字符与空白。
pub fn normalize_touch_quick_symbols(symbols: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for symbol in symbols {
        let value = symbol.trim();
        if value.graphemes(true).count() != 1
            || value.chars().any(|c| c.is_whitespace() || c.is_control())
            || normalized.iter().any(|stored| stored == value)
        {
            continue;
        }
        normalized.push(value.to_owned());
        if normalized.len() == 12 {
            break;
        }
    }
    if normalized.is_empty() {
        default_touch_quick_symbols()
    } else {
        normalized
    }
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
            candidate_window_fit_content: false,
            default_ime_mode: DefaultImeMode::default(),
            ime_mode_scope: ImeModeScope::default(),
            app_input_mode_rules: BTreeMap::new(),
            vim_mode_apps: Vec::new(),
            ai_assistant: AiAssistantPreferences::default(),
            sentence_association: SentenceAssociationPreferences::default(),
            next_word_prediction: false,
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
            global_theme: crate::theme::GlobalTheme::default(),
            custom_theme: CustomTheme::default(),
            candidate_layout: CandidateLayout::default(),
            candidate_preedit_style: CandidatePreeditStyle::default(),
            show_candidate_page_number: true,
            tsf_preedit_style: PreeditStyle::default(),
            diagnostic_log: DiagnosticLogPreferences::default(),
            candidate_follow_cursor: true,
            input_mode_hud: true,
            input_mode_focus_hud: false,
            input_mode_corner_hud: true,
            input_mode_status_item: false,
            show_app_logo: false,
            expand_candidates: false,
            show_candidate_expand_button: true,
            scheme: InputScheme::default(),
            wubi_code_hint: true,
            wubi_auto_commit_unique: true,
            wubi_z_wildcard: false,
            wubi_fifth_code_top_commit: true,
            wubi_mixed_pinyin: false,
            wubi_profile: WubiProfile::default(),
            touch_keyboard_layout: TouchKeyboardLayout::default(),
            touch_keyboard_split_portrait: false,
            touch_shuangpin_layout: TouchShuangpinLayout::default(),
            touch_keyboard_schemes: TouchKeyboardSchemePreferences::default(),
            touch_key_spacing_tenths: default_touch_key_spacing_tenths(),
            android_text_trackpad: TextTrackpadPreferences::default(),
            touch_row_spacing_tenths: default_touch_row_spacing_tenths(),
            touch_keyboard_height_adjustment: 0,
            touch_voice_shortcut: false,
            touch_number_keypad_order: NumberKeypadOrder::default(),
            touch_text_edit_button_order: default_text_edit_button_order(),
            touch_twenty_six_key_number_layout: TwentySixKeyNumberLayout::default(),
            touch_shuangpin_key_hints: false,
            touch_japanese_toggle_input: true,
            touch_spelling_position: TouchSpellingPosition::default(),
            touch_quick_punctuation_symbols: default_touch_quick_symbols(),
            touch_toolbar: TouchToolbarPreferences::default(),
            harmony_toolbar: HarmonyToolbarPreferences::default(),
            last_chinese_scheme: None,
            shuangpin_profile: ShuangpinProfile::default(),
            shuangpin_custom_profile: ShuangpinCustomProfile::default(),
            nine_key_shuangpin_profile: NineKeyShuangpinProfile::default(),
            nine_key_shuangpin_custom_profile: ShuangpinCustomProfile::default(),
            shuangpin_preedit_uses_raw: true,
            shuangpin_keymap_hint: None,
            vietnamese: VietnamesePreferences::default(),
            korean: KoreanPreferences::default(),
            single_character_only: false,
            candidate_page_size: 6,
            number_row_selection: true,
            candidate_font_size: default_candidate_font_size(),
            candidate_preedit_font_size: default_candidate_preedit_font_size(),
            candidate_translation_font_size: None,
            candidate_scale_percent: default_candidate_scale_percent(),
            candidate_opacity_percent: default_candidate_opacity_percent(),
            candidate_corner_radius: None,
            candidate_padding: None,
            candidate_padding_horizontal: None,
            candidate_padding_vertical: None,
            candidate_item_padding_horizontal: None,
            candidate_item_padding_vertical: None,
            candidate_item_corner_radius: None,
            candidate_item_spacing: None,
            candidate_font_family: default_candidate_font_family(),
            candidate_english_font: None,
            candidate_fallback_fonts: default_candidate_fallback_fonts(),
            learning: true,
            enter_learns_english_word: true,
            quanpin: QuanpinPreferences::default(),
            fuzzy_pinyin: FuzzyPinyinPreferences::default(),
            quanpin_helpcode: default_quanpin_helpcode(),
            shuangpin_helpcode: default_shuangpin_helpcode(),
            traditional_chinese_output: false,
            traditional_chinese_variant: TraditionalVariant::default(),
            chinese_english_spacing: false,
            cross_commit_spacing: false,
            cross_commit_spacing_excluded_apps: Vec::new(),
            chinese_punctuation: true,
            smart_punctuation: smart_punctuation_default(),
            smart_punctuation_repeat: smart_punctuation_default(),
            smart_punctuation_space_convert: false,
            smart_punctuation_direct_digit: smart_punctuation_default(),
            smart_punctuation_direct_letter: smart_punctuation_default(),
            smart_punctuation_migrated: true,
            paired_punctuation: true,
            punctuation_lock: PunctuationLock::Follow,
            caps_lock_ascii_punctuation: false,
            quote_style: QuoteStyle::Curly,
            bracket_style: BracketStyle::Solid,
            enumeration_comma_backslash: true,
            enumeration_comma_slash: false,
            navigation: NavigationPreferences::default(),
            keybindings: KeybindingPreferences::default(),
            word_character: WordCharacterPreferences::default(),
            second_third_candidate: SecondThirdCandidatePreferences::default(),
            wubi_candidate_selection: WubiCandidateSelectionPreferences::default(),
            backspace_reverts_selection: true,
            escape_keeps_selected_word: false,
            frequency: FrequencyPreferences::default(),
            mixed_input: MixedInputPreferences::default(),
            multilingual_input: MultilingualInputPreferences::default(),
            local_modes: LocalModePreferences::default(),
            plugins: PluginPreferences::default(),
            game_compatibility: GameCompatibilityPreferences::default(),
            clipboard_history: false,
            clipboard_history_limit: default_clipboard_history_limit(),
            cloud_clipboard_relay: CloudClipboardRelayPreferences::default(),
            // 会把输入内容发出设备的两条路径新装都关闭，由 Windows、macOS、Linux 的首次询问或各平台的设置开关打开。
            cloud_candidates: false,
            cloud_candidate_provider: Default::default(),
            network: crate::network::NetworkPreferences::default(),
            candidate_translations: false,
            candidate_english_gloss: false,
            candidate_pronunciation: false,
            // 候选释义与角标的显示默认值对齐 Windows v0.11.1 的出厂配置。
            candidate_translation_show_pos: true,
            candidate_translation_single_sense: false,
            candidate_fixed_badge: true,
            candidate_fixed_badge_style: default_candidate_fixed_badge_style(),
            candidate_ai_badge_style: default_candidate_ai_badge_style(),
            english_suggestions: true,
            english_online_candidates: false,
            translation_target_language: TranslationTargetLanguage::default(),
            translation_secondary_language: None,
            translation_account: false,
            translation_ai: false,
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

fn is_default_traditional_variant(value: &TraditionalVariant) -> bool {
    *value == TraditionalVariant::default()
}

fn is_true(value: &bool) -> bool {
    *value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
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
    SmartAbc,
    /// 用户自己的键位表，见 `Preferences::shuangpin_custom_profile`。
    Custom,
}

/// 九键双拼方案（#6572）。Engine 的编码是声明顺序（`NineKeyShuangpinKind`）；内置的三个方案和它们的来源见 `.agents/notes/implemented/feature/2026-10-10-nine-key-shuangpin.md`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NineKeyShuangpinProfile {
    /// 米旮旯 2。
    Migala2,
    /// 米旮旯 3。
    #[default]
    Migala3,
    /// 辜氏 2 低重码。
    Gushi2Low,
    /// 用户自己的表，见 `Preferences::nine_key_shuangpin_custom_profile`。
    Custom,
}

impl NineKeyShuangpinProfile {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// 自定义双拼方案的键位表，与 Engine 的 `ShuangpinCustomTable` 一一对应：`initials` 只放 `zh` `ch` `sh`，`finals` 放 33 个韵母，`zero_initials` 放 12 个零声母音节的两键编码；键是小写字母或 `;`，`ü` 写作 `v`。导入导出用的也是这个 JSON 形状。
///
/// 九键双拼的自定义表（`Preferences::nine_key_shuangpin_custom_profile`）用同一个形状，对应 Engine 的 `NineKeyShuangpinTable`：`initials` 放全部 23 个声母，键是数字。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ShuangpinCustomProfile {
    #[serde(default)]
    pub initials: BTreeMap<String, String>,
    #[serde(default)]
    pub finals: BTreeMap<String, String>,
    #[serde(default)]
    pub zero_initials: BTreeMap<String, String>,
}

impl ShuangpinCustomProfile {
    /// 一张合法的表最多有 3 + 33 + 12 个单位；留出余量，只挡住明显不是键位表的东西。
    const MAX_UNITS_PER_PART: usize = 64;
    const MAX_UNIT_BYTES: usize = 8;
    const MAX_KEY_BYTES: usize = 4;

    pub fn is_empty(&self) -> bool {
        self.initials.is_empty() && self.finals.is_empty() && self.zero_initials.is_empty()
    }

    /// 只看大小和字符，不看键位合不合法。
    fn validate(&self) -> bool {
        [&self.initials, &self.finals, &self.zero_initials]
            .into_iter()
            .all(|part| {
                part.len() <= Self::MAX_UNITS_PER_PART
                    && part.iter().all(|(unit, key)| {
                        unit.len() <= Self::MAX_UNIT_BYTES
                            && key.len() <= Self::MAX_KEY_BYTES
                            && unit.bytes().all(|byte| byte.is_ascii_lowercase())
                            && key.bytes().all(|byte| byte.is_ascii_graphic())
                    })
            })
    }
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
    /// 五笔 86：每个字取 86 五笔全码的前两码（`resources/helpcodes/wubi86_helpcode.txt`）。
    Wubi86,
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
            Self::Wubi86 => "wubi86",
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
            "wubi86" => Ok(Self::Wubi86),
            value if crate::helpcode::is_custom_schema(value) => Ok(Self::Custom(value.into())),
            _ => Err(serde::de::Error::custom("unknown helpcode schema")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelpcodePreferences {
    /// Linux 的辅助码前导键；空串保留 Shift 加字母。
    #[serde(default)]
    pub trigger_key: String,
    /// Windows `shuangpin_helpcode`/`quanpin_helpcode` 出厂关、门禁 pin 出厂开，本仓沿用开；缺这个键的旧文档读作开。
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    pub schema: HelpcodeSchema,
    #[serde(default = "enabled_by_default")]
    pub show_in_candidate_window: bool,
    /// 双拼专属的辅助码触发方式，对应 Windows v0.11.1 `[helpcode]` 组里 `shuangpin_helpcode_direct` 起的十一枚键；`quanpin_helpcode` 不读这一段。等于默认值时不写进文档，旧文档缺这一段时按 [`ShuangpinHelpcodeModes::default`] 处理。
    #[serde(default, skip_serializing_if = "ShuangpinHelpcodeModes::is_default")]
    pub modes: ShuangpinHelpcodeModes,
}

impl Default for HelpcodePreferences {
    fn default() -> Self {
        Self {
            enabled: true,
            schema: HelpcodeSchema::default(),
            trigger_key: String::new(),
            show_in_candidate_window: true,
            modes: ShuangpinHelpcodeModes::default(),
        }
    }
}

/// 双拼辅助码的触发方式（Windows v0.11.1 `installer/default_config/config.default.toml` 的 `[helpcode]` 组）：末尾直接、末尾间接的三枚触发键、句中间接及它的三枚触发键、万象式直接及它的两枚四码结束标记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShuangpinHelpcodeModes {
    /// 只在 Tab 后输入辅助码；保留其它触发方式的设置，关闭后恢复。
    pub manual_tab: bool,
    /// 末尾直接（`shuangpin_helpcode_direct`）：完整音节基座后直接接辅码，`ulpbx` 是单码、`ulpbxY` 是双码；关闭后末尾裸字母不再当辅助码，只有触发键开启的段才认。
    pub direct: bool,
    /// 末尾间接触发键：反引号段（`shuangpin_helpcode_trigger_backtick`）。
    pub trigger_backtick: bool,
    /// 末尾间接触发键：分号段（`shuangpin_helpcode_trigger_semicolon`）。
    pub trigger_semicolon: bool,
    /// 末尾间接触发键：段首大写（`shuangpin_helpcode_trigger_uppercase`）。
    pub trigger_uppercase: bool,
    /// 句中（间接）辅助码总开关（`shuangpin_mid_sentence_helpcode`）：完整音节后敲触发键，后面的字母筛这个音节上的字。
    pub mid_sentence: bool,
    /// 句中触发键：反引号（`shuangpin_mid_sentence_helpcode_backtick`）。
    pub mid_sentence_backtick: bool,
    /// 句中触发键：分号（`shuangpin_mid_sentence_helpcode_semicolon`）。
    pub mid_sentence_semicolon: bool,
    /// 句中触发键：完整音节后的大写字母（`shuangpin_mid_sentence_helpcode_uppercase`）。
    pub mid_sentence_uppercase: bool,
    /// 句中（直接）「万象式」总开关（`shuangpin_direct_helpcode`）：逐音节解析，四码单独敲时直接定字。
    pub direct_helpcode: bool,
    /// 万象式四码句中用 `/` 结束（`shuangpin_direct_helpcode_slash`）。
    pub direct_helpcode_slash: bool,
    /// 万象式四码句中用第二位辅码大写结束（`shuangpin_direct_helpcode_uppercase`）。
    pub direct_helpcode_uppercase: bool,
}

/// 默认值与 Windows v0.11.1 的出厂配置一致：末尾直接开、末尾间接的分号开，句中间接与万象式直接关。
impl Default for ShuangpinHelpcodeModes {
    fn default() -> Self {
        Self {
            manual_tab: false,
            direct: true,
            trigger_backtick: false,
            trigger_semicolon: true,
            trigger_uppercase: false,
            mid_sentence: false,
            mid_sentence_backtick: true,
            mid_sentence_semicolon: false,
            mid_sentence_uppercase: false,
            direct_helpcode: false,
            direct_helpcode_slash: true,
            direct_helpcode_uppercase: false,
        }
    }
}

impl ShuangpinHelpcodeModes {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// 写入端归一：句中间接开着时，同一枚触发键只能归句中、不再算末尾间接；`mid_sentence` 与 `direct_helpcode` 不能同时为真，都真时按万象式直接（句中让位）。
    pub fn normalized(mut self) -> Self {
        if self.direct_helpcode {
            self.mid_sentence = false;
        }
        if self.mid_sentence {
            self.trigger_backtick &= !self.mid_sentence_backtick;
            self.trigger_semicolon &= !self.mid_sentence_semicolon;
            self.trigger_uppercase &= !self.mid_sentence_uppercase;
        }
        self
    }
}

fn default_quanpin_helpcode() -> HelpcodePreferences {
    HelpcodePreferences {
        enabled: true,
        schema: HelpcodeSchema::Ziranma,
        trigger_key: String::new(),
        show_in_candidate_window: false,
        modes: ShuangpinHelpcodeModes::default(),
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

/// 判断 `voice_input.asr_model_path` 能否保存：空字符串，或不超过 4096 字节、不含控制字符的绝对路径（[`is_absolute_model_path`]）。
pub fn valid_model_path(path: &str) -> bool {
    crate::text::is_bounded_text(path, 4096) && (path.is_empty() || is_absolute_model_path(path))
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
    let Ok(url) = url::Url::parse(mirror) else {
        return false;
    };
    crate::endpoint::valid_endpoint_shape(mirror, &url, "https") && url.query().is_none()
}

fn default_shuangpin_helpcode() -> HelpcodePreferences {
    HelpcodePreferences {
        enabled: true,
        schema: HelpcodeSchema::Lantian,
        trigger_key: String::new(),
        show_in_candidate_window: true,
        modes: ShuangpinHelpcodeModes::default(),
    }
}

/// Persisted recognition provider identifiers. Hosts expose only the providers they implement: `system` is the platform speech adapter, not a cloud profile, and `local` is an installed on-device sherpa-onnx model directory named by `asr_model_path`, which needs a host built with the recognizer behind it.
pub const ASR_PROVIDERS: [&str; 12] = [
    "doubao",
    "siliconflow",
    "openai",
    "groq",
    "everyapi",
    "mistral",
    "bailian",
    "zhipu",
    "minimax",
    "custom",
    "system",
    "local",
];
/// OpenAI-compatible AI services exposed by the Apple settings surface and
/// shared by every host. Providers that need special request fields are still
/// handled in `ai.rs`; the rest use the common Chat Completions shape.
pub const AI_PROVIDERS: [&str; 15] = [
    "everyapi",
    "openai",
    "anthropic",
    "gemini",
    "deepseek",
    "qwen",
    "kimi",
    "zhipu",
    "minimax",
    "volcengine",
    "siliconflow",
    "groq",
    "openrouter",
    "ollama",
    "custom",
];
/// Polishing additionally supports DeepSeek, which offers no recognition, and
/// accepts doubao plus legacy ids the catalog resolves without listing.
pub const POLISH_PROVIDERS: [&str; 12] = [
    "siliconflow",
    "openai",
    "deepseek",
    "groq",
    "doubao",
    "minimax",
    "kimi",
    "zhipu",
    "qwen",
    "volcengine",
    "openrouter",
    "custom",
];

impl Preferences {
    pub fn active_helpcode(&self) -> HelpcodePreferences {
        match self.scheme {
            InputScheme::Shuangpin => self.shuangpin_helpcode.clone(),
            InputScheme::Quanpin => self.quanpin_helpcode.clone(),
            _ => HelpcodePreferences {
                enabled: false,
                modes: ShuangpinHelpcodeModes::default(),
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

        // 中继的地址和令牌成对保留，理由与上面的服务地址相同。
        next.cloud_clipboard_relay = self.cloud_clipboard_relay.clone();
        next.cloud_candidate_provider = self.cloud_candidate_provider.clone();
        // 代理是为了让这台机器连得上服务：恢复默认设置把服务凭据留下了，代理地址跟着留下，否则所有出口会一起断。
        next.network = self.network.clone();

        next.fuzzy_pinyin.seeded = self.fuzzy_pinyin.seeded;

        next
    }

    /// 偏好里的全部服务凭据（token、密钥、应用 id），按文档里的路径列出。恢复默认设置时原样保留的、导出诊断包时换成 [`REDACTED`] 的，都是这一份清单；`preferences/tests.rs` 用字段名兜底，新加的凭据字段不进清单测试就失败。
    pub fn credential_slots(&mut self) -> [(&'static str, CredentialSlot<'_>); 13] {
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
            (
                "cloud_clipboard_relay.token",
                CredentialSlot::Text(&mut self.cloud_clipboard_relay.token),
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
        // 自定义地址、请求头和模板都可能含不以敏感词命名的令牌，整组不进入诊断。
        value["cloud_candidate_provider"] = REDACTED.into();
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
        let expected = default_text_edit_button_order();
        if self.touch_text_edit_button_order.len() != expected.len()
            || !expected
                .iter()
                .all(|button| self.touch_text_edit_button_order.contains(button))
        {
            return Err(PreferencesError::InvalidTextEditButtonOrder);
        }
        for helpcode in [&self.quanpin_helpcode, &self.shuangpin_helpcode] {
            if !matches!(
                helpcode.trigger_key.as_str(),
                "" | ";" | "/" | "\\" | "`" | "="
            ) {
                return Err(PreferencesError::InvalidHelpcodeTriggerKey);
            }
        }
        if !self.cloud_candidate_provider.validate() {
            return Err(PreferencesError::InvalidCloudCandidateProvider);
        }
        if !crate::clipboard::CLIPBOARD_HISTORY_LIMITS.contains(&self.clipboard_history_limit) {
            return Err(PreferencesError::InvalidClipboardHistoryLimit);
        }
        if !self.wubi_candidate_selection.validate() {
            return Err(PreferencesError::InvalidWubiCandidateSelection);
        }
        if self.wubi_candidate_selection.enabled {
            let keys = [
                &self.wubi_candidate_selection.second_key,
                &self.wubi_candidate_selection.third_key,
            ];
            let edge_keys = match self.word_character.keys {
                WordCharacterKeys::Brackets => "[]",
                WordCharacterKeys::MinusEqual => "-=",
            };
            if keys.iter().any(|key| {
                (self.navigation.comma_period && ",.".contains(key.as_str()))
                    || (self.navigation.minus_equal && "-=".contains(key.as_str()))
                    || (self.navigation.brackets && "[]".contains(key.as_str()))
                    || (self.word_character.enabled && edge_keys.contains(key.as_str()))
            }) {
                return Err(PreferencesError::ConflictingKeyBindings);
            }
        }
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
        if !self.network.proxy_server.is_empty()
            && !crate::network::valid_proxy_server(&self.network.proxy_server)
        {
            return Err(PreferencesError::InvalidNetwork);
        }
        if !ASR_PROVIDERS.contains(&self.voice_input.asr_provider.as_str())
            || !POLISH_PROVIDERS.contains(&self.voice_input.polish_provider.as_str())
            || !valid_model_path(&self.voice_input.asr_model_path)
            || !valid_model_mirror(&self.voice_input.asr_model_mirror)
        {
            return Err(PreferencesError::InvalidVoiceInput);
        }
        if !(50..=200).contains(&self.floating_toolbar.scale_percent)
            || !(12..=48).contains(&self.floating_toolbar.font_size)
        {
            return Err(PreferencesError::InvalidFloatingToolbar);
        }
        if !(1..=8).contains(&self.mixed_input.minimum_prefix)
            || !(1..=3).contains(&self.mixed_input.first_page_count)
        {
            return Err(PreferencesError::InvalidMixedInput);
        }
        if !self.plugins.validate() {
            return Err(PreferencesError::InvalidPlugins);
        }
        if !self.shuangpin_custom_profile.validate() {
            return Err(PreferencesError::InvalidShuangpinCustomProfile);
        }
        if !self.nine_key_shuangpin_custom_profile.validate() {
            return Err(PreferencesError::InvalidNineKeyShuangpinCustomProfile);
        }
        if !self.game_compatibility.validate() {
            return Err(PreferencesError::InvalidGameCompatibility);
        }
        if !valid_app_input_mode_rules(&self.app_input_mode_rules) {
            return Err(PreferencesError::InvalidAppInputModeRules);
        }
        if !valid_app_identifier_list(&self.vim_mode_apps) {
            return Err(PreferencesError::InvalidVimModeApps);
        }
        if !valid_app_identifier_list(&self.cross_commit_spacing_excluded_apps) {
            return Err(PreferencesError::InvalidCrossCommitSpacingApps);
        }
        if !self.cloud_clipboard_relay.validate() {
            return Err(PreferencesError::InvalidCloudClipboardRelay);
        }
        if !(1..=10).contains(&self.frequency.trigger_count)
            || !(1..=10).contains(&self.frequency.linear_step)
        {
            return Err(PreferencesError::InvalidFrequency);
        }
        if !(1..=MAX_CANDIDATE_PAGE_SIZE).contains(&self.candidate_page_size) {
            return Err(PreferencesError::InvalidPageSize);
        }
        if !self.android_text_trackpad.validate() {
            return Err(PreferencesError::InvalidTextTrackpad);
        }
        if !(30..=60).contains(&self.touch_key_spacing_tenths)
            || !(40..=100).contains(&self.touch_row_spacing_tenths)
            || !(-12..=48).contains(&self.touch_keyboard_height_adjustment)
        {
            return Err(PreferencesError::InvalidTouchKeyboardSpacing);
        }
        if self.touch_quick_punctuation_symbols
            != normalize_touch_quick_symbols(&self.touch_quick_punctuation_symbols)
        {
            return Err(PreferencesError::InvalidTouchQuickSymbols);
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
        if self
            .candidate_translation_font_size
            .is_some_and(|size| !(12..=32).contains(&size))
        {
            return Err(PreferencesError::InvalidCandidateFontSize);
        }
        if !(50..=200).contains(&self.candidate_scale_percent)
            || !(50..=100).contains(&self.candidate_opacity_percent)
            || self
                .candidate_corner_radius
                .is_some_and(|radius| radius > 32)
            || self.candidate_padding.is_some_and(|padding| padding > 16)
            || [
                self.candidate_padding_horizontal,
                self.candidate_padding_vertical,
                self.candidate_item_padding_horizontal,
                self.candidate_item_padding_vertical,
                self.candidate_item_spacing,
            ]
            .into_iter()
            .flatten()
            .any(|value| value > 16)
            || self
                .candidate_item_corner_radius
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
        // 切换热键两两不同，且不占已被其他开关认走的组合键。保留表随各开关开合：开关关掉时组合键已经还给应用，绑给它不冲突。占用表的完整说明见 `KeybindingPreferences::occupied_toggle_chords`，设置页的录键试写走同一个表。
        let keybindings = &self.keybindings;
        let occupied: Vec<crate::key_chord::KeyChord> = keybindings
            .occupied_toggle_chords(self.voice_input.hotkey_ctrl_f9)
            .into_iter()
            .map(|(chord, _)| chord)
            .collect();
        let mut toggles: Vec<crate::key_chord::KeyChord> = Vec::new();
        for text in [
            &keybindings.toggle_english_candidates,
            &keybindings.toggle_full_width,
            &keybindings.toggle_punctuation,
            &keybindings.toggle_voice_input,
        ] {
            if text.trim().is_empty() {
                continue;
            }
            let chord = text
                .parse()
                .map_err(|_| PreferencesError::InvalidToggleHotkey)?;
            if toggles.contains(&chord) || occupied.contains(&chord) {
                return Err(PreferencesError::ConflictingToggleHotkeys);
            }
            toggles.push(chord);
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
    #[error("text edit button order must contain each of the seven side actions once")]
    InvalidTextEditButtonOrder,
    #[error("辅助码触发键必须为空或 ; / \\ ` = 中的一个标点")]
    InvalidHelpcodeTriggerKey,
    #[error("五笔二三候选键必须是两个不同的单个 ASCII 标点")]
    InvalidWubiCandidateSelection,
    #[error("floating toolbar settings are invalid")]
    InvalidFloatingToolbar,
    #[error("AI assistant provider or candidate limit is invalid")]
    InvalidAiAssistant,
    #[error(
        "网络代理地址必须是 host:port（可带 http:// 或 https:// 前缀），不能包含用户名、路径、查询或片段"
    )]
    InvalidNetwork,
    #[error("voice recognition or polishing provider is not supported")]
    InvalidVoiceInput,
    #[error("custom translation endpoint or API key is invalid")]
    InvalidCustomTranslation,
    #[error("Tencent translation credentials or region are invalid")]
    InvalidTencentTmt,
    #[error("NiuTrans translation credentials are invalid")]
    InvalidNiuTrans,
    #[error("candidate page size must be between 1 and 10")]
    InvalidPageSize,
    #[error("touch keyboard key spacing must be 3.0-6.0 and row spacing must be 4.0-10.0")]
    InvalidTouchKeyboardSpacing,
    #[error("触控板水平步长须为 4–48 dp，垂直步长须为 8–96 dp")]
    InvalidTextTrackpad,
    #[error("快捷符号必须是去重的单字素列表，最多十二项且不含空白或控制字符")]
    InvalidTouchQuickSymbols,
    #[error(
        "at least one touch keyboard scheme must be enabled and the selection must be visible"
    )]
    InvalidTouchKeyboardSchemes,
    #[error("custom shuangpin profile table is too large or has non-ASCII units or keys")]
    InvalidShuangpinCustomProfile,
    #[error("nine-key shuangpin custom table is too large or has non-ASCII units or keys")]
    InvalidNineKeyShuangpinCustomProfile,
    #[error("custom theme keyboard design is invalid")]
    InvalidTouchKeyboardSkinDesign,
    #[error("custom theme base must be system or a built-in theme")]
    InvalidCustomThemeBase,
    #[error("candidate font size must be between 12 and 32")]
    InvalidCandidateFontSize,
    #[error(
        "candidate window scale must be 50-200%, opacity 50-100%, corner radius 0-32 and padding 0-16"
    )]
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
    #[error(
        "candidate font family must be non-empty, contain no control characters, and be at most 128 bytes"
    )]
    InvalidCandidateFontFamily,
    #[error("custom theme candidate skin identifier is invalid")]
    InvalidCandidateSkin,
    #[error("word-to-character and paging cannot use the same keys")]
    ConflictingKeyBindings,
    #[error("切换热键必须是合法的快捷键组合；留空表示不绑定")]
    InvalidToggleHotkey,
    #[error("切换热键不能互相相同，也不能和已启用的快捷键相同")]
    ConflictingToggleHotkeys,
    #[error("frequency trigger count and linear step must be between 1 and 10")]
    InvalidFrequency,
    #[error("mixed English minimum prefix must be between 1 and 8")]
    InvalidMixedInput,
    #[error("plugin settings are invalid")]
    InvalidPlugins,
    #[error(
        "game compatibility process names must be distinct .exe base names of at most 64 characters, at most 32 in total"
    )]
    InvalidGameCompatibility,
    #[error(
        "app input mode rules must use distinct application identifiers of at most 64 bytes without control characters or path separators, at most 32 in total"
    )]
    InvalidAppInputModeRules,
    #[error(
        "vim mode app identifiers must be distinct of at most 64 bytes without control characters or path separators, at most 32 in total"
    )]
    InvalidVimModeApps,
    #[error(
        "cross-commit spacing app identifiers must be distinct of at most 64 bytes without control characters or path separators, at most 32 in total"
    )]
    InvalidCrossCommitSpacingApps,
    #[error(
        "cloud clipboard relay needs an https address (http only for loopback) without query, fragment or credentials, and a token of 16-256 printable ASCII characters"
    )]
    InvalidCloudClipboardRelay,
    #[error(
        "云候选配置无效：请检查地址、方法、请求头、JSON 模板、响应路径和超时（100–10000 毫秒）"
    )]
    InvalidCloudCandidateProvider,
    #[error("clipboard history limit must be one of 50, 100, 200, 500 or 1000")]
    InvalidClipboardHistoryLimit,
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
        // 还没有文件，也就没有未知键。
        self.save_locked(current, &[], 0, preferences)
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

    fn lock(&self) -> Result<crate::file_lock::ScopedFileLock, PreferencesError> {
        let lock = self.open_lock()?;
        let lock = crate::file_lock::ScopedFileLock::exclusive(lock)?;
        Ok(lock)
    }

    fn path(&self) -> PathBuf {
        self.directory.join("preferences.json")
    }

    fn read_locked(&self) -> Result<PreferencesSnapshot, PreferencesError> {
        self.read_locked_keeping_unknown()
            .map(|(snapshot, _unknown)| snapshot)
    }

    /// `read_locked`，另外交出文档里本版本不认识的偏好键，供保存时原样写回（见 [`UnknownPreferenceField`]）。
    fn read_locked_keeping_unknown(
        &self,
    ) -> Result<(PreferencesSnapshot, Vec<UnknownPreferenceField>), PreferencesError> {
        let path = self.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let snapshot = self.missing_document();
                crate::network::install(&snapshot.preferences.network);
                return Ok((snapshot, Vec::new()));
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
        let mut document: serde_json::Value = serde_json::from_slice(&bytes)?;
        // 只在读磁盘时回退新版本的未知布局；设置提交仍由枚举反序列化严格校验。
        if let Some(layout) = document.pointer_mut("/preferences/korean/layout") {
            if layout
                .as_str()
                .is_some_and(|value| !matches!(value, "dubeolsik" | "romaja"))
            {
                *layout = "dubeolsik".into();
            }
        }
        let (mut snapshot, unknown) =
            deserialize_keeping_unknown::<PreferencesSnapshot>(&document, &["preferences"])?;
        if snapshot.format_version != 1 {
            return Err(PreferencesError::UnsupportedFormat);
        }
        snapshot.preferences.validate()?;
        snapshot
            .preferences
            .voice_input
            .retire_unavailable_asr_model();
        snapshot
            .preferences
            .settle_smart_punctuation_default(SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT);
        // 读到哪份文档就按哪份文档的代理设置发请求：设置窗口和输入宿主都可能建 HTTP 客户端，
        // 每个进程自己那份 `network::current` 由它读到的最近一版偏好决定（见 `crate::network`）。
        crate::network::install(&snapshot.preferences.network);
        Ok((snapshot, unknown))
    }

    pub fn load(&self) -> Result<PreferencesSnapshot, PreferencesError> {
        let _lock = self.lock()?;
        self.read_locked()
    }

    /// None means the writer lock is busy; retry later without using defaults.
    /// File operations may still block on storage. Validation matches load().
    pub fn try_load(&self) -> Result<Option<PreferencesSnapshot>, PreferencesError> {
        let Some(_lock) = crate::file_lock::ScopedFileLock::try_exclusive(self.open_lock()?)?
        else {
            return Ok(None);
        };
        self.read_locked().map(Some)
    }

    /// Capture under the preferences lock so disabling cannot race a later write.
    /// Lock order is preferences, then clipboard history; never reverse it.
    pub fn capture_clipboard_text(&self, text: String) -> Result<bool, PreferencesError> {
        let _lock = self.lock()?;
        if !self.read_locked()?.preferences.clipboard_history {
            return Ok(false);
        }
        let limit = self.read_locked()?.preferences.clipboard_history_limit;
        let mut history = crate::clipboard::ClipboardHistoryStore::open_with_limit(
            self.directory.join("clipboard_history.json"),
            limit,
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
        let (current, unknown) = self.read_locked_keeping_unknown()?;
        self.save_locked(current, &unknown, expected_revision, preferences)
    }

    /// 「恢复默认设置」：在同一把锁里读出当前文档，换成本存储所属版本的默认偏好（[`Preferences::restored_to_defaults_for`]，服务凭据和 `fuzzy_pinyin.seeded` 保留），再按 `expected_revision` 做比较并交换写回。修订号不符时返回 `Conflict`，什么也不写。
    pub fn restore_defaults(
        &self,
        expected_revision: u64,
    ) -> Result<PreferencesSnapshot, PreferencesError> {
        let _lock = self.lock()?;
        let current = self.read_locked()?;
        let restored = current.preferences.restored_to_defaults_for(self.edition());
        // 恢复默认值连同更新版本写下的设置一起清掉：用户要的是整份回到默认。
        self.save_locked(current, &[], expected_revision, restored)
    }

    /// `save` 的主体，调用方已持有锁并读出了 `current`。`unknown` 是盘上文档里本版本不认识的键，写入时原样放回。
    fn save_locked(
        &self,
        current: PreferencesSnapshot,
        unknown: &[UnknownPreferenceField],
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
        // 写入方自己已经标了 seeded 时不再种：共享设置页首次打开时自己种好规则并标记，导入的设置文件带的是用户在另一台电脑上定下的规则（settings_document::import_document），这里再种会把它们盖成全部规则。
        if preferences.fuzzy_pinyin.enabled
            && !preferences.fuzzy_pinyin.seeded
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
        // 智能标点的一次性迁移标记同样只增不减：`current` 是按迁移后的样子读出来的，标记总在；写入方（设置页、宿主、MCP、同步）的偏好里没有这个键时照样带上，否则下一次读会把用户刚关掉的智能标点又打开。
        if current.preferences.smart_punctuation_migrated {
            preferences.smart_punctuation_migrated = true;
        }
        // 切换热键在文档里只有规范形一种写法：别名写法在写入处归一，与出厂值等价的写法按出厂值不落盘。
        preferences.keybindings.normalize_toggle_hotkeys();
        // 双拼辅助码触发方式的互斥规则在写入端归一：末尾间接与句中间接的同一枚触发键只能归一边（句中开着时归句中），`mid_sentence` 与 `direct_helpcode` 同真时按万象式直接。
        preferences.shuangpin_helpcode.modes = preferences.shuangpin_helpcode.modes.normalized();
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
        let mut document = serde_json::to_value(&snapshot)?;
        if let Some(preferences) = document.get_mut("preferences") {
            restore_unknown_fields(preferences, unknown);
        }
        atomic_write(
            &self.directory,
            &self.path(),
            &serde_json::to_vec_pretty(&document)?,
        )?;
        // 保存后立刻生效：设置页保存完不用等下一次读取，本进程接着发出的请求就用新代理。
        crate::network::install(&snapshot.preferences.network);
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

    /// `recover`, restricted to a document that is not well-formed JSON (truncated, empty, overwritten with other bytes). A well-formed document the schema rejects - a newer `format_version`, say - returns the load error unchanged, because it is most likely a newer build's file and rewriting it behind the user's back would lose that build's settings. Input method hosts call this automatically; the explicit settings-page repair uses `recover`.
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
        let backup_path = self.write_backup(&bytes, time::OffsetDateTime::now_utc())?;
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

    /// 按给定时间创建损坏文件备份并同步到磁盘，随后才能替换原文件；已有名称用 `-N` 后缀避让。
    fn write_backup(
        &self,
        bytes: &[u8],
        now: time::OffsetDateTime,
    ) -> Result<PathBuf, PreferencesError> {
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

/// 偏好文档里本版本不认识的一个键：从 `preferences` 对象起的键路径和它的原值。
///
/// 偏好文档由好几个程序各自按自己的版本读写：macOS 的输入法由 Sparkle 单独更新，设置应用和 `msime-mcp` 不跟着变，Windows、Linux 的宿主也会写设置页不认识的键。偏好结构因此不拒绝未知键，读取时记下它们，保存时放回原处，旧版本读得了新版本的文档，也不会把新版本的设置抹掉。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UnknownPreferenceField {
    path: Vec<String>,
    value: serde_json::Value,
}

/// 把 `document` 反序列化成 `T`，同时收集 `prefix`（逐级的对象键）之下本版本不认识的键。路径经过数组元素的键不收集：数组整个属于一个已知字段，元素位置在新旧版本之间没有对应关系。
pub(crate) fn deserialize_keeping_unknown<T: serde::de::DeserializeOwned>(
    document: &serde_json::Value,
    prefix: &[&str],
) -> Result<(T, Vec<UnknownPreferenceField>), serde_json::Error> {
    let mut paths: Vec<Vec<String>> = Vec::new();
    let value = serde_ignored::deserialize(document, |path| {
        if let Some(keys) = object_key_path(&path) {
            paths.push(keys);
        }
    })?;
    let unknown = paths
        .into_iter()
        .filter_map(|keys| {
            let rest = keys.strip_prefix(
                prefix
                    .iter()
                    .map(|key| key.to_string())
                    .collect::<Vec<_>>()
                    .as_slice(),
            )?;
            let value = keys
                .iter()
                .try_fold(document, |node, key| node.get(key.as_str()))?
                .clone();
            Some(UnknownPreferenceField {
                path: rest.to_vec(),
                value,
            })
        })
        .collect();
    Ok((value, unknown))
}

/// `serde_ignored` 报告的路径换成逐级的对象键；经过数组元素时返回 `None`。
fn object_key_path(path: &serde_ignored::Path<'_>) -> Option<Vec<String>> {
    match path {
        serde_ignored::Path::Root => Some(Vec::new()),
        serde_ignored::Path::Map { parent, key } => {
            let mut keys = object_key_path(parent)?;
            keys.push(key.clone());
            Some(keys)
        }
        serde_ignored::Path::Some { parent }
        | serde_ignored::Path::NewtypeStruct { parent }
        | serde_ignored::Path::NewtypeVariant { parent } => object_key_path(parent),
        serde_ignored::Path::Seq { .. } => None,
    }
}

/// 把读取时记下的未知键放回本版本写出的偏好对象。父对象这次没有写出（取默认值时省略）就不放：凭一个未知键重建半个已知对象，读它的版本会缺字段；本版本已经写出同名的键时以本版本为准。
fn restore_unknown_fields(preferences: &mut serde_json::Value, unknown: &[UnknownPreferenceField]) {
    for field in unknown {
        let Some((key, parents)) = field.path.split_last() else {
            continue;
        };
        let Some(serde_json::Value::Object(parent)) = parents
            .iter()
            .try_fold(&mut *preferences, |node, key| node.get_mut(key.as_str()))
        else {
            continue;
        };
        parent
            .entry(key.clone())
            .or_insert_with(|| field.value.clone());
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
    // 同理，没有智能标点迁移标记的损坏文档出自默认改变之前，垫底值里也去掉标记，读回来照旧文档那样迁移一次。
    if !source.contains_key("smart_punctuation_migrated") {
        salvaged.remove("smart_punctuation_migrated");
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
    let mut preferences: Preferences = serde_json::from_value(serde_json::Value::Object(salvaged))?;
    preferences.settle_smart_punctuation_default(SMART_PUNCTUATION_WAS_OFF_BY_DEFAULT);
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

/// 每页候选数的上限。数字键 1–9 选前九个，第十个由 0 键选（宿主各自按键位映射）。
pub const MAX_CANDIDATE_PAGE_SIZE: u8 = 10;

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
const DIAGNOSTIC_ENDPOINTS: [&str; 5] = [
    "/voice_input/asr_endpoint",
    "/voice_input/polish_endpoint",
    "/ai_assistant/endpoint",
    "/custom_translation/endpoint",
    "/cloud_clipboard_relay/url",
];

/// 去掉地址里的用户信息、查询串和片段；解析不了的地址整个换成 [`REDACTED`]，因为看不出密钥藏在哪。
fn diagnostic_endpoint(endpoint: &str) -> String {
    if endpoint.is_empty() {
        return String::new();
    }
    let Ok(mut url) = url::Url::parse(endpoint) else {
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

#[cfg(test)]
mod quick_symbols_tests {
    use super::*;
    #[test]
    fn touch_symbols_are_validated_and_persisted() {
        let symbols = vec![
            "☆".into(),
            "☆".into(),
            "ab".into(),
            "\n".into(),
            "\0".into(),
            "👩‍💻".into(),
        ];
        let normalized = normalize_touch_quick_symbols(&symbols);
        assert_eq!(normalized, ["☆", "👩‍💻"]);
        assert_eq!(
            normalize_touch_quick_symbols(&[]),
            default_touch_quick_symbols()
        );
        let directory = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(directory.path());
        let mut snapshot = store.load().unwrap();
        snapshot.preferences.touch_quick_punctuation_symbols = normalized.clone();
        store.save(snapshot.revision, snapshot.preferences).unwrap();
        assert_eq!(
            store
                .load()
                .unwrap()
                .preferences
                .touch_quick_punctuation_symbols,
            normalized
        );
        let invalid = Preferences {
            touch_quick_punctuation_symbols: symbols,
            ..Preferences::default()
        };
        assert!(invalid.validate().is_err());
        let many: Vec<_> = "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬".graphemes(true).map(str::to_owned).collect();
        assert_eq!(normalize_touch_quick_symbols(&many).len(), 12);
    }
}
